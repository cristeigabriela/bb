use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::{App, Focus, TuiData};

/* ──────────────────────────────── Rendering ─────────────────────────────── */

/// The basic layout of the TUI described in [`bb-tui`].
pub fn draw<D: TuiData>(frame: &mut Frame, app: &App<D>) {
    let [search_area, main_area, status_area] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    let [tree_area, list_area] =
        Layout::horizontal([Constraint::Length(tree_width(app)), Constraint::Fill(1)])
            .areas(main_area);

    draw_search(frame, app, search_area);
    draw_tree(frame, app, tree_area);
    draw_content(frame, app, list_area);
    draw_status(frame, app, status_area);

    if app.focus == Focus::Search {
        frame.set_cursor_position((
            search_area.x + 1 + app.search.len() as u16,
            search_area.y + 1,
        ));
    }
}

/// The searchbar.
fn draw_search<D: TuiData>(frame: &mut Frame, app: &App<D>, area: ratatui::layout::Rect) {
    let style = if app.focus == Focus::Search {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let input = Paragraph::new(app.search.as_str())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(style)
                .title(" Search "),
        );

    frame.render_widget(input, area);
}

fn draw_tree<D: TuiData>(frame: &mut Frame, app: &App<D>, area: ratatui::layout::Rect) {
    let border_style = if app.focus == Focus::Tree {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let files = app.data.files();

    let tree_scroll = if app.file_cursor >= visible_height {
        app.file_cursor - visible_height + 1
    } else {
        0
    };

    let end = (tree_scroll + visible_height).min(files.len());
    let visible = &files[tree_scroll..end];

    let lines: Vec<Line> = visible
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let idx = tree_scroll + i;
            let is_selected = idx == app.file_cursor;

            let count_str = format!(" ({})", entry.count);
            let mut spans = vec![];

            if is_selected {
                spans.push(Span::styled(
                    entry.name.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(count_str, Style::default().fg(Color::Yellow)));
            } else {
                spans.push(Span::styled(
                    entry.name.clone(),
                    Style::default().fg(Color::Gray),
                ));
                spans.push(Span::styled(
                    count_str,
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        })
        .collect();

    let tree = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Files "),
    );

    frame.render_widget(tree, area);
}

/// The content area.
fn draw_content<D: TuiData>(frame: &mut Frame, app: &App<D>, area: ratatui::layout::Rect) {
    let border_style = if app.focus == Focus::Content {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let row_count = app.data.row_count();

    let end = (app.scroll + visible_height).min(row_count);
    let lines: Vec<Line> = (app.scroll..end).map(|i| app.data.render_row(i)).collect();

    let title = match app.selected_file() {
        Some(f) => format!(" {f} "),
        None => format!(" {} ", app.data.title()),
    };

    let list = Paragraph::new(lines).scroll((0, app.hscroll as u16)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title),
    );

    frame.render_widget(list, area);
}

/// The status bar. Prints helpful information for the current [`Focus`].
fn draw_status<D: TuiData>(frame: &mut Frame, app: &App<D>, area: ratatui::layout::Rect) {
    let hints = if app.focus == Focus::Search {
        vec![
            Span::styled("Enter", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" apply  "),
            Span::styled("Esc", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ]
    } else {
        let mut h = vec![
            Span::styled("/", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" search  "),
            Span::styled("Tab", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" switch pane  "),
            Span::styled("j/k", Style::default().add_modifier(Modifier::BOLD)),
        ];

        // What j/k will do in this context.
        if app.focus == Focus::Tree {
            h.push(Span::raw(" select file  "));
        } else {
            h.push(Span::raw(" scroll  "));
        }

        // h/l only works on content focus
        if app.focus == Focus::Content {
            h.push(Span::styled(
                "h/l",
                Style::default().add_modifier(Modifier::BOLD),
            ));
            h.push(Span::raw(" pan  "));
        }

        // Extend with the remainder of statics.
        h.extend(vec![
            Span::styled("q", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(" quit  "),
            Span::styled(
                format!(
                    " {} files  {} results",
                    app.data.files().len().saturating_sub(1),
                    app.data.row_count()
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]);

        h
    };

    let status = Paragraph::new(Line::from(hints));
    frame.render_widget(status, area);
}

/* ──────────────────────────────── Utilities ─────────────────────────────── */

fn tree_width<D: TuiData>(app: &App<D>) -> u16 {
    let max_name = app
        .data
        .files()
        .iter()
        .map(|f| f.name.len())
        .max()
        .unwrap_or(5);
    (max_name + 10).min(40) as u16
}
