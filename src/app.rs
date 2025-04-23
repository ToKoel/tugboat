use std::sync::Arc;

use ratatui::{crossterm::event::KeyCode, widgets::ListState};
use smart_default::SmartDefault;
use tokio::{sync::RwLock, task::JoinHandle};

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Normal,
    ContextMenu,
    Logs,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Continue,
    Exit,
}

#[derive(SmartDefault)]
pub struct AppState {
    pub container_data: Vec<(String, Vec<String>)>,
    pub selected: usize,
    pub mode: AppMode,
    pub menu_selected: usize,
    pub logs: Vec<String>,
    pub log_state: ListState,
    #[default(_code = "vec![\"Show Logs\", \"Restart\"]")]
    pub menu_items: Vec<&'static str>,
    pub horizontal_scroll: u16,
    pub vertical_scroll: u16,
    pub log_task: Option<JoinHandle<()>>,
    pub user_scrolled_up: bool,
}

pub type SharedState = Arc<RwLock<AppState>>;

impl AppState {
    pub fn handle_input(&mut self, key: KeyCode) -> Action {
        match self.mode {
            AppMode::Normal => match key {
                KeyCode::Char('q') => return Action::Exit,
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.selected + 1 < self.container_data.len() {
                        self.selected += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected > 0 {
                        self.selected -= 1;
                    }
                }
                KeyCode::Enter => {
                    self.mode = AppMode::ContextMenu;
                    self.menu_selected = 0;
                }
                _ => {}
            },
            AppMode::ContextMenu => match key {
                KeyCode::Esc | KeyCode::Char('q') => self.mode = AppMode::Normal,
                KeyCode::Down => {
                    if self.menu_selected + 1 < self.menu_items.len() {
                        self.menu_selected += 1;
                    } else {
                        self.menu_selected = 0;
                    }
                }
                KeyCode::Up => {
                    if self.menu_selected > 0 {
                        self.menu_selected -= 1;
                    } else {
                        self.menu_selected = self.menu_items.len() - 1;
                    }
                }
                KeyCode::Enter => match self.menu_selected {
                    0 => {
                        self.mode = AppMode::Logs;
                        self.logs = vec!["Loading logs...".to_string()];
                        self.log_state.select(Some(0));
                    }
                    1 => {
                        self.mode = AppMode::Normal;
                    }
                    _ => {}
                },
                _ => {}
            },
            AppMode::Logs => match key {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                    if let Some(handle) = self.log_task.take() {
                        handle.abort();
                    }
                    self.mode = AppMode::Normal;
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    if self.horizontal_scroll > 0 {
                        self.horizontal_scroll -= 10;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.horizontal_scroll += 10;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.vertical_scroll += 1;
                    self.user_scrolled_up = true;
                    let selected = self.log_state.selected();
                    if selected.unwrap_or(0) + 1 < self.logs.len() {
                        self.log_state.select(Some(selected.unwrap_or(0) + 1));
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.vertical_scroll > 0 {
                        self.vertical_scroll -= 1;
                    }
                    self.user_scrolled_up = true;
                    let selected = self.log_state.selected();
                    if selected.unwrap_or(0) > 0 {
                        self.log_state.select(Some(selected.unwrap_or(0) - 1));
                    }
                }
                _ => {}
            },
        }
        Action::Continue
    }
}
