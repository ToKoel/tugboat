use std::{
    io::{self},
    time::Duration,
    vec,
};

use ratatui::{
    Frame, Terminal,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    prelude::CrosstermBackend,
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        Axis, Block, Borders, Cell, Chart, Clear, Dataset, List, ListItem, ListState, Paragraph,
        Row, Scrollbar, ScrollbarState, Table, Wrap,
    },
};

use crate::{
    app::{AppMode, AppState, SharedState},
    docker::{get_container_data, stream_logs, stream_stats},
    keybindings::default_keybindings,
};

pub async fn start_ui(app_state: SharedState) -> Result<(), io::Error> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        {
            let app = app_state.read().await;
            terminal.draw(|f| {
                draw_ui(f, &app);
            })?;
            if !app.running {
                break;
            }
        }

        if event::poll(Duration::from_millis(200))? {
            let mut app = app_state.write().await;
            let container_data = get_container_data().await;
            app.container_data = container_data.unwrap_or(Vec::new());
            if let Event::Key(key_event) = event::read()? {
                app.handle_input(key_event.code);
                if app.mode == AppMode::Logs && app.logs == vec!["Loading logs...".to_string()] {
                    terminal.draw(|f| {
                        draw_ui(f, &app);
                        let area = f.area();
                        let overlay_area = centered_rect(80, 80, area);
                        let visible_height = overlay_area.height.saturating_sub(2);
                        app.visible_height = visible_height;
                    })?;

                    let container_id = app.container_data[app.selected].0.clone();
                    let log_task = stream_logs(container_id, app_state.clone());
                    app.log_task = Some(log_task);
                }
                if app.mode == AppMode::Resources {
                    let container_id = app.container_data[app.selected].0.clone();
                    let stats_task = stream_stats(container_id, app_state.clone());
                    app.stats_task = Some(stats_task);
                }
            }
        }
    }

    terminal.clear()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    terminal.show_cursor()?;
    Ok(())
}

fn draw_ui(f: &mut Frame, app_state: &AppState) {
    let area = f.area();

    match app_state.mode {
        AppMode::Normal => {
            draw_normal_mode(f, area, app_state, false);
        }
        AppMode::ContextMenu => {
            draw_normal_mode(f, area, app_state, true);
            draw_context_mode(f, area, app_state);
        }
        AppMode::Logs => {
            draw_normal_mode(f, area, app_state, true);
            draw_logs_mode(f, area, app_state);
        }
        AppMode::Search => {
            let mut rect;
            if app_state.last_mode == AppMode::Logs {
                rect = draw_normal_mode(f, area, app_state, true);
                rect = draw_logs_mode(f, area, app_state);
            } else {
                rect = draw_normal_mode(f, area, app_state, false);
            }
            draw_search_mode(f, rect, app_state);
        }
        AppMode::Help => {
            draw_help(f, area);
        }
        AppMode::Resources => {
            draw_normal_mode(f, area, app_state, true);
            draw_resource_graph(f, area, app_state);
        }
    }
}

fn get_stats_graph<'a>(
    data_points: &'a Vec<(f64, f64)>,
    max_value: f64,
    title: &'a str,
) -> Chart<'a> {
    let dataset = Dataset::default()
        .marker(symbols::Marker::Braille)
        .graph_type(ratatui::widgets::GraphType::Line)
        .style(Style::default().fg(Color::Cyan))
        .data(&data_points);

    let mut x_start = 0.0;
    let mut x_end = 1.0;
    if !data_points.is_empty() {
        x_start = data_points[0].0;
        x_end = data_points[data_points.len() - 1].0;
    }
    let rounded_start = x_start.round() as i64;
    let rounded_end = x_end.round() as i64;

    let y_end = max_value;
    let y_mid = y_end / 2.0;

    Chart::new(vec![dataset])
        .block(Block::default().borders(Borders::NONE))
        .x_axis(
            Axis::default()
                .title("Time (s)")
                .bounds([x_start, x_end])
                .labels(vec![rounded_start.to_string(), rounded_end.to_string()]),
        )
        .y_axis(
            Axis::default()
                .title(title)
                .style(Style::default().fg(Color::Gray))
                .bounds([-1.0, y_end])
                .labels(vec![
                    "0.0".to_string(),
                    format!("{:.2}", y_mid).into(),
                    format!("{:.2}", y_end).into(),
                ]),
        )
}

fn draw_resource_graph(f: &mut Frame, area: Rect, app_state: &AppState) {
    let cpu_points: Vec<(f64, f64)> = app_state.cpu_data.data.iter().cloned().collect();
    let cpu_max = app_state.cpu_data.get_max().unwrap_or(101.0);
    let cpu_chart = get_stats_graph(&cpu_points, cpu_max, "CPU %");

    let mem_points: Vec<(f64, f64)> = app_state.mem_data.data.iter().cloned().collect();
    let mem_max = app_state.mem_data.get_max().unwrap_or(101.0);
    let mem_chart = get_stats_graph(&mem_points, mem_max, "Memory %");

    let overlay_area = centered_rect(80, 80, area);
    let outer_block = Block::default()
        .title("Resource Usage")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black));
    f.render_widget(Clear, overlay_area);
    f.render_widget(outer_block, overlay_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(overlay_area);

    f.render_widget(cpu_chart, centered_rect(90, 90, chunks[0]));
    f.render_widget(mem_chart, centered_rect(90, 90, chunks[1]));
}

