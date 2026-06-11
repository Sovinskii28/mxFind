use std::io::{self, Stdout};

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use sqlx::SqlitePool;

use crate::db::search_rooms;
use crate::models::{Room, RoomHealth, RoomStatus, ServerHealth, ServerStatus};
use crate::room_status::check_rooms_status;
use crate::server_status::check_servers_status;

const TUI_RESULT_LIMIT: usize = 50;

pub struct AppState {
    pub query: String,
    pub selected: usize,
    results_offset: usize,
    details_scroll: u16,
    focused_panel: FocusedPanel,
    pub should_quit: bool,
    results: Vec<Room>,
    server_statuses: Vec<ServerHealth>,
    room_statuses: std::collections::HashMap<String, RoomHealth>,
    has_searched: bool,
    message: Option<String>,
}

impl AppState {
    fn new(server_statuses: Vec<ServerHealth>) -> Self {
        Self {
            query: String::new(),
            selected: 0,
            results_offset: 0,
            details_scroll: 0,
            focused_panel: FocusedPanel::Results,
            should_quit: false,
            results: Vec::new(),
            server_statuses,
            room_statuses: std::collections::HashMap::new(),
            has_searched: false,
            message: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedPanel {
    Results,
    Details,
}

pub async fn run(pool: SqlitePool, servers: Vec<String>) -> anyhow::Result<()> {
    let server_statuses = check_servers_status(servers.clone()).await;

    enable_raw_mode()?;
    let _guard = TerminalGuard;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_loop(&mut terminal, &pool, servers, server_statuses).await
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
    }
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    pool: &SqlitePool,
    servers: Vec<String>,
    server_statuses: Vec<ServerHealth>,
) -> anyhow::Result<()> {
    let mut state = AppState::new(server_statuses);

    while !state.should_quit {
        // Draw the full TUI frame: layout, server status, search, help, results, and details.
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(area);
            let title = Paragraph::new("mxFind")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            let servers = servers_list(&state);
            let search = Paragraph::new(format!("Search: {}", state.query))
                .block(Block::default().borders(Borders::ALL));
            let help_text = match &state.message {
                Some(message) => {
                    format!(
                        "Type to search | Enter search | r refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit | {message}"
                    )
                }
                None => {
                    "Type to search | Enter search | r refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit"
                        .to_string()
                }
            };
            let help = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[4]);
            let results = results_list(&state);
            let mut results_state = ListState::default()
                .with_offset(state.results_offset)
                .with_selected((!state.results.is_empty()).then_some(state.selected));
            let details = room_details(&state);

            frame.render_widget(title, chunks[0]);
            frame.render_widget(servers, chunks[1]);
            frame.render_widget(search, chunks[2]);
            frame.render_widget(help, chunks[3]);
            frame.render_stateful_widget(results, content_chunks[0], &mut results_state);
            frame.render_widget(details, content_chunks[1]);
            state.results_offset = results_state.offset();
            // End of full TUI frame drawing.
        })?;

        match event::read()? {
            Event::Key(key) => handle_key(&mut state, key.code, pool, &servers).await?,
            Event::Resize(_, _) => terminal.clear()?,
            _ => {}
        }
    }

    Ok(())
}

async fn handle_key(
    state: &mut AppState,
    code: KeyCode,
    pool: &SqlitePool,
    servers: &[String],
) -> anyhow::Result<()> {
    match code {
        KeyCode::Esc => state.should_quit = true,
        KeyCode::Right => state.focused_panel = FocusedPanel::Details,
        KeyCode::Left => state.focused_panel = FocusedPanel::Results,
        KeyCode::Down => {
            if state.focused_panel == FocusedPanel::Results {
                if state.selected + 1 < state.results.len() {
                    state.selected += 1;
                    state.details_scroll = 0;
                }
            } else {
                state.details_scroll = state.details_scroll.saturating_add(1);
            }
        }
        KeyCode::Up => {
            if state.focused_panel == FocusedPanel::Results {
                if state.selected > 0 {
                    state.selected -= 1;
                    state.details_scroll = 0;
                }
            } else {
                state.details_scroll = state.details_scroll.saturating_sub(1);
            }
        }
        KeyCode::PageDown => {
            if state.focused_panel == FocusedPanel::Details {
                state.details_scroll = state.details_scroll.saturating_add(5);
            }
        }
        KeyCode::PageUp => {
            if state.focused_panel == FocusedPanel::Details {
                state.details_scroll = state.details_scroll.saturating_sub(5);
            }
        }
        KeyCode::Enter => {
            state.results = search_rooms(pool, &state.query, TUI_RESULT_LIMIT).await?;
            state.server_statuses =
                check_servers_status(tui_servers_to_check(servers, &state.results)).await;
            state.room_statuses = check_rooms_status(&state.results).await;
            state.selected = 0;
            state.results_offset = 0;
            state.details_scroll = 0;
            state.focused_panel = FocusedPanel::Results;
            state.has_searched = true;
            state.message = Some("Search submitted".to_string());
        }
        KeyCode::Backspace => {
            state.query.pop();
            state.message = None;
        }
        KeyCode::Char('q') if state.query.is_empty() => state.should_quit = true,
        KeyCode::Char('r') if state.query.is_empty() => {
            state.message = Some("Refreshing statuses...".to_string());
            state.server_statuses =
                check_servers_status(tui_servers_to_check(servers, &state.results)).await;
            state.room_statuses = check_rooms_status(&state.results).await;
            state.message = Some("Statuses refreshed".to_string());
        }
        KeyCode::Char(character) => {
            state.query.push(character);
            state.message = None;
        }
        _ => {}
    }

    Ok(())
}

