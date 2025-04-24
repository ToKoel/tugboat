use futures::StreamExt;
use std::{error::Error, pin::Pin};
use tokio::time::Duration;
use tokio::{task::JoinHandle, time};

use shiplift::{ContainerListOptions, Docker, LogsOptions, tty::TtyChunk};
use strip_ansi_escapes::strip;

use async_trait::async_trait;

use crate::app::SharedState;

const MAX_LOG_LINES: usize = 1000;
const CLEANUP_THRESHOLD: usize = 100;

type LogStream<'a> =
    Pin<Box<dyn futures::Stream<Item = Result<TtyChunk, shiplift::Error>> + Send + 'a>>;

#[async_trait]
pub trait DockerApi: Send + Sync {
    async fn get_containers(
        &self,
    ) -> Result<Vec<shiplift::rep::Container>, Box<dyn std::error::Error>>;

    async fn get_logs<'a>(
        &'a self,
        container_id: &'a str,
    ) -> Result<LogStream<'a>, Box<dyn std::error::Error>>;

    async fn inspect_container(
        &self,
        container_id: &str,
    ) -> Result<shiplift::rep::ContainerDetails, Box<dyn Error>>;
}

#[async_trait]
impl DockerApi for shiplift::Docker {
    async fn get_containers(
        &self,
    ) -> Result<Vec<shiplift::rep::Container>, Box<dyn std::error::Error>> {
        Ok(self
            .containers()
            .list(&ContainerListOptions::default())
            .await?)
    }

    async fn get_logs<'a>(
        &'a self,
        container_id: &'a str,
    ) -> Result<LogStream<'a>, Box<dyn std::error::Error>> {
        let stream = self
            .containers()
            .get(container_id)
            .logs(&LogsOptions::builder().stdout(true).stderr(true).build());
        Ok(Box::pin(stream))
    }

    async fn inspect_container(
        &self,
        container_id: &str,
    ) -> Result<shiplift::rep::ContainerDetails, Box<dyn Error>> {
        Ok(self.containers().get(container_id).inspect().await?)
    }
}

pub fn stream_logs(container_id: String, app_state: SharedState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let docker = Docker::new();
        let options = LogsOptions::builder()
            .stdout(true)
            .stderr(true)
            .follow(true)
            .tail("2000")
            .build();

        let mut log_stream = docker.containers().get(&container_id).logs(&options);
        let mut new_lines_since_cleanup = 0;
        let mut buffer: Vec<String> = Vec::new();

        let flush_interval = Duration::from_millis(100);
        let mut interval = time::interval(flush_interval);

        loop {
            tokio::select! {
                maybe_line = log_stream.next() => {
                    match maybe_line {
                        Some(Ok(chunk)) => {
                            let cleaned = strip(&*chunk);
                            let line = String::from_utf8_lossy(&cleaned).to_string();
                            buffer.push(line);
                            new_lines_since_cleanup += 1;
                        }
                        Some(Err(e)) => {
                            let mut app = app_state.write().await;
                            app.logs.push(format!("Error streaming logs: {e}"));
                        }
                        None => {
                            flush_buffer(&mut buffer, &app_state, &mut new_lines_since_cleanup).await;
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                            flush_buffer(&mut buffer, &app_state, &mut new_lines_since_cleanup).await;

                    }
            }
        }
    })
}

async fn flush_buffer(
    buffer: &mut Vec<String>,
    app_state: &SharedState,
    new_lines_since_cleanup: &mut usize,
) {
    if buffer.is_empty() {
        return;
    }

    let mut app = app_state.write().await;
    app.logs.append(buffer);
    let number_of_log_lines = app.logs.len();

    if !app.user_scrolled {
        if number_of_log_lines > app.visible_height as usize {
            app.vertical_scroll = (number_of_log_lines - app.visible_height as usize) as u16;
        } else {
            app.vertical_scroll = 0;
        }
    }

    if *new_lines_since_cleanup >= CLEANUP_THRESHOLD {
        if number_of_log_lines > MAX_LOG_LINES {
            let excess = number_of_log_lines - MAX_LOG_LINES;
            app.logs.drain(0..excess);
        }
        *new_lines_since_cleanup = 0;
    }
}

pub async fn get_container_data() -> Result<Vec<(String, Vec<String>)>, Box<dyn Error>> {
    let docker = Docker::new();
    get_container_data_with_api(&docker).await
}

