mod app;
mod docker;
mod ui;

use std::{error::Error, sync::Arc};

use app::AppState;
use docker::get_container_data;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let container_data = get_container_data().await?;
    let app_state = Arc::new(RwLock::new(AppState {
        container_data,
        ..Default::default()
    }));

    ui::start_ui(app_state)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(())
}
