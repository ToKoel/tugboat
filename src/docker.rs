use futures::StreamExt;
use std::{error::Error, pin::Pin};

use shiplift::{ContainerListOptions, Docker, LogsOptions, tty::TtyChunk};
use strip_ansi_escapes::strip;

use async_trait::async_trait;

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

pub async fn fetch_logs(container_id: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let docker = Docker::new();
    fetch_logs_with_api(&docker, container_id).await
}

async fn fetch_logs_with_api(
    docker: &dyn DockerApi,
    container_id: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let log_stream = docker.get_logs(container_id).await?;
    Ok(fetch_logs_from_stream(log_stream).await)
}

async fn fetch_logs_from_stream<S>(mut stream: S) -> Vec<String>
where
    S: futures::Stream<Item = Result<TtyChunk, shiplift::Error>> + Unpin,
{
    let mut logs = Vec::new();

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(data) => {
                let cleaned = strip(&*data);
                logs.push(String::from_utf8_lossy(&cleaned).to_string());
                if logs.len() > 100 {
                    logs.remove(0);
                }
            }
            Err(_) => break,
        }
    }
    logs
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
    use std::collections::HashMap;

    struct MockDockerApi;

    #[async_trait]
    impl DockerApi for MockDockerApi {
        async fn get_containers(
            &self,
        ) -> Result<Vec<shiplift::rep::Container>, Box<dyn std::error::Error>> {
            Ok(vec![shiplift::rep::Container {
                id: "mock_id".to_string(),
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
                },
            })
        }
    }

    #[tokio::test]
    async fn test_fetch_logs() {
        let mock = MockDockerApi;
        let logs = fetch_logs_with_api(&mock, "mock_id").await.unwrap();
        assert_eq!(logs.len(), 2);
        assert!(logs[0].contains("Log line 1"));
        assert!(logs[1].contains("Log line 2"));
    }

    #[tokio::test]
    async fn test_get_container_data() {
        let mock = MockDockerApi;
        let data = get_container_data_with_api(&mock).await.unwrap();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].1[0], "mock_id".to_string()[..12]);
        assert_eq!(data[0].1[1], "mock_image");
    }
}
