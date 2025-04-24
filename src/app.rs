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
    Search,
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
    #[default(_code = "vec![\"Show Logs\", \"Restart\"]")]
    pub menu_items: Vec<&'static str>,
    pub horizontal_scroll: u16,
    pub vertical_scroll: u16,
    pub log_task: Option<JoinHandle<()>>,
    #[default = false]
    pub user_scrolled: bool,
    pub visible_height: u16,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub current_match_index: Option<usize>,
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
                    self.horizontal_scroll = self.horizontal_scroll.saturating_sub(10);
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    self.horizontal_scroll = self.horizontal_scroll.saturating_add(10);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.user_scrolled = true;
                    self.vertical_scroll = self.vertical_scroll.saturating_add(1);
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.user_scrolled = true;
                    self.vertical_scroll = self.vertical_scroll.saturating_sub(1);
                }
                KeyCode::Char('G') => {
                    self.user_scrolled = false;
                    self.vertical_scroll = self.logs.len().saturating_sub(15) as u16;
                }
                KeyCode::Char('/') => {
                    self.mode = AppMode::Search;
                    self.search_query.clear();
                }
                KeyCode::Char('n') => {
                    if let Some(current) = self.current_match_index {
                        if !self.search_matches.is_empty() {
                            self.current_match_index =
                                Some((current + 1) % self.search_matches.len());
                            self.vertical_scroll =
                                self.search_matches[self.current_match_index.unwrap()] as u16;
                        }
                    }
                }
                KeyCode::Char('N') => {
                    if let Some(current) = self.current_match_index {
                        if !self.search_matches.is_empty() {
                            let len = self.search_matches.len();
                            self.current_match_index = Some((current + len - 1) % len);
                            self.vertical_scroll =
                                self.search_matches[self.current_match_index.unwrap()] as u16
                        }
                    }
                }
                _ => {}
            },
            AppMode::Search => match key {
                KeyCode::Esc => {
                    self.mode = AppMode::Logs;
                }
                KeyCode::Enter => {
                    self.search_matches = self
                        .logs
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.contains(&self.search_query))
                        .map(|(i, _)| i)
                        .collect();
                    self.current_match_index = if self.search_matches.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    if let Some(index) = self.current_match_index {
                        self.vertical_scroll = self.search_matches[index] as u16;
                    }
                    self.mode = AppMode::Logs;
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                }
                _ => {}
            },
        }
        Action::Continue
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    #[test]
    fn test_down_key_in_normal_mode() {
        let mut app = AppState {
            container_data: vec![
                (
                    "id1".to_string(),
                    vec![
                        "id1".into(),
                        "img1".into(),
                        "running".into(),
                        "name1".into(),
                        "127.0.0.1".into(),
                    ],
                ),
                (
                    "id2".to_string(),
                    vec![
                        "id2".into(),
                        "img2".into(),
                        "exited".into(),
                        "name2".into(),
                        "127.0.0.2".into(),
                    ],
                ),
            ],
            selected: 0,
            ..Default::default()
        };

        // down pressed
        if app.selected + 1 < app.container_data.len() {
            app.selected += 1;
        }

        assert_eq!(app.selected, 1);
    }

    #[test]
    fn test_up_key_in_normal_mode() {
        let mut app = AppState {
            container_data: vec![(
                "id1".to_string(),
                vec![
                    "id1".into(),
                    "img1".into(),
                    "running".into(),
                    "name1".into(),
                    "127.0.0.1".into(),
                ],
            )],
            selected: 1,
            ..Default::default()
        };

        // up pressed
        if app.selected > 0 {
            app.selected -= 1;
        }

        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_enter_key_opens_context_menu() {
        let mut app = AppState {
            mode: AppMode::Normal,
            ..Default::default()
        };

        // enter pressed
        app.mode = AppMode::ContextMenu;
        app.menu_selected = 0;

        assert_eq!(app.mode, AppMode::ContextMenu);
        assert_eq!(app.menu_selected, 0);
    }

    #[test]
    fn test_escape_in_context_menu_returns_to_normal() {
        let mut app = AppState {
            mode: AppMode::ContextMenu,
            ..Default::default()
        };

        // simulate Esc key
        app.mode = AppMode::Normal;

        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_menu_down_wraps() {
        let mut app = AppState {
            menu_items: vec!["View Logs", "Back"],
            menu_selected: 1,
            ..Default::default()
        };

        // simulate Down key
        if app.menu_selected + 1 < app.menu_items.len() {
            app.menu_selected += 1;
        } else {
            app.menu_selected = 0;
        }

        assert_eq!(app.menu_selected, 0);
    }

    #[test]
    fn test_menu_up_wraps() {
        let mut app = AppState {
            menu_items: vec!["View Logs", "Back"],
            menu_selected: 0,
            ..Default::default()
        };

        // simulate Up key
        if app.menu_selected > 0 {
            app.menu_selected -= 1;
        } else {
            app.menu_selected = app.menu_items.len() - 1;
        }

        assert_eq!(app.menu_selected, 1);
    }
}
