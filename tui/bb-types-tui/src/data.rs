use std::collections::{BTreeMap, HashSet};

use bb_clang::{Field, RecordKind, Struct, TypedefIndex};
use bb_shared::glob_match;
use bb_tui::{FileEntry, TuiData, matches_file};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// Maximum depth for inline expansion of nested record types in the TUI.
/// Matches the JSON `to_json_full` expansion depth in
/// `crates/bb-clang/src/json.rs` so the TUI shows the same shape the
/// JSON consumer would see.
const MAX_DEPTH: usize = 8;

/// Pre-rendered field row data — owned strings, no `Field` lifetime
/// dependency. Lets us recursively expand nested anonymous records
/// without juggling self-referential borrows on the TUI state.
struct RenderedField {
    prefix: String,
    connector: &'static str, // "├─" or "╰─"
    offset_bytes: usize,
    size: usize,
    name: Option<String>, // None ⇒ anonymous, name column suppressed
    type_name: Option<String>,
    annotation: Option<String>, // dim "(canonical)" chip for typedef'd types
    anon_chip: Option<&'static str>, // "anonymous union" / "anonymous struct"
}

enum Row<'a> {
    StructHeader(&'a Struct<'a>),
    Field(RenderedField),
    Footer(usize),
    Blank,
}

pub struct TypeData<'a> {
    structs: &'a [Struct<'a>],
    typedef_index: &'a TypedefIndex,
    rows: Vec<Row<'a>>,
    files: Vec<FileEntry>,
}

impl<'a> TypeData<'a> {
    pub fn new(structs: &'a [Struct<'a>], typedef_index: &'a TypedefIndex) -> Self {
        Self {
            structs,
            typedef_index,
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

            // Match the canonical name OR any typedef alias, so the user
            // can search the TUI by `LARGE_INTEGER` and still find
            // `_LARGE_INTEGER`.
            let name_matches = match search {
                Some(pat) => {
                    glob_match(s.get_name(), pat, false)
                        || s.get_aliases().iter().any(|a| glob_match(a, pat, false))
                }
                None => true,
            };

            if !name_matches {
                continue;
            }

            self.rows.push(Row::StructHeader(s));

            let fields = s.get_fields();
            let count = fields.len();
            let mut seen: HashSet<String> = HashSet::new();
            seen.insert(s.get_name().to_string());
            push_fields_recursive(&mut self.rows, fields, "", 0, self.typedef_index, &mut seen);

            self.rows.push(Row::Footer(count));
            self.rows.push(Row::Blank);
        }
    }
}

/* ─────────────────────── Recursive field expansion ──────────────────────── */

/// Walk `fields` and push a row per field. When a field's type is a
/// record (named or anonymous) with its own fields, recurse up to
/// [`MAX_DEPTH`] levels. Cycles are broken by `seen`, keyed on the
/// composite identity (`enclosing_record::field_path` for anonymous,
/// canonical decl name for named).
fn push_fields_recursive(
    rows: &mut Vec<Row<'_>>,
    fields: &[Field<'_>],
    prefix: &str,
    current_depth: usize,
    typedef_index: &TypedefIndex,
    seen: &mut HashSet<String>,
) {
    let count = fields.len();
    for (i, f) in fields.iter().enumerate() {
        let is_last = i == count - 1;
        let connector: &'static str = if is_last { "╰─" } else { "├─" };
        rows.push(Row::Field(make_rendered_field(
            f,
            prefix.to_string(),
            connector,
            typedef_index,
        )));

        if current_depth >= MAX_DEPTH || !f.has_children() {
            continue;
        }

        let type_key = if let Some(aref) = f.get_anon_ref() {
            Some(aref.identity())
        } else {
            f.get_underlying_type()
                .get_declaration()
                .and_then(|d| d.get_name())
        };

        let Some(key) = type_key else {
            continue;
        };
        if !seen.insert(key.clone()) {
            continue;
        }
        let child_fields = f.get_child_fields();
        if !child_fields.is_empty() {
            let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
            push_fields_recursive(
                rows,
                &child_fields,
                &child_prefix,
                current_depth + 1,
                typedef_index,
                seen,
            );
        }
        seen.remove(&key);
    }
}

/// Build the pre-rendered row payload for one field, resolving the
/// typedef annotation eagerly so render time has no `TypedefIndex`
/// dependency.
fn make_rendered_field(
    field: &Field,
    prefix: String,
    connector: &'static str,
    typedef_index: &TypedefIndex,
) -> RenderedField {
    let type_name = field.get_type_name().map(str::to_string);

    let annotation = type_name.as_deref().and_then(|ty| {
        bb_clang::display::typedef_annotation(
            ty,
            field.get_type_info().underlying_record.as_deref(),
            Some(typedef_index),
        )
    });

    // Anonymous-type chip: when a field's type itself has no name
    // (the OVERLAPPED anon union case), show "<anonymous union>" or
    // "<anonymous struct>" in the type column. AnonRef carries the
    // record kind directly.
    let anon_chip: Option<&'static str> = if type_name.is_none() {
        match field.get_anon_ref().map(|r| r.kind) {
            Some(RecordKind::Union) => Some("union"),
            Some(RecordKind::Struct) => Some("struct"),
            None => None,
        }
    } else {
        None
    };

    // Suppress synthetic `<anonymous_N>` names — only show real C
    // identifiers.
    let name = if field.is_anonymous() {
        None
    } else {
        Some(field.get_name().to_string())
    };

    RenderedField {
        prefix,
        connector,
        offset_bytes: field.get_offset_bytes(),
        size: field.get_size(),
        name,
        type_name,
        annotation,
        anon_chip,
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

            let aliases = s.get_aliases();
            if !aliases.is_empty() {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("[aka {}]", aliases.join(", ")),
                    Style::default().fg(Color::DarkGray),
                ));
            }

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

        Row::Field(rf) => {
            let mut spans: Vec<Span<'static>> = vec![
                Span::raw("  "),
                Span::styled(rf.prefix.clone(), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} ", rf.connector),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("+0x{:04X}", rf.offset_bytes),
                    Style::default().fg(Color::Yellow),
                ),
                Span::raw("  "),
            ];

            if let Some(n) = &rf.name {
                spans.push(Span::styled(
                    n.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            if let Some(ty) = &rf.type_name {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(ty.clone(), Style::default().fg(Color::Cyan)));
                if let Some(ann) = &rf.annotation {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(
                        format!("({ann})"),
                        Style::default().fg(Color::DarkGray),
                    ));
                }
            } else if let Some(chip) = rf.anon_chip {
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("<anonymous {chip}>"),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("{} bytes", rf.size),
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
            Some(pat) => {
                glob_match(s.get_name(), pat, false)
                    || s.get_aliases().iter().any(|a| glob_match(a, pat, false))
            }
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
