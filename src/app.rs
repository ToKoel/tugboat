use std::sync::Arc;

use ratatui::crossterm::event::KeyCode;
use smart_default::SmartDefault;
use tokio::{sync::RwLock, task::JoinHandle};

use crate::keybindings::{default_keybindings, matches_keys};

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
        for binding in default_keybindings() {
            if binding.matchers.iter().any(|m| matches_keys(&key, m)) {
                return (binding.action)(self, &key);
            }
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
