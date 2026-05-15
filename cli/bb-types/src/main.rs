use std::path::PathBuf;

use anyhow::Result;
use bb_clang::{Struct, ToJson, Typedef, TypedefIndex};
use bb_cli::{current_command_string, get_header_config, print_suggestions};
use bb_sql::export_json_to_sqlite;
use bb_types_lib::{
    StructFilter, collect_structs, find_struct_by_name, find_typedef_hits, iter_structs,
};
use clang::{Clang, Index};
use clap::Parser;
use colored::Colorize;
use serde_json::Value;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-types",
    about = "Parse Windows SDK or PHNT embedded headers and extract struct information."
)]
struct Args {
    #[command(flatten)]
    shared: bb_cli::SharedArgs,

    // Common options
    #[arg(long, help = "Output as JSON")]
    json: bool,
    #[arg(
        short = 'H',
        long = "filter",
        help = "Filter by header file (e.g., winternl.h)"
    )]
    filter: Option<String>,

    #[arg(
        short = 's',
        long = "struct",
        help = "Struct name pattern (supports * wildcard). \
                Matches typedef aliases too — `LARGE_INTEGER` finds `_LARGE_INTEGER`."
    )]
    struct_name: Option<String>,

    #[arg(
        short = 'f',
        long = "field",
        help = "Field name pattern (supports * wildcard)"
    )]
    field_name: Option<String>,

    #[arg(short = 'c', long = "case-sensitive", help = "Case-sensitive matching")]
    case_sensitive: bool,

    #[arg(
        short = 'd',
        long = "depth",
        default_value = "0",
        help = "Recursion depth for nested types"
    )]
    depth: usize,

    #[arg(long = "sqlite", help = "Export results to a SQLite database file")]
    sqlite: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Build header configuration.
    let config = get_header_config(&args.shared)?;

    // Set up Clang.
    let clang_instance = Clang::new().expect("failed to initialize clang");
    let index = Index::new(&clang_instance, false, args.shared.diagnostics);

    // Parse headers.
    let tu = config.parse(&index, false)?;

    // Build the typedef index once: needed to (1) discover aliases for
    // every struct we render, (2) resolve typedef-only lookups like
    // `HANDLE`, (3) drive the inline `(canonical)` annotation on field
    // type cells.
    let typedef_index = TypedefIndex::build(&tu);

    let filter = StructFilter {
        name_pattern: args.struct_name.clone(),
        header_filter: args.filter.clone(),
        case_sensitive: args.case_sensitive,
    };
    let mut structs = collect_structs(&tu, &filter, Some(&typedef_index));

    // Auto-expand pointer-typedef targets: when the user searches a
    // pointer typedef like `LPSECURITY_ATTRIBUTES`, also pull in the
    // `_SECURITY_ATTRIBUTES` struct it points to. Saves consumers from
    // having to do a second lookup, and makes text rendering useful
    // ("show me the layout of what this points at").
    {
        let initial_typedef_pattern_hits = find_typedef_hits(&typedef_index, &filter);
        let already: std::collections::HashSet<String> =
            structs.iter().map(|s| s.get_name().to_string()).collect();
        let mut to_pull: Vec<String> = Vec::new();
        let mut seen_pull: std::collections::HashSet<String> = std::collections::HashSet::new();
        for td in &initial_typedef_pattern_hits {
            // Prefer canonical_decl_name (direct record alias) over
            // properties.underlying_record (pointer-to-record), but
            // expand for either.
            let candidate = td
                .canonical_decl_name
                .as_deref()
                .or(td.properties.underlying_record.as_deref());
            if let Some(record_name) = candidate
                && !already.contains(record_name)
                && seen_pull.insert(record_name.to_string())
            {
                to_pull.push(record_name.to_string());
            }
        }
        for record_name in to_pull {
            if let Some(s) = find_struct_by_name(&tu, &record_name, Some(&typedef_index)) {
                structs.push(s);
            }
        }
    }

    // Resolve every typedef the user could reasonably want surfaced:
    //
    //  1. Typedefs whose **name** matches the search pattern. Surfaces
    //     pointer/primitive typedefs that don't resolve to a struct
    //     (e.g. `HANDLE`, `PVOID`) so they're never invisible.
    //
    //  2. Typedefs that resolve to any **rendered struct's canonical
    //     name**. Surfaces struct aliases regardless of which name the
    //     user typed — `-s LARGE_INTEGER` and `-s _LARGE_INTEGER` both
    //     produce a `LARGE_INTEGER` typedef entry, so API consumers
    //     always see the bidirectional mapping in one place.
    //
    // Results are merged + deduplicated by typedef name.
    let rendered_canonical_names: std::collections::HashSet<&str> =
        structs.iter().map(Struct::get_name).collect();
    let typedef_hits: Vec<&Typedef> = {
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut acc: Vec<&Typedef> = Vec::new();

        // Pattern matches.
        for t in find_typedef_hits(&typedef_index, &filter) {
            if seen.insert(t.name.as_str()) {
                acc.push(t);
            }
        }

        // Aliases of every rendered struct.
        for s in &structs {
            for alias_name in typedef_index.aliases_for(s.get_name()) {
                if let Some(t) = typedef_index.lookup(alias_name)
                    && seen.insert(t.name.as_str())
                {
                    acc.push(t);
                }
            }
        }

        acc.sort_by(|a, b| a.name.cmp(&b.name));
        acc
    };

    // Suppress noise in plain-text rendering: a typedef stub printed
    // inline below a struct render is redundant with the `[aka …]`
    // chip in the struct header. JSON / SQLite always include the full
    // typedef list, since machine consumers want both directions.
    let typedef_hits_text: Vec<&Typedef> = typedef_hits
        .iter()
        .copied()
        .filter(|t| {
            t.canonical_decl_name
                .as_deref()
                .is_none_or(|c| !rendered_canonical_names.contains(c))
        })
        .collect();

    // No-results suggestion: include both struct names and typedef names
    // in the candidate pool. The user might be off by an underscore or a
    // capitalization.
    if structs.is_empty() && typedef_hits.is_empty() {
        let struct_names: Vec<String> =
            iter_structs(&tu).filter_map(|e| e.get_name()).collect();
        let mut candidates: Vec<&str> = struct_names.iter().map(String::as_str).collect();
        candidates.extend(typedef_index.names());
        print_suggestions(
            "structs or typedefs",
            args.struct_name.as_deref(),
            candidates.into_iter(),
        );
    }

    if let Some(ref path) = args.sqlite {
        let json_rows: Vec<Value> = structs.iter().map(bb_clang::ToJson::to_json).collect();
        export_json_to_sqlite(path, "types", &json_rows)?;
        // Export typedef hits to a sibling table for symmetry. Always
        // create the table so consumers can rely on the schema; an empty
        // typedef set produces an empty table.
        let typedef_rows: Vec<Value> = typedef_hits
            .iter()
            .map(|t| serde_json::to_value(t).expect("Typedef serializes"))
            .collect();
        export_json_to_sqlite(path, "typedefs", &typedef_rows)?;
    } else if args.json {
        print_json(structs.as_slice(), &typedef_hits)?;
    } else {
        print_display(
            structs.as_slice(),
            args.depth,
            &args.field_name,
            Some(&typedef_index),
            &typedef_hits_text,
        );
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

/// Print using `WinDbg` `dt`, tree-like structure style.
///
/// Renders each struct (with typedef aliases in its header and dim
/// `(canonical)` annotations on typedef'd field types), then renders any
/// typedef-only hits (`HANDLE → PVOID → void *`) as a separate trailing
/// section.
fn print_display(
    structs: &[Struct],
    depth: usize,
    field_name: &Option<String>,
    typedef_index: Option<&TypedefIndex>,
    typedef_hits: &[&Typedef],
) {
    for s in structs {
        print!("{}", s.display(depth, field_name.as_deref(), typedef_index));
    }

    if !typedef_hits.is_empty() {
        if !structs.is_empty() {
            println!();
        }
        println!("{}", "typedefs".white().bold().underline());
        for t in typedef_hits {
            print_typedef_summary(t);
        }
    }
}

/// One-line summary for a typedef-only hit.
///
/// Example: `HANDLE  →  PVOID → void *   (pointer)  winnt.h:1234:5`.
fn print_typedef_summary(t: &Typedef) {
    let name = t.name.cyan().bold();
    let arrow_chain: String = t
        .chain
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(" → ");
    let kind = format!("({})", typedef_kind_label(t.kind)).dimmed();
    let loc = t
        .location
        .as_ref()
        .map(|l| format!("  {}", l.to_string().dimmed()))
        .unwrap_or_default();
    println!("  {name}  →  {arrow_chain}   {kind}{loc}");
}

/// Human label for a [`bb_clang::TypedefKind`] in CLI output.
const fn typedef_kind_label(k: bb_clang::TypedefKind) -> &'static str {
    match k {
        bb_clang::TypedefKind::Struct => "struct",
        bb_clang::TypedefKind::Union => "union",
        bb_clang::TypedefKind::Enum => "enum",
        bb_clang::TypedefKind::FunctionPointer => "function pointer",
        bb_clang::TypedefKind::Pointer => "pointer",
        bb_clang::TypedefKind::Array => "array",
        bb_clang::TypedefKind::Primitive => "primitive",
        bb_clang::TypedefKind::Other => "other",
    }
}

