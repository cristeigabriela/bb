use std::collections::BTreeMap;

use bb_clang::{ConstLookup, Constant, Enum, MacroBodyToken};
use bb_shared::glob_match;
use bb_tui::{FileEntry, TuiData, matches_file};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/* ────────────────────────────────── Types ───────────────────────────────── */

enum Row<'a> {
    EnumHeader(&'a Enum<'a>),
    EnumConstant {
        constant: &'a Constant<'a>,
        is_last: bool,
    },
    EnumFooter(usize),
    Blank,
    Standalone(&'a Constant<'a>),
    Composition {
        constant: &'a Constant<'a>,
        nested: bool,
        parent_is_last: bool,
    },
}

pub struct ConstData<'a> {
    enums: &'a [Enum<'a>],
    vars: &'a [Constant<'a>],
    lookup: &'a ConstLookup,
    rows: Vec<Row<'a>>,
    files: Vec<FileEntry>,
}

impl<'a> ConstData<'a> {
    pub fn new(enums: &'a [Enum<'a>], vars: &'a [Constant<'a>], lookup: &'a ConstLookup) -> Self {
        Self {
            enums,
            vars,
            lookup,
            rows: Vec::new(),
            files: vec![FileEntry {
                name: "(all)".to_string(),
                count: 0,
            }],
        }
    }

    fn has_composition(&self, c: &Constant) -> bool {
        let tokens = c.get_body_tokens();
        !tokens.is_empty()
            && tokens
                .iter()
                .any(|t| t.is_identifier && self.lookup.contains_key(&t.lit_representation))
    }
}

impl TuiData for ConstData<'_> {
    fn title(&self) -> &'static str {
        "Constants"
    }

    fn files(&self) -> &[FileEntry] {
        &self.files
    }

    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn render_row(&self, index: usize) -> Line<'static> {
        render_row(&self.rows[index], self.lookup)
    }

    fn rebuild_index(&mut self, search: Option<&str>) {
        self.files = build_file_index(self.enums, self.vars, search);
    }

    fn rebuild_rows(&mut self, search: Option<&str>, file_filter: Option<&str>) {
        self.rows.clear();

        // Enums
        for e in self.enums {
            if !matches_file(
                e.get_location().and_then(|l| l.file.as_deref()),
                file_filter,
            ) {
                continue;
            }

            let matching_constants: Vec<&Constant> = match search {
                Some(pat) => e
                    .get_constants()
                    .iter()
                    .filter(|c| glob_match(c.get_name(), pat, false))
                    .collect(),
                None => e.get_constants().iter().collect(),
            };

            if matching_constants.is_empty() {
                continue;
            }

            self.rows.push(Row::EnumHeader(e));

            let count = matching_constants.len();
            for (i, c) in matching_constants.iter().enumerate() {
                let is_last = i == count - 1;
                self.rows.push(Row::EnumConstant {
                    constant: c,
                    is_last,
                });
                if self.has_composition(c) {
                    self.rows.push(Row::Composition {
                        constant: c,
                        nested: true,
                        parent_is_last: is_last,
                    });
                }
            }

            self.rows.push(Row::EnumFooter(count));
            self.rows.push(Row::Blank);
        }

        // Standalone constants
        let matching_vars: Vec<&Constant> = match search {
            Some(pat) => self
                .vars
                .iter()
                .filter(|c| {
                    matches_file(
                        c.get_location().and_then(|l| l.file.as_deref()),
                        file_filter,
                    ) && glob_match(c.get_name(), pat, false)
                })
                .collect(),
            None => self
                .vars
                .iter()
                .filter(|c| {
                    matches_file(
                        c.get_location().and_then(|l| l.file.as_deref()),
                        file_filter,
                    )
                })
                .collect(),
        };

        for c in matching_vars {
            self.rows.push(Row::Standalone(c));
            if self.has_composition(c) {
                self.rows.push(Row::Composition {
                    constant: c,
                    nested: false,
                    parent_is_last: false,
                });
            }
        }
    }
}

/* ──────────────────────────────── Rendering ─────────────────────────────── */

