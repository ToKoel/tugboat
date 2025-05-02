mod app;
mod docker;
mod keybindings;
mod max_sliding_window;
mod ui;

use std::{error::Error, sync::Arc};

use app::AppState;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app_state = Arc::new(RwLock::new(AppState {
        ..Default::default()
    }));

    ui::start_ui(app_state)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(())
}
