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
use crate::models::Room;

const TUI_RESULT_LIMIT: usize = 50;

pub struct AppState {
    pub query: String,
    pub selected: usize,
    results_offset: usize,
    details_scroll: u16,
    focused_panel: FocusedPanel,
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
            results_offset: 0,
            details_scroll: 0,
            focused_panel: FocusedPanel::Results,
            should_quit: false,
            results: Vec::new(),
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
            let title = Paragraph::new("mxFind")
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL));
            let search = Paragraph::new(format!("Search: {}", state.query))
                .block(Block::default().borders(Borders::ALL));
            let help_text = match &state.message {
                Some(message) => {
                    format!(
                        "Type to search | Enter search | ←/→ panel | ↑/↓ scroll | Esc quit | {message}"
                    )
                }
                None => {
                    "Type to search | Enter search | ←/→ panel | ↑/↓ scroll | Esc quit"
                        .to_string()
                }
            };
            let help = Paragraph::new(help_text).block(Block::default().borders(Borders::ALL));
            let content_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(chunks[3]);
            let results = results_list(&state);
            let mut results_state = ListState::default()
                .with_offset(state.results_offset)
                .with_selected((!state.results.is_empty()).then_some(state.selected));
            let details = room_details(&state);

            frame.render_widget(title, chunks[0]);
            frame.render_widget(search, chunks[1]);
            frame.render_widget(help, chunks[2]);
            frame.render_stateful_widget(results, content_chunks[0], &mut results_state);
            frame.render_widget(details, content_chunks[1]);
            state.results_offset = results_state.offset();
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

            ListItem::new(format!(
                "{id}\n{name} | members: {members} | server: {}",
                room.server
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

    Paragraph::new(format!(
        "{id}\n\nname: {name}\n\n\
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