/// Print JSON with `types` and `typedefs` arrays, both always present.
///
/// Shape (designed for API consumers — predictable, no required indirection):
///
/// ```json
/// {
///   "command": "...",
///   "types":    [ /* full Struct objects, each with `aliases` */ ],
///   "typedefs": [
///     {
///       "name": "LARGE_INTEGER",
///       "kind": "struct",
///       "typedef_of": "_LARGE_INTEGER",
///       "canonical": "_LARGE_INTEGER",
///       "canonical_decl_name": "_LARGE_INTEGER",
///       "chain": ["_LARGE_INTEGER"]
///     },
///     {
///       "name": "HANDLE",
///       "kind": "pointer",
///       "typedef_of": "PVOID",
///       "canonical": "void *",
///       "chain": ["PVOID", "void *"]
///     }
///   ]
/// }
/// ```
///
/// - `types` is the full struct render (`ToJson::to_json_full`).
/// - `typedefs` contains stubs for every typedef the user *searched* by
///   name, including those that resolve to a struct already in `types`
///   — so consumers can always look up by either name and find a clear
///   pointer back to the canonical entry.
fn print_json(structs: &[Struct], typedef_hits: &[&Typedef]) -> anyhow::Result<()> {
    let command = current_command_string();
    let typedefs_value = serde_json::to_value(typedef_hits)?;

    let mut output = structs.to_json_full();
    let obj = output
        .as_object_mut()
        .expect("Struct slice to_json_full returns an object");
    obj.insert("command".to_string(), Value::String(command));
    obj.insert("typedefs".to_string(), typedefs_value);

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
