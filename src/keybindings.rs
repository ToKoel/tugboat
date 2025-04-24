use ratatui::crossterm::event::KeyCode;

use crate::app::{Action, AppMode, AppState};

pub struct KeyBinding {
    pub matchers: Vec<KeyMatch>,
    pub description: &'static str,
    pub action: fn(&mut AppState, &KeyCode) -> Action,
}

pub enum KeyMatch {
    Exact(KeyCode),
    MatchFn(fn(&KeyCode) -> bool),
}
pub fn matches_keys(k: &KeyCode, matcher: &KeyMatch) -> bool {
    match matcher {
        KeyMatch::Exact(kk) => k == kk,
        KeyMatch::MatchFn(f) => f(k),
    }
}

pub fn default_keybindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding {
            matchers: vec![
                KeyMatch::Exact(KeyCode::Esc),
                KeyMatch::Exact(KeyCode::Char('q')),
            ],
            description: "Quit / Close dialog",
            action: |app, _| match app.mode {
                AppMode::Normal => Action::Exit,
                AppMode::Logs => {
                    if let Some(handle) = app.log_task.take() {
                        handle.abort();
                    }
                    app.mode = AppMode::Normal;
                    Action::Continue
                }
                AppMode::Search => {
                    app.mode = AppMode::Logs;
                    Action::Continue
                }
                AppMode::ContextMenu => {
                    app.mode = AppMode::Normal;
                    Action::Continue
                }
            },
        },
        KeyBinding {
            matchers: vec![
                KeyMatch::Exact(KeyCode::Up),
                KeyMatch::Exact(KeyCode::Char('k')),
            ],
            description: "Scroll up",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.selected = app.selected.saturating_sub(1);
                    Action::Continue
                }
                AppMode::Logs => {
                    app.user_scrolled = true;
                    app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                    Action::Continue
                }
                AppMode::ContextMenu => {
                    if app.menu_selected > 0 {
                        app.menu_selected -= 1;
                    } else {
                        app.menu_selected = app.menu_items.len() - 1;
                    }
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![
                KeyMatch::Exact(KeyCode::Down),
                KeyMatch::Exact(KeyCode::Char('j')),
            ],
            description: "Scroll down",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.selected = app.selected.saturating_add(1);
                    Action::Continue
                }
                AppMode::Logs => {
                    app.user_scrolled = true;
                    app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                    Action::Continue
                }
                AppMode::ContextMenu => {
                    if app.menu_selected + 1 < app.menu_items.len() {
                        app.menu_selected += 1;
                    } else {
                        app.menu_selected = 0;
                    }
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![
                KeyMatch::Exact(KeyCode::Left),
                KeyMatch::Exact(KeyCode::Char('h')),
            ],
            description: "Scroll left",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    app.horizontal_scroll = app.horizontal_scroll.saturating_sub(10);
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![
                KeyMatch::Exact(KeyCode::Right),
                KeyMatch::Exact(KeyCode::Char('l')),
            ],
            description: "Scroll right",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    app.horizontal_scroll = app.horizontal_scroll.saturating_add(10);
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Enter)],
            description: "Open / confirm",
            action: |app, _| match app.mode {
                AppMode::Normal => {
                    app.mode = AppMode::ContextMenu;
                    app.menu_selected = 0;
                    Action::Continue
                }
                AppMode::ContextMenu => match app.menu_selected {
                    0 => {
                        app.mode = AppMode::Logs;
                        app.logs = vec!["Loading logs...".to_string()];
                        Action::Continue
                    }
                    1 => {
                        app.mode = AppMode::Normal;
                        Action::Continue
                    }
                    _ => Action::Continue,
                },
                AppMode::Search => {
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
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Backspace)],
            description: "Delete character in search",
            action: |app, _| match app.mode {
                AppMode::Search => {
                    app.search_query.pop();
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Char('G'))],
            description: "Jump to latest log entry",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    app.user_scrolled = false;
                    app.vertical_scroll = app.logs.len().saturating_sub(15) as u16;
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Char('/'))],
            description: "Open search",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    app.mode = AppMode::Search;
                    app.search_query.clear();
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Char('n'))],
            description: "Jump to next match",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    if let Some(current) = app.current_match_index {
                        if !app.search_matches.is_empty() {
                            app.current_match_index =
                                Some((current + 1) % app.search_matches.len());
                            app.vertical_scroll =
                                app.search_matches[app.current_match_index.unwrap()] as u16;
                        }
                    }
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::Exact(KeyCode::Char('N'))],
            description: "Jump to previous match",
            action: |app, _| match app.mode {
                AppMode::Logs => {
                    if let Some(current) = app.current_match_index {
                        if !app.search_matches.is_empty() {
                            let len = app.search_matches.len();
                            app.current_match_index = Some((current + len - 1) % len);
                            app.vertical_scroll =
                                app.search_matches[app.current_match_index.unwrap()] as u16
                        }
                    }
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
        KeyBinding {
            matchers: vec![KeyMatch::MatchFn(|k| matches!(k, KeyCode::Char(_)))],
            description: "Raw input for search",
            action: |app, key| match app.mode {
                AppMode::Search => {
                    if let KeyCode::Char(c) = key {
                        app.search_query.push(*c);
                    }
                    Action::Continue
                }
                _ => Action::Continue,
            },
        },
    ]
}
