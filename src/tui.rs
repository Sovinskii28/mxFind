use std::io::{self, Stdout};
use std::time::Duration;

use crossterm::cursor::Show;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::stream::{FuturesUnordered, StreamExt};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Terminal;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::db::search_rooms;
use crate::matrix::{fetch_public_rooms, fetch_public_rooms_search, PublicRoomsErrorKind};
use crate::models::{Room, RoomHealth, RoomStatus, ServerHealth, ServerStatus};
use crate::room_status::check_rooms_status;
use crate::search::{dedup_rooms, filter_rooms};
use crate::server_status::check_servers_status;

const TUI_RESULT_LIMIT: usize = 50;
const TUI_TICK_RATE: Duration = Duration::from_millis(120);
const SPINNER_FRAMES: [&str; 4] = ["|", "/", "-", "\\"];

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
    search_mode: SearchMode,
    has_searched: bool,
    message: Option<String>,
    is_loading: bool,
    loading_label: Option<String>,
    status_loading: bool,
    spinner_frame: usize,
    next_search_id: u64,
    active_search_id: Option<u64>,
    active_status_id: Option<u64>,
}

impl AppState {
    fn new(server_statuses: Vec<ServerHealth>, search_mode: SearchMode) -> Self {
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
            search_mode,
            has_searched: false,
            message: None,
            is_loading: false,
            loading_label: None,
            status_loading: false,
            spinner_frame: 0,
            next_search_id: 0,
            active_search_id: None,
            active_status_id: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FocusedPanel {
    Results,
    Details,
}

#[derive(Clone, Copy)]
enum SearchMode {
    Live,
    Local,
}

struct SearchOutcome {
    id: u64,
    query: String,
    result: anyhow::Result<Vec<Room>>,
}

struct StatusOutcome {
    id: u64,
    server_statuses: Vec<ServerHealth>,
    room_statuses: std::collections::HashMap<String, RoomHealth>,
}

enum TuiOutcome {
    Search(SearchOutcome),
    Status(StatusOutcome),
}

pub async fn run_live(servers: Vec<String>) -> anyhow::Result<()> {
    let server_statuses = check_servers_status(servers.clone()).await;

    enable_raw_mode()?;
    let _guard = TerminalGuard;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_loop(&mut terminal, SearchBackend::Live, servers, server_statuses).await
}

pub async fn run_local(pool: SqlitePool, servers: Vec<String>) -> anyhow::Result<()> {
    let server_statuses = check_servers_status(servers.clone()).await;

    enable_raw_mode()?;
    let _guard = TerminalGuard;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_loop(
        &mut terminal,
        SearchBackend::Local(pool),
        servers,
        server_statuses,
    )
    .await
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
    backend: SearchBackend,
    servers: Vec<String>,
    server_statuses: Vec<ServerHealth>,
) -> anyhow::Result<()> {
    let mut state = AppState::new(server_statuses, backend.search_mode());
    let (task_tx, mut task_rx) = mpsc::unbounded_channel();

    while !state.should_quit {
        while let Ok(outcome) = task_rx.try_recv() {
            apply_tui_outcome(&mut state, outcome, &servers, &task_tx);
        }

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
            let search = Paragraph::new(format!(
                "Search [{}]: {}",
                search_mode_label(state.search_mode),
                state.query
            ))
            .block(Block::default().borders(Borders::ALL));
            let help_text = help_text(&state);
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

        if event::poll(TUI_TICK_RATE)? {
            match event::read()? {
                Event::Key(key) => {
                    handle_key(&mut state, key, &backend, &servers, &task_tx).await?
                }
                Event::Resize(_, _) => terminal.clear()?,
                _ => {}
            }
        } else if state.is_loading || state.status_loading {
            state.spinner_frame = state.spinner_frame.wrapping_add(1);
        }
    }

    Ok(())
}

async fn handle_key(
    state: &mut AppState,
    key: KeyEvent,
    backend: &SearchBackend,
    servers: &[String],
    task_tx: &mpsc::UnboundedSender<TuiOutcome>,
) -> anyhow::Result<()> {
    match key.code {
        KeyCode::Esc => state.should_quit = true,
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.should_quit = true;
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            submit_status_refresh(state, servers, task_tx);
        }
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
            if state.query.trim().is_empty() {
                submit_status_refresh(state, servers, task_tx);
            } else {
                submit_search(state, backend, servers, task_tx);
            }
        }
        KeyCode::Backspace => {
            state.query.pop();
            state.message = None;
        }
        KeyCode::Char(character) => {
            state.query.push(character);
            state.message = None;
        }
        _ => {}
    }

