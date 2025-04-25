use std::sync::Arc;

use ratatui::crossterm::event::KeyCode;
use smart_default::SmartDefault;
use tokio::{sync::RwLock, task::JoinHandle};

use crate::keybindings::default_keybindings;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum AppMode {
    #[default]
    Normal,
    ContextMenu,
    Logs,
    Search,
    Help,
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
    pub last_mode: AppMode,
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
            if self.mode == AppMode::Search {
                let search_keys = [KeyCode::Backspace, KeyCode::Enter, KeyCode::Esc];
                if !search_keys.contains(&key) {
                    if let KeyCode::Char(c) = key {
                        self.search_query.push(c);
                        return Action::Continue;
                    }
                }
            }
            if binding.keys.contains(&key) {
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

    fn get_app_state() -> AppState {
        AppState {
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
            logs: std::iter::repeat_n("log_line".to_string(), 50).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn handle_input_handles_exit() {
        let mut app = AppState {
            ..Default::default()
        };

        let result = app.handle_input(KeyCode::Esc);
        assert_eq!(Action::Exit, result);
    }

    #[test]
    fn esc_exits_search_input_and_clears_input() {
        let mut app = get_app_state();
        app.mode = AppMode::Search;
        app.search_query = "test".to_string();
        let action = app.handle_input(KeyCode::Esc);
        assert_eq!(Action::Continue, action);
        assert_eq!(AppMode::Logs, app.mode);
        assert_eq!("".to_string(), app.search_query);
    }

    #[test]
    fn enter_exits_search_input() {
        let mut app = get_app_state();
        app.mode = AppMode::Search;
        app.search_query = "test".to_string();
        let action = app.handle_input(KeyCode::Enter);
        assert_eq!(Action::Continue, action);
        assert_eq!(AppMode::Logs, app.mode);
        assert_eq!("test".to_string(), app.search_query);
    }

    #[test]
    fn bound_keys_can_be_entered_in_search() {
        let mut app = get_app_state();
        app.mode = AppMode::Search;
        let action = app.handle_input(KeyCode::Char('h'));
        assert_eq!(Action::Continue, action);
        assert_eq!("h".to_string(), app.search_query);
    }

    #[test]
    fn question_mark_opens_help() {
        let mut app = get_app_state();
        app.mode = AppMode::Logs;
        let action = app.handle_input(KeyCode::Char('?'));
        assert_eq!(Action::Continue, action);
        assert_eq!(AppMode::Help, app.mode);
    }

    #[test]
    fn enter_opens_context_menu() {
        let mut app = get_app_state();
        let action = app.handle_input(KeyCode::Enter);
        assert_eq!(Action::Continue, action);
        assert_eq!(AppMode::ContextMenu, app.mode);
    }

    #[test]
    fn enter_opens_logs() {
        let mut app = get_app_state();
        app.mode = AppMode::ContextMenu;
        app.menu_selected = 1;
        let action = app.handle_input(KeyCode::Up);
        let action_2 = app.handle_input(KeyCode::Enter);
        assert_eq!(Action::Continue, action);
        assert_eq!(Action::Continue, action_2);
        assert_eq!(AppMode::Logs, app.mode);
    }

    #[test]
    fn slash_opens_search() {
        let mut app = get_app_state();
        app.mode = AppMode::Logs;
        let action = app.handle_input(KeyCode::Char('/'));
        let action_2 = app.handle_input(KeyCode::Char('a'));
        assert_eq!(Action::Continue, action);
        assert_eq!(Action::Continue, action_2);
        assert_eq!(AppMode::Search, app.mode);
        assert_eq!("a".to_string(), app.search_query);
    }

    #[test]
    fn handle_input_scroll_up() {
        let mut app = get_app_state();
        app.selected = 1;
        let action = app.handle_input(KeyCode::Up);
        assert_eq!(app.selected, 0);
        assert_eq!(action, Action::Continue);

        app.selected = 1;
        let action_2 = app.handle_input(KeyCode::Char('k'));
        assert_eq!(app.selected, 0);
        assert_eq!(action_2, Action::Continue);
    }

    #[test]
    fn handle_input_scroll_down() {
        let mut app = get_app_state();
        app.selected = 0;
        let action = app.handle_input(KeyCode::Down);
        assert_eq!(app.selected, 1);
        assert_eq!(action, Action::Continue);

        app.selected = 0;
        let action = app.handle_input(KeyCode::Char('j'));
        assert_eq!(app.selected, 1);
        assert_eq!(action, Action::Continue);
    }
}