async fn get_container_data_with_api(
    docker: &dyn DockerApi,
) -> Result<Vec<(String, Vec<String>)>, Box<dyn Error>> {
    let containers = docker.get_containers().await?;

    let container_data: Vec<(String, Vec<String>)> =
        futures::future::join_all(containers.into_iter().map(|c| async move {
            let id = c.id.to_string();
            let ip = docker
                .inspect_container(&id)
                .await
                .ok()
                .map(|info| info.network_settings)
                .map(|settings| settings.networks)
                .and_then(|mut networks| {
                    networks
                        .values_mut()
                        .next()
                        .map(|net| net.ip_address.clone())
                })
                .unwrap_or_else(|| "N/A".to_string());

            let row = vec![
                c.id[..12].to_string(),
                c.image,
                c.status,
                c.names.join(", "),
                ip,
            ];
            (id, row)
        }))
        .await;
    Ok(container_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use futures::stream;
    use shiplift::tty::TtyChunk;
    use std::{collections::HashMap, vec};

    struct MockDockerApi;

    #[async_trait]
    impl DockerApi for MockDockerApi {
        async fn get_containers(
            &self,
        ) -> Result<Vec<shiplift::rep::Container>, Box<dyn std::error::Error>> {
            Ok(vec![shiplift::rep::Container {
                id: "mock_id_1234567891".to_string(),
                image: "mock_image".to_string(),
                names: vec!["/mock_container".to_string()],
                status: "running".to_string(),
                command: "".to_string(),
                created: chrono::prelude::Utc::now(),
                image_id: "".to_string(),
                labels: HashMap::new(),
                ports: vec![],
                state: "".to_string(),
                size_rw: None,
                size_root_fs: None,
            }])
        }

        async fn get_logs<'a>(
            &'a self,
            _container_id: &'a str,
        ) -> Result<LogStream<'a>, Box<dyn std::error::Error>> {
            let chunks = vec![
                Ok(TtyChunk::StdOut(b"Log line 1\n".to_vec())),
                Ok(TtyChunk::StdOut(b"Log line 2\n".to_vec())),
            ];
            Ok(Box::pin(stream::iter(chunks)))
        }

        async fn inspect_container(
            &self,
            _container_id: &str,
        ) -> Result<shiplift::rep::ContainerDetails, Box<dyn std::error::Error>> {
            Ok(shiplift::rep::ContainerDetails {
                network_settings: shiplift::rep::NetworkSettings {
                    networks: {
                        let mut networks = std::collections::HashMap::new();
                        networks.insert(
                            "bridge".to_string(),
                            shiplift::rep::NetworkEntry {
                                ip_address: "172.17.0.2".to_string(),
                                gateway: "".to_string(),
                                global_ipv6_address: "".to_string(),
                                global_ipv6_prefix_len: 0,
                                ip_prefix_len: 0,
                                endpoint_id: "".to_string(),
                                mac_address: "".to_string(),
                                network_id: "".to_string(),
                                ipv6_gateway: "".to_string(),
                            },
                        );
                        networks
                    },
                    bridge: "".to_string(),
                    gateway: "".to_string(),
                    ip_prefix_len: 0,
                    ip_address: "".to_string(),
                    mac_address: "".to_string(),
                    ports: None,
                },
                app_armor_profile: "".to_string(),
                args: vec![],
                config: shiplift::rep::Config {
                    attach_stdout: false,
                    attach_stdin: false,
                    cmd: None,
                    attach_stderr: false,
                    domainname: "".to_string(),
                    entrypoint: None,
                    env: None,
                    exposed_ports: None,
                    hostname: "".to_string(),
                    image: "".to_string(),
                    labels: None,
                    on_build: None,
                    open_stdin: false,
                    stdin_once: false,
                    tty: false,
                    user: "".to_string(),
                    working_dir: "".to_string(),
                },
                created: chrono::prelude::Utc::now(),
                driver: "".to_string(),
                image: "".to_string(),
                id: "mock_id_1234567891".to_string(),
                restart_count: 0,
                resolv_conf_path: "".to_string(),
                process_label: "".to_string(),
                path: "".to_string(),
                log_path: "".to_string(),
                hosts_path: "".to_string(),
                hostname_path: "".to_string(),
                state: shiplift::rep::State {
                    error: "".to_string(),
                    exit_code: 0,
                    finished_at: chrono::prelude::Utc::now(),
                    oom_killed: false,
                    restarting: false,
                    paused: false,
                    pid: 0,
                    running: true,
                    started_at: chrono::prelude::Utc::now(),
                    status: "".to_string(),
                },
                host_config: shiplift::rep::HostConfig {
                    cgroup_parent: None,
                    container_id_file: "".to_string(),
                    cpuset_cpus: None,
                    cpu_shares: None,
                    memory: None,
                    memory_swap: None,
                    pid_mode: None,
                    network_mode: "".to_string(),
                    port_bindings: None,
                    privileged: false,
                    publish_all_ports: false,
                    readonly_rootfs: None,
                },
                mount_label: "".to_string(),
                name: "test".to_string(),
                mounts: vec![],
            })
        }
    }

    #[tokio::test]
    async fn test_get_container_data() {
        let mock = MockDockerApi;
        let data = get_container_data_with_api(&mock).await.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].1[0], "mock_id_1234");
        assert_eq!(data[0].1[1], "mock_image");
        assert_eq!(data[0].1[2], "running");
        assert_eq!(data[0].1[3], "/mock_container");
        assert_eq!(data[0].1[4], "172.17.0.2");
    }
}