    Ok(())
}

#[derive(Clone)]
enum SearchBackend {
    Live,
    Local(SqlitePool),
}

impl SearchBackend {
    fn search_mode(&self) -> SearchMode {
        match self {
            Self::Live => SearchMode::Live,
            Self::Local(_) => SearchMode::Local,
        }
    }
}

fn submit_search(
    state: &mut AppState,
    backend: &SearchBackend,
    servers: &[String],
    task_tx: &mpsc::UnboundedSender<TuiOutcome>,
) {
    let query = state.query.trim().to_string();

    state.next_search_id = state.next_search_id.wrapping_add(1);
    let id = state.next_search_id;
    state.active_search_id = Some(id);
    state.active_status_id = None;
    state.status_loading = false;
    state.is_loading = true;
    state.loading_label = Some(format!("Searching for \"{query}\"..."));
    state.spinner_frame = 0;
    state.has_searched = true;
    state.message = None;

    let backend = backend.clone();
    let servers = servers.to_vec();
    let sender = task_tx.clone();

    tokio::spawn(async move {
        let result = search_tui_rooms(&backend, &servers, &query).await;
        let _ = sender.send(TuiOutcome::Search(SearchOutcome { id, query, result }));
    });
}

fn submit_status_refresh(
    state: &mut AppState,
    servers: &[String],
    task_tx: &mpsc::UnboundedSender<TuiOutcome>,
) {
    state.next_search_id = state.next_search_id.wrapping_add(1);
    let id = state.next_search_id;
    state.active_status_id = Some(id);
    state.status_loading = true;
    state.spinner_frame = 0;
    state.message = Some("Refreshing statuses...".to_string());

    let servers_to_check = tui_servers_to_check(servers, &state.results);
    let rooms = state.results.clone();
    let sender = task_tx.clone();

    tokio::spawn(async move {
        let server_statuses = check_servers_status(servers_to_check).await;
        let room_statuses = check_rooms_status(&rooms).await;
        let _ = sender.send(TuiOutcome::Status(StatusOutcome {
            id,
            server_statuses,
            room_statuses,
        }));
    });
}

fn apply_tui_outcome(
    state: &mut AppState,
    outcome: TuiOutcome,
    servers: &[String],
    task_tx: &mpsc::UnboundedSender<TuiOutcome>,
) {
    match outcome {
        TuiOutcome::Search(outcome) => {
            if apply_search_outcome(state, outcome) {
                submit_status_refresh(state, servers, task_tx);
            }
        }
        TuiOutcome::Status(outcome) => apply_status_outcome(state, outcome),
    }
}

fn apply_search_outcome(state: &mut AppState, outcome: SearchOutcome) -> bool {
    if state.active_search_id != Some(outcome.id) {
        return false;
    }

    state.active_search_id = None;
    state.is_loading = false;
    state.loading_label = None;

    match outcome.result {
        Ok(results) => {
            state.results = results;
            state.room_statuses.clear();
            state.selected = 0;
            state.results_offset = 0;
            state.details_scroll = 0;
            state.focused_panel = FocusedPanel::Results;
            state.message = Some(format!("Search completed: {}", outcome.query));
            true
        }
        Err(error) => {
            state.message = Some(format!("Search failed: {error}"));
            false
        }
    }
}

