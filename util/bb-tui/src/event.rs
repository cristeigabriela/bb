use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};

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
                    if app.cursor > 0 {
                        // Find the previous char boundary.
                        let prev = app.search[..app.cursor]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i);
                        app.search.remove(prev);
                        app.cursor = prev;
                        app.rebuild();
                    }
                }
                KeyCode::Delete => {
                    if app.cursor < app.search.len() {
                        app.search.remove(app.cursor);
                        app.rebuild();
                    }
                }
                KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.cursor = word_boundary_left(&app.search, app.cursor);
                }
                KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.cursor = word_boundary_right(&app.search, app.cursor);
                }
                KeyCode::Left => {
                    if app.cursor > 0 {
                        app.cursor = app.search[..app.cursor]
                            .char_indices()
                            .next_back()
                            .map_or(0, |(i, _)| i);
                    }
                }
                KeyCode::Right => {
                    if app.cursor < app.search.len() {
                        app.cursor += app.search[app.cursor..]
                            .chars()
                            .next()
                            .map_or(0, char::len_utf8);
                    }
                }
                KeyCode::Home => {
                    app.cursor = 0;
                }
                KeyCode::End => {
                    app.cursor = app.search.len();
                }
                KeyCode::Char(c) => {
                    app.search.insert(app.cursor, c);
                    app.cursor += c.len_utf8();
                    app.rebuild();
                }
                _ => {}
            },

            Focus::Tree => match key.code {
                KeyCode::Char('q') => break Ok(()),
                KeyCode::Char('/') => {
                    app.cursor = app.search.len();
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
                    app.cursor = app.search.len();
                    app.focus = Focus::Search;
                }
                KeyCode::Tab => {
                    app.focus = Focus::Tree;
                }
                KeyCode::Char('J') => app.scroll_down(10),
                KeyCode::Char('K') => app.scroll_up(10),
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

/* ───────────────────────────────── Helpers ──────────────────────────────── */

/// Move cursor left to the start of the previous word.
///
/// Skips trailing non-alphanumeric chars, then skips the word itself.
fn word_boundary_left(s: &str, cursor: usize) -> usize {
    let left = &s[..cursor];
    let mut chars = left.char_indices().rev();

    // Skip non-word characters (separators like `_`, spaces, punctuation).
    let mut pos = cursor;
    for (i, c) in chars.by_ref() {
        if c.is_alphanumeric() {
            pos = i;
            break;
        }
        pos = i;
    }

    // Skip word characters.
    for (i, c) in chars {
        if !c.is_alphanumeric() {
            return i + c.len_utf8();
        }
        pos = i;
    }

    pos
}

/// Move cursor right to the end of the next word.
///
/// Skips leading non-alphanumeric chars, then skips the word itself.
fn word_boundary_right(s: &str, cursor: usize) -> usize {
    let right = &s[cursor..];
    let mut chars = right.char_indices();

    // Skip non-word characters.
    let mut pos = cursor;
    for (i, c) in chars.by_ref() {
        if c.is_alphanumeric() {
            pos = cursor + i;
            break;
        }
        pos = cursor + i + c.len_utf8();
    }

    // Skip word characters.
    for (i, c) in chars {
        if !c.is_alphanumeric() {
            return cursor + i;
        }
        pos = cursor + i + c.len_utf8();
    }

    pos
}
