use std::{error::Error, io, sync::Arc, time::Duration, vec};

use futures::StreamExt;
use ratatui::{
    Frame, Terminal,
    crossterm::{
        event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
    layout::{Constraint, Direction, Layout, Rect},
    prelude::CrosstermBackend,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Table},
};
use shiplift::{ContainerListOptions, Docker, LogsOptions};
use strip_ansi_escapes::strip;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
enum AppMode {
    #[default]
    Normal,
    ContextMenu,
    Logs,
}

#[derive(Clone, Default)]
struct AppState {
    container_data: Vec<(String, Vec<String>)>,
    selected: usize,
    mode: AppMode,
    menu_selected: usize,
    logs: Vec<String>,
    log_state: ListState,
    menu_items: Vec<&'static str>,
    horizontal_scroll: u16,
}

type SharedState = Arc<RwLock<AppState>>;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let docker = Docker::new();
    let containers = docker
        .containers()
        .list(&ContainerListOptions::default())
        .await?;

    let container_data: Vec<(String, Vec<String>)> =
        futures::future::join_all(containers.into_iter().map(|c| {
            let docker = docker.clone();
            async move {
                let id = c.id.to_string();
                let ip = docker
                    .containers()
                    .get(&c.id)
                    .inspect()
                    .await
                    .ok()
                    .map(|info| info.network_settings)
                    .map(|settings| settings.networks)
                    .and_then(|mut networks| {
                        networks
                            .values_mut()
                            .next()
                            .map(|net| net.ip_address.clone())
                    })
                    .unwrap_or_else(|| "N/A".to_string());

                let row = vec![
                    c.id[..12].to_string(),
                    c.image,
                    c.status,
                    c.names.join(", "),
                    ip,
                ];
                (id, row)
            }
        }))
        .await;

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

    start_ui(app_state).await?;

    Ok(())
}

async fn start_ui(app_state: SharedState) -> Result<(), io::Error> {
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
        }

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                let mut app = app_state.write().await;
                match app.mode {
                    AppMode::Normal => match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Down | KeyCode::Char('j') => {
                            if app.selected + 1 < app.container_data.len() {
                                app.selected += 1;
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if app.selected > 0 {
                                app.selected -= 1;
                            }
                        }
                        KeyCode::Enter => {
                            app.mode = AppMode::ContextMenu;
                            app.menu_selected = 0;
                        }
                        _ => {}
                    },
                    AppMode::ContextMenu => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.mode = AppMode::Normal,
                        KeyCode::Down => {
                            if app.menu_selected + 1 < app.menu_items.len() {
                                app.menu_selected += 1;
                            } else {
                                app.menu_selected = 0;
                            }
                        }
                        KeyCode::Up => {
                            if app.menu_selected > 0 {
                                app.menu_selected -= 1;
                            } else {
                                app.menu_selected = app.menu_items.len() - 1;
                            }
                        }
                        KeyCode::Enter => match app.menu_selected {
                            0 => {
                                app.mode = AppMode::Logs;
                                app.logs = vec!["Loading logs...".to_string()];
                                app.log_state.select(Some(0));

                                terminal.draw(|f| {
                                    draw_ui(f, &app);
                                })?;

                                let container_id = app.container_data[app.selected].0.clone();
                                drop(app);
                                let logs = fetch_logs(&container_id)
                                    .await
                                    .unwrap_or_else(|_| vec!["Failed to load logs.".into()]);
                                let mut app = app_state.write().await;
                                let app_logs = logs.clone();
                                app.logs = logs;
                                app.log_state.select(Some(app_logs.len().saturating_sub(1)));
                            }
                            1 => {
                                app.mode = AppMode::Normal;
                            }
                            _ => {}
                        },
                        _ => {}
                    },
                    AppMode::Logs => match key.code {
                        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                            app.mode = AppMode::Normal
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            let selected = app.log_state.selected();
                            if selected.unwrap_or(0) + 1 < app.logs.len() {
                                app.log_state.select(Some(selected.unwrap_or(0) + 1));
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            let selected = app.log_state.selected();
                            if selected.unwrap_or(0) > 0 {
                                app.log_state.select(Some(selected.unwrap_or(0) - 1));
                            }
                        }
                        _ => {}
                    },
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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

    let log_list = Paragraph::new(log_spans)
        .block(Block::default().title("Logs").borders(Borders::ALL))
        .scroll((
            app_state.log_state.selected().unwrap_or(0) as u16,
            app_state.horizontal_scroll,
        ));

    let overlay_area = centered_rect(80, 80, area);
    f.render_widget(Clear, overlay_area);
    f.render_widget(log_list, overlay_area);
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

    let widths = [
        Constraint::Min(10),
        Constraint::Min(10),
        Constraint::Min(10),
        Constraint::Min(10),
        Constraint::Min(10),
    ];

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

async fn fetch_logs(container_id: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let docker = Docker::new();
    let docker = docker.clone();
    let mut log_stream = docker
        .containers()
        .get(container_id)
        .logs(&LogsOptions::builder().stdout(true).stderr(true).build());

    let mut logs = Vec::new();
    while let Some(chunk) = log_stream.next().await {
        match chunk {
            Ok(data) => {
                let cleaned = strip(&*data);
                logs.push(String::from_utf8_lossy(&cleaned).to_string());
                if logs.len() > 100 {
                    logs.remove(0);
                }
            }
            Err(_) => break,
        }
    }
    Ok(logs)
}
