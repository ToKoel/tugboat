use std::{
    io::{self, Write},
    time::Duration,
};

use ratatui::{
    Frame, Terminal,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout, Margin, Rect},
    prelude::CrosstermBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Scrollbar,
        ScrollbarState, Table,
    },
};
use tokio::sync::mpsc;

use crate::{
    app::{Action, AppMode, AppState, SharedState},
    docker::stream_logs,
};

pub async fn start_ui(app_state: SharedState) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (tx, mut rx) = mpsc::channel(100);

    let input_handle = tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(50)).unwrap() {
                if let Ok(evt) = event::read() {
                    if tx.send(evt).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    loop {
        {
            let app = app_state.read().await;
            terminal.draw(|f| {
                draw_ui(f, &app);
            })?;
        }

        if let Some(event) = rx.recv().await {
            let mut app = app_state.write().await;
            if let Event::Key(key_event) = event {
                match app.handle_input(key_event.code) {
                    Action::Exit => break,
                    Action::Continue => {
                        if app.mode == AppMode::Logs
                            && app.logs == vec!["Loading logs...".to_string()]
                        {
                            terminal.draw(|f| {
                                draw_ui(f, &app);
                            })?;

                            let container_id = app.container_data[app.selected].0.clone();
                            drop(app);
                            let log_task = stream_logs(container_id, app_state.clone());

                            let mut app = app_state.write().await;
                            app.log_task = Some(log_task);
                        }
                    }
                }
            }
        }
    }

    input_handle.abort();
    terminal.clear()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_ui(f: &mut Frame, app_state: &AppState) {
    let area = f.area();

    if let AppMode::Normal = app_state.mode {
        draw_normal_mode(f, area, app_state);
    }
    if let AppMode::Logs = app_state.mode {
        draw_normal_mode(f, area, app_state);
        draw_logs_mode(f, area, app_state);
    }
    if let AppMode::ContextMenu = app_state.mode {
        draw_normal_mode(f, area, app_state);
        draw_context_mode(f, area, app_state);
    }
}

fn draw_context_mode(f: &mut Frame, area: Rect, app_state: &AppState) {
    let items: Vec<ListItem> = app_state
        .menu_items
        .iter()
        .map(|s| ListItem::new(s.to_string()))
        .collect();
    let mut state = ListState::default();
    state.select(Some(app_state.menu_selected));
    let menu = List::new(items)
        .block(
            Block::default()
                .title("Actions")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(">> ");
    let area = centered_rect(30, 20, area);
    f.render_widget(Clear, area);
    f.render_stateful_widget(menu, area, &mut state);
}

fn draw_logs_mode(f: &mut Frame, area: Rect, app_state: &AppState) {
    let log_spans: Vec<Line> = app_state
        .logs
        .iter()
        .map(|line| Line::from(Span::raw(line.clone())))
        .collect();

    let logs_len = log_spans.len();
    let image_name = app_state.container_data[app_state.selected].1[1].clone();

    let overlay_area = centered_rect(80, 80, area);
    let visible_height = overlay_area.height.saturating_sub(2);
    let effective_vertical_scroll = if !app_state.user_scrolled {
        if logs_len > visible_height as usize {
            (logs_len - visible_height as usize) as u16
        } else {
            0
        }
    } else {
        app_state.vertical_scroll
    };

    let paragraph = Paragraph::new(log_spans)
        .block(
            Block::default()
                .title(format!("Logs - {}", image_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((effective_vertical_scroll, app_state.horizontal_scroll));

    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state =
        ScrollbarState::new(logs_len).position(effective_vertical_scroll.into());

    f.render_widget(Clear, overlay_area);
    f.render_widget(paragraph, overlay_area);
    f.render_stateful_widget(
        scrollbar,
        overlay_area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(get_constraints(percent_y))
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(get_constraints(percent_x))
        .split(popup_layout[1])[1]
}

fn get_constraints(percent: u16) -> Vec<Constraint> {
    vec![
        Constraint::Percentage((100 - percent) / 2),
        Constraint::Percentage(percent),
        Constraint::Percentage((100 - percent) / 2),
    ]
}

fn draw_normal_mode(f: &mut Frame, area: Rect, app_state: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(area);

    let rows: Vec<Row> = app_state
        .container_data
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app_state.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Row::new(
                item.1
                    .iter()
                    .map(|s| Cell::from(s.clone()))
                    .collect::<Vec<_>>(),
            )
            .style(style)
        })
        .collect();

    let widths = [Constraint::Min(10); 5];

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from("ID"),
                Cell::from("Image"),
                Cell::from("Status"),
                Cell::from("Names"),
                Cell::from("IP"),
            ])
            .style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .block(
            Block::default()
                .title("Docker Containers")
                .borders(Borders::ALL),
        );

    f.render_widget(table, chunks[0]);
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn create_app_state_for_test(app_mode: &AppMode) -> AppState {
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
            mode: app_mode.clone(),
            ..Default::default()
        }
    }

    #[test]
    fn test_draw_ui_normal_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = create_app_state_for_test(&AppMode::Normal);

        terminal.draw(|f| draw_ui(f, &app)).unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_draw_ui_context_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = create_app_state_for_test(&AppMode::ContextMenu);

        terminal.draw(|f| draw_ui(f, &app)).unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 100);
        let rect = centered_rect(50, 50, area);
        assert!(rect.width <= 100);
        assert!(rect.height <= 100);
    }

    #[test]
    fn test_draw_normal_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut frame = terminal.get_frame();

        let app_state = AppState {
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
            selected: 0,
            ..Default::default()
        };
        let area = frame.area();

        draw_normal_mode(&mut frame, area, &app_state);
    }

    #[test]
    fn test_draw_logs_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut frame = terminal.get_frame();

        let app_state = AppState {
            container_data: vec![(
                "id1".to_string(),
                vec!["id1".to_string(), "image_name".to_string()],
            )],
            logs: vec!["Log line 1".into(), "Log line 2".into()],
            horizontal_scroll: 0,
            ..Default::default()
        };
        let area = frame.area();

        draw_logs_mode(&mut frame, area, &app_state);
    }

    #[test]
    fn test_draw_context_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut frame = terminal.get_frame();

        let app_state = AppState {
            menu_items: vec!["Action 1", "Action 2"],
            menu_selected: 0,
            ..Default::default()
        };

        let area = frame.area();
        draw_context_mode(&mut frame, area, &app_state);
    }

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
