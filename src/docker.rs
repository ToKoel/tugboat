use bollard::Docker as BollardDocker;
use bollard::container::{CPUStats, ListContainersOptions, MemoryStats, MemoryStatsStats};
use futures::StreamExt;
use std::error::Error;
use tokio::time::{Duration, Instant};
use tokio::{task::JoinHandle, time};

use strip_ansi_escapes::strip;

use crate::app::SharedState;

const MAX_LOG_LINES: usize = 1000;
const CLEANUP_THRESHOLD: usize = 100;

fn calculate_cpu_usage(cpu_stats: CPUStats, pre_cpu_stats: CPUStats) -> Option<f64> {
    let cpu_delta: f64 =
        cpu_stats.cpu_usage.total_usage as f64 - pre_cpu_stats.cpu_usage.total_usage as f64;
    let system_cpu_delta =
        cpu_stats.system_cpu_usage? as f64 - pre_cpu_stats.system_cpu_usage? as f64;
    if system_cpu_delta == 0. {
        return None;
    }
    let numper_cpus = cpu_stats.online_cpus?;
    let cpu_usage = ((cpu_delta / system_cpu_delta) * numper_cpus as f64) * 100.0;
    Some(cpu_usage)
}

fn calculate_memory_usage(mem_stats: MemoryStats) -> Option<f64> {
    let cache = mem_stats.stats.map(|s| {
        if let MemoryStatsStats::V1(v1) = s {
            v1.cache
        } else {
            0
        }
    });
    let used_memory = mem_stats.usage? - cache?;
    let available_memory = mem_stats.limit?;
    if available_memory == 0 {
        return None;
    }
    Some((used_memory as f64 / available_memory as f64) * 100.0)
}

pub fn stream_stats(container_id: String, app_state: SharedState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let docker = BollardDocker::connect_with_socket_defaults().unwrap();
        let stream = &mut docker.stats(&container_id, None);
        let start_time = Instant::now();

        while let Some(result) = stream.next().await {
            match result {
                Ok(stats) => {
                    let cpu_stats = stats.cpu_stats;
                    let pre_cpu_stats = stats.precpu_stats;
                    let timestamp = start_time.elapsed().as_secs_f64();
                    let cpu_usage_result = calculate_cpu_usage(cpu_stats, pre_cpu_stats);
                    let mut app = app_state.write().await;
                    if let Some(cpu) = cpu_usage_result {
                        app.cpu_data.add((timestamp, cpu));
                    }

                    let mem = calculate_memory_usage(stats.memory_stats);
                    if let Some(mem) = mem {
                        app.mem_data.add((timestamp, mem));
                    }
                }
                Err(e) => eprintln!("Error: {}", e),
            }
        }
    })
}

pub fn stream_logs(container_id: String, app_state: SharedState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let docker = BollardDocker::connect_with_socket_defaults().unwrap();

        let options = Some(bollard::container::LogsOptions {
            follow: true,
            stdout: true,
            stderr: true,
            tail: "2000",
            ..Default::default()
        });

        let mut log_stream = docker.logs(&container_id, options);
        let mut new_lines_since_cleanup = 0;
        let mut buffer: Vec<String> = Vec::new();

        let flush_interval = Duration::from_millis(100);
        let mut interval = time::interval(flush_interval);

        loop {
            tokio::select! {
                maybe_line = log_stream.next() => {
                    match maybe_line {
                        Some(Ok(chunk)) => {
                            let cleaned = strip(chunk);
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
    let docker = BollardDocker::connect_with_socket_defaults().unwrap();
    let containers = &docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await
        .unwrap();

    let container_data: Vec<(String, Vec<String>)> =
        futures::future::join_all(containers.clone().into_iter().map(|container| async {
            let id = container.id.unwrap_or_default();

            let ip = docker
                .inspect_container(&id, None)
                .await
                .ok()
                .map(|info| info.network_settings)
                .and_then(|network_settings| {
                    if let Some(network_settings) = network_settings {
                        network_settings.ip_address
                    } else {
                        None
                    }
                })
                .unwrap_or("N/A".to_string());

            let row = vec![
                id[..12].to_string(),
                container.image.unwrap_or_default(),
                container.status.unwrap_or_default(),
                container.names.unwrap_or_default().join(", "),
                ip,
            ];
            (id, row)
        }))
        .await;
    Ok(container_data)
}
