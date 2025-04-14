use futures::StreamExt;
use std::error::Error;

use shiplift::{ContainerListOptions, Docker, LogsOptions};
use strip_ansi_escapes::strip;

pub async fn fetch_logs(container_id: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let docker = Docker::new();
    let docker = docker.clone();
    let mut log_stream = docker
        .containers()
        .get(container_id)
        .logs(&LogsOptions::builder().stdout(true).stderr(true).build());

    let mut logs = Vec::new();

    while let Some(chunk) = log_stream.next().await {
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
    Ok(logs)
}

pub async fn get_container_data() -> Result<Vec<(String, Vec<String>)>, Box<dyn Error>> {
    let docker = Docker::new();
    let containers = match docker
        .containers()
        .list(&ContainerListOptions::default())
        .await
    {
        Ok(containers) => containers,
        Err(e) => {
            eprintln!("Failed to connect to Docker: {e}");
            return Err(Box::new(e) as Box<dyn std::error::Error>);
        }
    };

    let container_data: Vec<(String, Vec<String>)> =
        futures::future::join_all(containers.into_iter().map(|c| {
            let docker = docker.clone();
            async move {
                let id = c.id.to_string();
                let ip = docker
                    .containers()
                    .get(&c.id)
                    .inspect()
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
            }
        }))
        .await;
    return Ok(container_data);
}
