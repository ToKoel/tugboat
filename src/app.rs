use std::sync::Arc;

use ratatui::widgets::ListState;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub enum AppMode {
    #[default]
    Normal,
    ContextMenu,
    Logs,
}

#[derive(Clone, Default)]
pub struct AppState {
    pub container_data: Vec<(String, Vec<String>)>,
    pub selected: usize,
    pub mode: AppMode,
    pub menu_selected: usize,
    pub logs: Vec<String>,
    pub log_state: ListState,
    pub menu_items: Vec<&'static str>,
    pub horizontal_scroll: u16,
}

pub type SharedState = Arc<RwLock<AppState>>;
