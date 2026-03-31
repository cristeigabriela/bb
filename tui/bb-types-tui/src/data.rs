use std::collections::BTreeMap;

use bb_clang::{Field, Struct};
use bb_shared::glob_match;
use bb_tui::{FileEntry, TuiData, matches_file};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/* ────────────────────────────────── Types ───────────────────────────────── */

enum Row<'a> {
    StructHeader(&'a Struct<'a>),
    Field { field: &'a Field<'a>, is_last: bool },
    Footer(usize),
    Blank,
}

pub struct TypeData<'a> {
    structs: &'a [Struct<'a>],
    rows: Vec<Row<'a>>,
    files: Vec<FileEntry>,
}

impl<'a> TypeData<'a> {
    pub fn new(structs: &'a [Struct<'a>]) -> Self {
        Self {
            structs,
            rows: Vec::new(),
            files: vec![FileEntry {
                name: "(all)".to_string(),
                count: 0,
            }],
        }
    }
}

impl TuiData for TypeData<'_> {
    fn title(&self) -> &'static str {
        "Structs"
    }

    fn files(&self) -> &[FileEntry] {
        &self.files
    }

    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn render_row(&self, index: usize) -> Line<'static> {
        render_row(&self.rows[index])
    }

    fn rebuild_index(&mut self, search: Option<&str>) {
        self.files = build_file_index(self.structs, search);
    }

    fn rebuild_rows(&mut self, search: Option<&str>, file_filter: Option<&str>) {
        self.rows.clear();

        for s in self.structs {
            if !matches_file(
                s.get_location().and_then(|l| l.file.as_deref()),
                file_filter,
            ) {
                continue;
            }

            let name_matches = match search {
                Some(pat) => glob_match(s.get_name(), pat, false),
                None => true,
            };

            if !name_matches {
                continue;
            }

            self.rows.push(Row::StructHeader(s));

            let fields = s.get_fields();
            let count = fields.len();
            for (i, f) in fields.iter().enumerate() {
                self.rows.push(Row::Field {
                    field: f,
                    is_last: i == count - 1,
                });
            }

            self.rows.push(Row::Footer(count));
            self.rows.push(Row::Blank);
        }
    }
}

/* ──────────────────────────────── Rendering ─────────────────────────────── */

fn render_row(row: &Row) -> Line<'static> {
    match row {
        Row::StructHeader(s) => {
            let mut spans = vec![Span::styled(
                s.get_name().to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];

            if let Some(size) = s.get_size() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("{size} bytes"),
                    Style::default().fg(Color::Cyan),
                ));
            }

            if let Some(loc) = s.get_location() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    loc.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }

        Row::Field { field, is_last } => {
            let connector = if *is_last {
                "  \u{2570}\u{2500} "
            } else {
                "  \u{251C}\u{2500} "
            };

            let mut spans = vec![
                Span::styled(connector.to_string(), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("+0x{:04X}", field.get_offset_bytes()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("  "),
                Span::styled(
                    field.get_name().to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            if let Some(ty) = field.get_type_name() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    ty.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{} bytes", field.get_size()),
                Style::default().fg(Color::DarkGray),
            ));

            Line::from(spans)
        }

        Row::Footer(count) => Line::from(Span::styled(
            format!("  \u{2570}\u{2500} {count} fields"),
            Style::default().fg(Color::DarkGray),
        )),

        Row::Blank => Line::raw(""),
    }
}

/* ─────────────────────────────── File index ─────────────────────────────── */

fn build_file_index(structs: &[Struct], pattern: Option<&str>) -> Vec<FileEntry> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;

    for s in structs {
        let name_matches = match pattern {
            Some(pat) => glob_match(s.get_name(), pat, false),
            None => true,
        };

        if !name_matches {
            continue;
        }

        if let Some(file) = s.get_location().and_then(|l| l.file.as_deref()) {
            *counts.entry(file.to_string()).or_default() += 1;
            total += 1;
        }
    }

    let mut files = vec![FileEntry {
        name: "(all)".to_string(),
        count: total,
    }];

    for (name, count) in counts {
        files.push(FileEntry { name, count });
    }

    files
}