fn draw_help(f: &mut Frame, area: Rect) {
    let lines: Vec<Line> = default_keybindings()
        .iter()
        .map(|binding| {
            let keys: Vec<String> = binding.keys.iter().map(|key| format!("{}", key)).collect();
            let key_text = keys.join(" / ");

            Line::from(vec![
                Span::styled(key_text, Style::default().fg(Color::Yellow)),
                Span::raw(" — "),
                Span::raw(binding.description),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .title("Help - Key Bindings")
                .borders(Borders::ALL),
        )
        .wrap(Wrap { trim: true })
        .alignment(Alignment::Left);

    let popup_area = centered_rect(60, 70, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn draw_search_mode(f: &mut Frame, area: Rect, app_state: &AppState) {
    let search_prompt = Paragraph::new(Span::raw(format!("/{}", app_state.search_query)))
        .block(Block::default().borders(Borders::ALL).title("Search"));

    let search_height = 3;
    let bottom_area = Rect {
        x: area.x,
        y: area.y + area.height.saturating_sub(search_height),
        width: area.width,
        height: search_height,
    };

    f.render_widget(Clear, bottom_area);
    f.render_widget(search_prompt, bottom_area);
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

fn draw_logs_mode(f: &mut Frame, area: Rect, app_state: &AppState) -> Rect {
    let log_spans: Vec<Line> = app_state
        .logs
        .iter()
        .map(|line| {
            if let Some(query) =
                (!app_state.search_query.is_empty()).then_some(&app_state.search_query)
            {
                if line.contains(query) {
                    let highlighted = line.replace(query, &format!("[{}]", query));
                    Line::from(Span::styled(
                        highlighted,
                        Style::default().fg(Color::Yellow),
                    ))
                } else {
                    Line::from(Span::raw(line.clone()))
                }
            } else {
                Line::from(Span::raw(line.clone()))
            }
        })
        .collect();

    let logs_len = log_spans.len();
    let image_name = app_state.container_data[app_state.selected].1[1].clone();

    let overlay_area = centered_rect(80, 80, area);

    let paragraph = Paragraph::new(log_spans)
        .block(
            Block::default()
                .title(format!("Logs - {}", image_name))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .scroll((app_state.vertical_scroll, app_state.horizontal_scroll));

    let scrollbar = Scrollbar::new(ratatui::widgets::ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state =
        ScrollbarState::new(logs_len).position(app_state.vertical_scroll.into());

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
    overlay_area
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

fn draw_normal_mode(f: &mut Frame, area: Rect, app_state: &AppState, blurred: bool) -> Rect {
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
            let mut matched= false;

            if let Some(query) =
                (!app_state.search_query.is_empty()).then_some(&app_state.search_query)
            {
                if item.1[1].contains(query) {
                    matched = true;
                }
            } 
            
            let mut style = if i == app_state.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            if blurred {
                style = style.add_modifier(Modifier::DIM);
            }
            if matched {
                style = style.bg(Color::Cyan);
            }
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

    let mut header_style = Style::default().add_modifier(Modifier::BOLD);
    let mut title_style = Style::default();
    if blurred {
        header_style = header_style.add_modifier(Modifier::DIM);
        title_style = title_style.add_modifier(Modifier::DIM);
    }

    let table = Table::new(rows, widths)
        .header(
            Row::new(vec![
                Cell::from("ID"),
                Cell::from("Image"),
                Cell::from("Status"),
                Cell::from("Names"),
                Cell::from("IP"),
            ])
            .style(header_style),
        )
        .block(
            Block::default()
                .title("Docker Containers")
                .borders(Borders::ALL)
                .style(title_style),
        );

    f.render_widget(table, chunks[0]);
    area
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
            logs: std::iter::repeat_n("log_line".to_string(), 50).collect(),
            vertical_scroll: 10,
            search_query: "log".to_string(),
            mode: *app_mode,
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
    fn test_draw_ui_stats_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let mut app = create_app_state_for_test(&AppMode::Resources);
        app.cpu_data.add((1.0, 10.0));
        app.cpu_data.add((10.0, 40.0));

        app.mem_data.add((1.0, 5.0));
        app.mem_data.add((10.0, 32.0));

        terminal.draw(|f| draw_ui(f, &app)).unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_draw_ui_help_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = create_app_state_for_test(&AppMode::Help);

        terminal.draw(|f| draw_ui(f, &app)).unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_draw_ui_log_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = create_app_state_for_test(&AppMode::Logs);

        terminal.draw(|f| draw_ui(f, &app)).unwrap();

        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_draw_ui_search_mode_snapshot() {
        let mut terminal = Terminal::new(TestBackend::new(80, 20)).unwrap();
        let app = create_app_state_for_test(&AppMode::Search);

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
}
