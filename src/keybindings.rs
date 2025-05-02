use ratatui::crossterm::event::KeyCode;

use crate::app::{AppMode, AppState};

pub struct KeyBinding {
    pub keys: Vec<KeyCode>,
    pub description: &'static str,
    pub action: fn(&mut AppState, &KeyCode),
}

pub fn default_keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding {
            keys: vec![KeyCode::Esc, KeyCode::Char('q')],
            description: "Quit / Close dialog",
            action: |app, _| match app.mode {
                AppMode::Normal => app.running = false,
                AppMode::Logs => {
                    if let Some(handle) = app.log_task.take() {
                        handle.abort();
                    }
                    app.mode = AppMode::Normal;
                }
                AppMode::Search => {
                    app.mode = AppMode::Logs;
                    app.search_query.clear();
                    app.search_matches.clear();
                }
                AppMode::ContextMenu => {
                    app.mode = AppMode::Normal;
                }
                AppMode::Help => {
                    app.mode = app.last_mode;
                }
                AppMode::Resources => {
                    if let Some(handle) = app.stats_task.take() {
                        handle.abort();
                    }
                    app.mode = AppMode::Normal;
                    app.cpu_data.clear();
                    app.mem_data.clear();
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Up, KeyCode::Char('k')],
            description: "Scroll up",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.selected = app.selected.saturating_sub(1);
                }
                AppMode::Logs => {
                    app.user_scrolled = true;
                    app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                }
                AppMode::ContextMenu => {
                    if app.menu_selected > 0 {
                        app.menu_selected -= 1;
                    } else {
                        app.menu_selected = app.menu_items.len() - 1;
                    }
                }
                _ => {}
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Down, KeyCode::Char('j')],
            description: "Scroll down",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.selected = app.selected.saturating_add(1);
                }
                AppMode::Logs => {
                    app.user_scrolled = true;
                    app.vertical_scroll = app.vertical_scroll.saturating_add(1);
                }
                AppMode::ContextMenu => {
                    if app.menu_selected + 1 < app.menu_items.len() {
                        app.menu_selected += 1;
                    } else {
                        app.menu_selected = 0;
                    }
                }
                _ => {}
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Left, KeyCode::Char('h')],
            description: "Scroll left",
            action: |app, _| {
                if app.mode == AppMode::Logs {
                    app.horizontal_scroll = app.horizontal_scroll.saturating_sub(10);
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Right, KeyCode::Char('l')],
            description: "Scroll right",
            action: |app, _| {
                if app.mode == AppMode::Logs {
                    app.horizontal_scroll = app.horizontal_scroll.saturating_add(10);
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Enter],
            description: "Open / confirm",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.mode = AppMode::ContextMenu;
                    app.menu_selected = 0;
                }
                AppMode::ContextMenu => match app.menu_selected {
                    0 => {
                        app.mode = AppMode::Logs;
                        app.logs = vec!["Loading logs...".to_string()];
                    }
                    1 => {
                        app.mode = AppMode::Resources;
                    }
                    2 => {
                        app.mode = AppMode::Normal;
                    }
                    _ => {}
                },
                AppMode::Search => {
                    if app.last_mode == AppMode::Logs {
                        app.search_matches = app
                        .logs
                        .iter()
                        .enumerate()
                        .filter(|(_, line)| line.contains(&app.search_query))
                        .map(|(i, _)| i)
                        .collect();
                    app.current_match_index = if app.search_matches.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    if let Some(index) = app.current_match_index {
                        app.vertical_scroll = app.search_matches[index] as u16;
                    }
                    app.mode = AppMode::Logs;
                } else {
                    app.search_matches = app.container_data.iter().enumerate()
                    .filter(|(_, data)| data.1[1].contains(&app.search_query))
                    .map(|(i, _)| i)
                    .collect();
                    app.current_match_index = if app.search_matches.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                    if let Some(index) = app.current_match_index {
                        app.selected = index;
                    }
                    app.mode = AppMode::Normal;
                }
                }
                _ => {}
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Backspace],
            description: "Delete character in search",
            action: |app, _| {
                if app.mode == AppMode::Search {
                    app.search_query.pop();
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Char('G')],
            description: "Jump to latest log entry",
            action: |app, _| {
                if app.mode == AppMode::Logs {
                    app.user_scrolled = false;
                    app.vertical_scroll = app.logs.len().saturating_sub(15) as u16;
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Char('/')],
            description: "Open search",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    app.last_mode = AppMode::Logs;
                    app.mode = AppMode::Search;
                    app.search_query.clear();
                }
                AppMode::Normal => {
                    app.last_mode = AppMode::Normal;
                    app.mode = AppMode::Search;
                    app.search_query.clear();
                }
                _ => {}
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Char('n')],
            description: "Jump to next match",
            action: |app, _| {
                if app.mode == AppMode::Logs {
                    if let Some(current) = app.current_match_index {
                        if !app.search_matches.is_empty() {
                            app.current_match_index =
                                Some((current + 1) % app.search_matches.len());
                            app.vertical_scroll =
                                app.search_matches[app.current_match_index.unwrap()] as u16;
                        }
                    }
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Char('N')],
            description: "Jump to previous match",
            action: |app, _| {
                if app.mode == AppMode::Logs {
                    if let Some(current) = app.current_match_index {
                        if !app.search_matches.is_empty() {
                            let len = app.search_matches.len();
                            app.current_match_index = Some((current + len - 1) % len);
                            app.vertical_scroll =
                                app.search_matches[app.current_match_index.unwrap()] as u16
                        }
                    }
                }
            },
        },
        KeyBinding {
            keys: vec![KeyCode::Char('?')],
            description: "Open help",
            action: |app, _| {
                app.last_mode = app.mode;
                app.mode = AppMode::Help;
            },
        },
    ]
}
