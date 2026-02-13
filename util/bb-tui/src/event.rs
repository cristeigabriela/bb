use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};

use crate::{App, Focus, TuiData};

/* ──────────────────────────────── TUI loop ──────────────────────────────── */

/// The main event loop for the TUI. It is responsible to catch input events
/// that help you interact with the application.
///
/// Polls every 100ms.
pub fn run<D: TuiData>(app: &mut App<D>) -> Result<()> {
    let mut terminal = ratatui::init();

    let result = loop {
        terminal.draw(|frame| crate::ui::draw(frame, app))?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        match app.focus {
            Focus::Search => match key.code {
                KeyCode::Esc => {
                    app.focus = Focus::Content;
                }
                KeyCode::Enter => {
                    app.rebuild();
                    app.focus = Focus::Content;
                }
                KeyCode::Backspace => {
                    app.search.pop();
                    app.rebuild();
                }
                KeyCode::Char(c) => {
                    app.search.push(c);
                    app.rebuild();
                }
                _ => {}
            },

            Focus::Tree => match key.code {
                KeyCode::Char('q') => break Ok(()),
                KeyCode::Char('/') => {
                    app.focus = Focus::Search;
                }
                KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => {
                    app.focus = Focus::Content;
                }
                KeyCode::Down | KeyCode::Char('j') => app.tree_down(),
                KeyCode::Up | KeyCode::Char('k') => app.tree_up(),
                KeyCode::Home => {
                    app.file_cursor = 0;
                    app.scroll = 0;
                    app.rebuild();
                }
                KeyCode::End => {
                    app.file_cursor = app.data.files().len().saturating_sub(1);
                    app.scroll = 0;
                    app.rebuild();
                }
                _ => {}
            },

            Focus::Content => match key.code {
                KeyCode::Char('q') => break Ok(()),
                KeyCode::Char('/') => {
                    app.focus = Focus::Search;
                }
                KeyCode::Tab => {
                    app.focus = Focus::Tree;
                }
                KeyCode::Down | KeyCode::Char('j') => app.scroll_down(1),
                KeyCode::Up | KeyCode::Char('k') => app.scroll_up(1),
                KeyCode::Left | KeyCode::Char('h') => app.scroll_left(4),
                KeyCode::Right | KeyCode::Char('l') => app.scroll_right(4),
                KeyCode::PageDown => app.scroll_down(20),
                KeyCode::PageUp => app.scroll_up(20),
                KeyCode::Home => app.scroll = 0,
                KeyCode::End => {
                    app.scroll = app.data.row_count().saturating_sub(1);
                }
                _ => {}
            },
        }
    };

    ratatui::restore();
    result
}