fn render_row(row: &Row, lookup: &ConstLookup) -> Line<'static> {
    match row {
        Row::EnumHeader(e) => {
            let mut spans = vec![Span::styled(
                e.get_name().to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )];

            if let Some(ty) = e.get_type_name() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    ty.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
            }

            if let Some(loc) = e.get_location() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    loc.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }

        Row::EnumConstant { constant, is_last } => {
            let connector = if *is_last {
                "  \u{2570}\u{2500} "
            } else {
                "  \u{251C}\u{2500} "
            };

            let spans = vec![
                Span::styled(connector.to_string(), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    constant.get_name().to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    constant.get_value().to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ];

            Line::from(spans)
        }

        Row::EnumFooter(count) => Line::from(Span::styled(
            format!("  \u{2570}\u{2500} {count} constants"),
            Style::default().fg(Color::DarkGray),
        )),

        Row::Blank => Line::raw(""),

        Row::Standalone(c) => {
            let mut spans = vec![
                Span::styled(
                    c.get_name().to_string(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];

            if let Some(ty) = c.get_type_name() {
                spans.push(Span::styled(
                    ty.to_string(),
                    Style::default().fg(Color::Cyan),
                ));
                spans.push(Span::raw("  "));
            }

            spans.push(Span::styled(
                c.get_value().to_string(),
                Style::default().fg(Color::Yellow),
            ));

            if let Some(loc) = c.get_location() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    loc.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }

        Row::Composition {
            constant,
            nested,
            parent_is_last,
        } => render_composition(constant, *nested, *parent_is_last, lookup),
    }
}

fn render_composition(
    c: &Constant,
    nested: bool,
    parent_is_last: bool,
    lookup: &ConstLookup,
) -> Line<'static> {
    let prefix = if nested {
        if parent_is_last {
            "     "
        } else {
            "  \u{2502}  "
        }
    } else {
        "   "
    };

    let mut spans = vec![
        Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)),
        Span::styled(
            "\u{2570}\u{2500} ".to_string(),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let tokens = strip_outer_parens(c.get_body_tokens());

    for token in tokens {
        if token.is_identifier {
            if let Some(val) = lookup.get(&token.lit_representation) {
                spans.push(Span::styled(
                    token.lit_representation.clone(),
                    Style::default().fg(Color::Cyan),
                ));
                spans.push(Span::styled(
                    "=".to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::styled(
                    val.to_string(),
                    Style::default().fg(Color::Yellow),
                ));
            } else {
                spans.push(Span::styled(
                    token.lit_representation.clone(),
                    Style::default().fg(Color::White),
                ));
            }
        } else {
            let s = token.lit_representation.trim();
            if !s.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    s.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
                spans.push(Span::raw(" "));
            }
        }
    }

    Line::from(spans)
}

fn strip_outer_parens(tokens: &[MacroBodyToken]) -> &[MacroBodyToken] {
    if tokens.len() >= 2
        && !tokens[0].is_identifier
        && tokens[0].lit_representation == "("
        && !tokens[tokens.len() - 1].is_identifier
        && tokens[tokens.len() - 1].lit_representation == ")"
    {
        &tokens[1..tokens.len() - 1]
    } else {
        tokens
    }
}

/* ─────────────────────────────── File index ─────────────────────────────── */

fn build_file_index(enums: &[Enum], vars: &[Constant], pattern: Option<&str>) -> Vec<FileEntry> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0;

    for e in enums {
        if let Some(file) = e.get_location().and_then(|l| l.file.as_deref()) {
            let matching = match pattern {
                Some(pat) => e
                    .get_constants()
                    .iter()
                    .filter(|c| glob_match(c.get_name(), pat, false))
                    .count(),
                None => e.get_constants().len(),
            };
            if matching > 0 {
                let n = 1 + matching;
                *counts.entry(file.to_string()).or_default() += n;
                total += n;
            }
        }
    }

    for c in vars {
        if let Some(file) = c.get_location().and_then(|l| l.file.as_deref()) {
            let matches = match pattern {
                Some(pat) => glob_match(c.get_name(), pat, false),
                None => true,
            };
            if matches {
                *counts.entry(file.to_string()).or_default() += 1;
                total += 1;
            }
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
