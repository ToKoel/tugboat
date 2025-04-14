mod app;
mod docker;
mod ui;

use std::{error::Error, sync::Arc, vec};

use app::{AppMode, AppState};
use docker::get_container_data;
use ratatui::widgets::ListState;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let container_data = get_container_data().await?;
    let app_state = Arc::new(RwLock::new(AppState {
        container_data,
        selected: 0,
        mode: AppMode::Normal,
        menu_selected: 0,
        logs: vec![],
        log_state: ListState::default(),
        menu_items: vec!["Show Logs", "Restart"],
        horizontal_scroll: 0,
    }));

    ui::start_ui(app_state)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

    Ok(())
}