fn apply_status_outcome(state: &mut AppState, outcome: StatusOutcome) {
    if state.active_status_id != Some(outcome.id) {
        return;
    }

    state.active_status_id = None;
    state.status_loading = false;
    state.server_statuses = outcome.server_statuses;
    state.room_statuses = outcome.room_statuses;
    state.message = Some("Statuses refreshed".to_string());
}

async fn search_tui_rooms(
    backend: &SearchBackend,
    servers: &[String],
    query: &str,
) -> anyhow::Result<Vec<Room>> {
    match backend {
        SearchBackend::Live => search_live_rooms(servers, query).await,
        SearchBackend::Local(pool) => search_rooms(pool, query, TUI_RESULT_LIMIT).await,
    }
}

async fn search_live_rooms(servers: &[String], query: &str) -> anyhow::Result<Vec<Room>> {
    let mut requests = FuturesUnordered::new();
    let query = query.trim();

    for server in servers
        .iter()
        .map(|server| server.trim())
        .filter(|server| !server.is_empty())
    {
        requests.push(async move { search_server_live_rooms(server, query).await });
    }

    let mut rooms = Vec::new();

    while let Some(mut server_rooms) = requests.next().await {
        rooms.append(&mut server_rooms);
    }

    let mut rooms = filter_rooms(query, &dedup_rooms(rooms));
    rooms.sort_by(|left, right| {
        right
            .num_joined_members
            .unwrap_or(0)
            .cmp(&left.num_joined_members.unwrap_or(0))
    });
    rooms.truncate(TUI_RESULT_LIMIT);

    Ok(rooms)
}

async fn search_server_live_rooms(server: &str, query: &str) -> Vec<Room> {
    match fetch_public_rooms_search(server, query, TUI_RESULT_LIMIT).await {
        Ok(rooms) if !rooms.is_empty() => rooms,
        Ok(_) => search_server_live_rooms_fallback(server, query).await,
        Err(error)
            if matches!(
                error.kind(),
                PublicRoomsErrorKind::InvalidResponse
                    | PublicRoomsErrorKind::NetworkError
                    | PublicRoomsErrorKind::NotFound
                    | PublicRoomsErrorKind::Unauthorized
            ) =>
        {
            search_server_live_rooms_fallback(server, query).await
        }
        Err(_) => Vec::new(),
    }
}

async fn search_server_live_rooms_fallback(server: &str, query: &str) -> Vec<Room> {
    fetch_public_rooms(server)
        .await
        .map(|rooms| filter_rooms(query, &rooms))
        .unwrap_or_default()
}

fn search_mode_label(mode: SearchMode) -> &'static str {
    match mode {
        SearchMode::Live => "live",
        SearchMode::Local => "local",
    }
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

fn help_text(state: &AppState) -> String {
    if state.is_loading {
        let label = state.loading_label.as_deref().unwrap_or("Working");
        return format!(
            "Type to edit query | Enter search | Ctrl+R refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit | {} {label}",
            spinner(state)
        );
    }

    if state.status_loading {
        return format!(
            "Type to search | Enter search | Ctrl+R refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit | {} Refreshing statuses...",
            spinner(state)
        );
    }

    match &state.message {
        Some(message) => {
            format!(
                "Type to search | Enter search | Ctrl+R refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit | {message}"
            )
        }
        None => {
            "Type to search | Enter search | Ctrl+R refresh statuses | ←/→ panel | ↑/↓ scroll | Esc quit"
                .to_string()
        }
    }
}

fn results_list(state: &AppState) -> List<'_> {
    if state.is_loading {
        let label = state.loading_label.as_deref().unwrap_or("Working");
        return List::new(vec![ListItem::new(format!("{} {label}", spinner(state)))]).block(
            panel_block("Results", state.focused_panel == FocusedPanel::Results),
        );
    }

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

fn spinner(state: &AppState) -> &'static str {
    SPINNER_FRAMES[state.spinner_frame % SPINNER_FRAMES.len()]
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
         room status: {room_status}\n\n\
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
        .or_else(|| state.status_loading.then_some("checking..."))
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