fn servers_list(state: &AppState) -> List<'_> {
    if state.server_statuses.is_empty() {
        return List::new(vec![ListItem::new("No servers configured")])
            .block(Block::default().borders(Borders::ALL).title("Servers"));
    }

    let items = state
        .server_statuses
        .iter()
        .map(|health| {
            let latency = health
                .latency_ms
                .map(|latency| format!("{latency}ms"))
                .unwrap_or_else(|| "-".to_string());
            let public_rooms = if health.public_rooms_available {
                "rooms:yes"
            } else {
                "rooms:no"
            };
            let reason = health
                .reason
                .as_deref()
                .map(|reason| format!(" | {reason}"))
                .unwrap_or_default();

            ListItem::new(format!(
                "{:<22} {:<10} {:>8}  {public_rooms}{reason}",
                health.server,
                server_status_label(health.status),
                latency
            ))
            .style(Style::default().fg(server_status_color(health.status)))
        })
        .collect::<Vec<_>>();

    List::new(items).block(Block::default().borders(Borders::ALL).title("Servers"))
}

fn results_list(state: &AppState) -> List<'_> {
    if !state.has_searched {
        return List::new(Vec::<ListItem>::new()).block(panel_block(
            "Results",
            state.focused_panel == FocusedPanel::Results,
        ));
    }

    if state.results.is_empty() {
        return List::new(vec![ListItem::new("No results")]).block(panel_block(
            "Results",
            state.focused_panel == FocusedPanel::Results,
        ));
    }

    let items = state
        .results
        .iter()
        .map(|room| {
            let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);
            let name = room.name.as_deref().unwrap_or("No name");
            let members = room
                .num_joined_members
                .map(|members| members.to_string())
                .unwrap_or_else(|| "?".to_string());
            let room_status = room_status_label_for_room(state, &room.room_id);

            ListItem::new(format!(
                "{id}\n{name} | members: {members} | server: {} | room: {room_status}",
                room.server,
            ))
        })
        .collect::<Vec<_>>();

    List::new(items)
        .block(panel_block(
            "Results",
            state.focused_panel == FocusedPanel::Results,
        ))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
}

fn room_details(state: &AppState) -> Paragraph<'_> {
    let Some(room) = state.results.get(state.selected) else {
        return Paragraph::new("Select a room").block(panel_block(
            "Details",
            state.focused_panel == FocusedPanel::Details,
        ));
    };

    let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);
    let name = room.name.as_deref().unwrap_or("No name");
    let topic = room.topic.as_deref().unwrap_or("No topic");
    let members = room
        .num_joined_members
        .map(|members| members.to_string())
        .unwrap_or_else(|| "?".to_string());
    let room_status = room_status_label_for_room(state, &room.room_id);

    Paragraph::new(format!(
        "{id}\n\nname: {name}\n\n\
         room status: {room_status}\n\
         topic: {topic}\n\n\
         members: {members}\n\
         server: {}\n\
         matrix.to: {}",
        room.server,
        room.matrix_to_url()
    ))
    .block(panel_block(
        "Details",
        state.focused_panel == FocusedPanel::Details,
    ))
    .scroll((state.details_scroll, 0))
    .wrap(Wrap { trim: true })
}

fn panel_block(title: &'static str, focused: bool) -> Block<'static> {
    let block = Block::default().borders(Borders::ALL).title(title);

    if focused {
        block.border_style(Style::default().fg(Color::Cyan))
    } else {
        block
    }
}

fn tui_servers_to_check(base_servers: &[String], rooms: &[Room]) -> Vec<String> {
    let mut servers = base_servers.to_vec();
    servers.extend(rooms.iter().map(|room| room.server.clone()));
    servers
}

fn room_status_label_for_room<'a>(state: &'a AppState, room_id: &str) -> &'a str {
    state
        .room_statuses
        .get(room_id)
        .map(|health| room_status_label(health.status))
        .unwrap_or("not checked")
}

fn server_status_label(status: ServerStatus) -> &'static str {
    match status {
        ServerStatus::Online => "online",
        ServerStatus::Offline => "offline",
        ServerStatus::Restricted => "restricted",
        ServerStatus::Unknown => "unknown",
    }
}

fn server_status_color(status: ServerStatus) -> Color {
    match status {
        ServerStatus::Online => Color::Green,
        ServerStatus::Offline => Color::Red,
        ServerStatus::Restricted => Color::Yellow,
        ServerStatus::Unknown => Color::Gray,
    }
}

fn room_status_label(status: RoomStatus) -> &'static str {
    match status {
        RoomStatus::Resolvable => "resolvable",
        RoomStatus::NotFound => "not found",
        RoomStatus::NoAlias => "no alias",
        RoomStatus::Unknown => "unknown",
    }
}
