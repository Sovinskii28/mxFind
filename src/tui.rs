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
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Terminal;
use sqlx::SqlitePool;

use crate::db::search_rooms;
use crate::models::Room;

const TUI_RESULT_LIMIT: usize = 50;

pub struct AppState {
    pub query: String,
    pub selected: usize,
    pub should_quit: bool,
    results: Vec<Room>,
    has_searched: bool,
    message: Option<String>,
}

impl AppState {
    fn new() -> Self {
        Self {
            query: String::new(),
            selected: 0,
            should_quit: false,
            results: Vec::new(),
            has_searched: false,
            message: None,
        }
    }
}

pub async fn run(pool: SqlitePool) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let _guard = TerminalGuard;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run_loop(&mut terminal, &pool).await
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
) -> anyhow::Result<()> {
    let mut state = AppState::new();

    while !state.should_quit {
        terminal.draw(|frame| {
            let area = frame.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ])
                .split(area);
            let title = Paragraph::new("mxfind TUI")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            let search = Paragraph::new(format!("Search: {}", state.query))
                .block(Block::default().borders(Borders::ALL));
            let help_text = match &state.message {
                Some(message) => {
                    format!("Type to search | Enter search | ↑/↓ select | Esc quit | {message}")
                }
                None => "Type to search | Enter search | ↑/↓ select | Esc quit".to_string(),
            };
            let help = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[3]);
            let results = results_list(&state);
            let details = room_details(&state);

            frame.render_widget(title, chunks[0]);
            frame.render_widget(search, chunks[1]);
            frame.render_widget(help, chunks[2]);
            frame.render_widget(results, content_chunks[0]);
            frame.render_widget(details, content_chunks[1]);
        })?;

        match event::read()? {
            Event::Key(key) => handle_key(&mut state, key.code, pool).await?,
            Event::Resize(_, _) => terminal.clear()?,
            _ => {}
        }
    }

    Ok(())
}

async fn handle_key(state: &mut AppState, code: KeyCode, pool: &SqlitePool) -> anyhow::Result<()> {
    match code {
        KeyCode::Esc => state.should_quit = true,
        KeyCode::Down => {
            if state.selected + 1 < state.results.len() {
                state.selected += 1;
            }
        }
        KeyCode::Up => {
            if state.selected > 0 {
                state.selected -= 1;
            }
        }
        KeyCode::Enter => {
            state.results = search_rooms(pool, &state.query, TUI_RESULT_LIMIT).await?;
            state.selected = 0;
            state.has_searched = true;
            state.message = Some("Search submitted".to_string());
        }
        KeyCode::Backspace => {
            state.query.pop();
            state.message = None;
        }
        KeyCode::Char('q') if state.query.is_empty() => state.should_quit = true,
        KeyCode::Char(character) => {
            state.query.push(character);
            state.message = None;
        }
        _ => {}
    }

    Ok(())
}

fn results_list(state: &AppState) -> List<'_> {
    if !state.has_searched {
        return List::new(Vec::<ListItem>::new())
            .block(Block::default().borders(Borders::ALL).title("Results"));
    }

    if state.results.is_empty() {
        return List::new(vec![ListItem::new("No results")])
            .block(Block::default().borders(Borders::ALL).title("Results"));
    }

    let items = state
        .results
        .iter()
        .enumerate()
        .map(|(index, room)| {
            let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);
            let name = room.name.as_deref().unwrap_or("No name");
            let members = room
                .num_joined_members
                .map(|members| members.to_string())
                .unwrap_or_else(|| "?".to_string());

            let item = ListItem::new(format!(
                "{id}\n{name} | members: {members} | server: {}",
                room.server
            ));

            if index == state.selected {
                item.style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                item
            }
        })
        .collect::<Vec<_>>();

    List::new(items).block(Block::default().borders(Borders::ALL).title("Results"))
}

fn room_details(state: &AppState) -> Paragraph<'_> {
    let Some(room) = state.results.get(state.selected) else {
        return Paragraph::new("Select a room")
            .block(Block::default().borders(Borders::ALL).title("Details"));
    };

    let id = room.canonical_alias.as_deref().unwrap_or(&room.room_id);
    let name = room.name.as_deref().unwrap_or("No name");
    let topic = room.topic.as_deref().unwrap_or("No topic");
    let members = room
        .num_joined_members
        .map(|members| members.to_string())
        .unwrap_or_else(|| "?".to_string());

    Paragraph::new(format!(
        "{id}\n\nname: {name}\n\n\
         topic: {topic}\n\n\
         members: {members}\n\
         server: {}\n\
         matrix.to: {}",
        room.server,
        room.matrix_to_url()
    ))
    .block(Block::default().borders(Borders::ALL).title("Details"))
    .wrap(Wrap { trim: true })
}
