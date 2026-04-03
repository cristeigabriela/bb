use std::path::PathBuf;

use anyhow::Result;
use bb_clang::{ConstLookup, Constant, Enum, ToJson, build_referred_components, render_constants};
use bb_cli::{get_header_config, print_suggestions};
use bb_consts_lib::{
    ConstFilter, build_lookup_table, collect_constants, collect_enums, filter_constants_by_name,
    iter_enums, parse_name_pattern,
};
use bb_shared::glob_match;
use clang::{Clang, Index};
use clap::Parser;
use serde_json::Value;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-consts",
    about = "Parse Windows SDK or PHNT embedded headers and extract constants."
)]
struct Args {
    #[command(flatten)]
    shared: bb_cli::SharedArgs,

    #[arg(long, help = "Output as JSON")]
    json: bool,

    #[arg(
        short = 'H',
        long = "filter",
        help = "Filter by header file (e.g., winternl.h)"
    )]
    filter: Option<String>,

    #[arg(
        short = 'n',
        long,
        help = "Constant name pattern (supports * wildcard, Enum::Const syntax)"
    )]
    name: Option<String>,

    #[arg(
        short = 'e',
        long = "enum",
        help = "Enum name pattern (supports * wildcard)"
    )]
    enum_name: Option<String>,

    #[arg(short = 'c', long = "case-sensitive", help = "Case-sensitive matching")]
    case_sensitive: bool,

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

    // Parse headers (with detailed preprocessing for macros).
    let tu = config.parse(&index, true)?;

    // Get (optional) enum from name, and (optional) constant pattern.
    let (enum_from_name, const_pattern) = parse_name_pattern(args.name.as_deref());

    let filter = ConstFilter {
        header_filter: args.filter.as_ref().map(|h| h.to_lowercase()),
        enum_pattern: args
            .enum_name
            .as_deref()
            .or(enum_from_name)
            .map(str::to_string),
        const_pattern: const_pattern.map(str::to_string),
        case_sensitive: args.case_sensitive,
        scoped_to_enum: enum_from_name.is_some() || args.enum_name.is_some(),
    };

    let enums = collect_enums(&tu, &filter);
    let vars = collect_constants(&tu, &filter);

    // Build display lookup from all collected constants.
    let known = build_lookup_table(&enums, &vars);

    // Apply name filter AFTER collection (collection is always unfiltered by
    // name so the TU entity map contains every constant needed for resolution).
    let vars = filter_constants_by_name(vars, &filter);

    // Suggest close constant names when nothing matched the const pattern.
    if let Some(pat) = filter.const_pattern.as_deref() {
        let has_enum_hit = enums.iter().any(|e| {
            e.get_constants()
                .iter()
                .any(|c| glob_match(c.get_name(), pat, filter.case_sensitive))
        });
        if vars.is_empty() && !has_enum_hit {
            print_suggestions("constants", Some(pat), known.keys().map(String::as_str));
        }
    }

    // Suggest close enum names when nothing matched the enum pattern.
    if enums.is_empty() && filter.enum_pattern.is_some() {
        let enum_names: Vec<String> = iter_enums(&tu).filter_map(|e| e.get_name()).collect();
        print_suggestions(
            "enums",
            filter.enum_pattern.as_deref(),
            enum_names.iter().map(String::as_str),
        );
    }

    if let Some(ref path) = args.sqlite {
        export_consts_sqlite(&enums, &vars, path)?;
    } else if args.json {
        print_json(&enums, &vars, &filter)?;
    } else {
        print_display(&enums, &vars, &filter, &known);
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

fn print_display(enums: &[Enum], vars: &[Constant], filter: &ConstFilter, lookup: &ConstLookup) {
    for e in enums {
        match filter.const_pattern.as_deref() {
            Some(pat) => print!("{}", e.display_filtered(pat, filter.case_sensitive)),
            None => print!("{}", e.display()),
        }
    }

    if !vars.is_empty() {
        print!("{}", render_constants(vars, false, Some(lookup)));
    }
}

/// Collect enums, their contents, and non-scoped constants into a JSON,
/// with a `referred_components` field containing fully serialized objects
/// for every constant transitively referenced as a component.
fn print_json(enums: &[Enum], vars: &[Constant], filter: &ConstFilter) -> Result<()> {
    let filtered_enums: Vec<&Enum> = enums
        .iter()
        .filter(|e| {
            filter.const_pattern.as_deref().is_none_or(|pat| {
                e.get_constants()
                    .iter()
                    .any(|c| glob_match(c.get_name(), pat, filter.case_sensitive))
            })
        })
        .collect();

    let command = bb_cli::current_command_string();

    let referred =
        build_referred_components(vars.iter().map(|c| c.get_name().to_string()), vars.iter());

    let output = serde_json::json!({
        "command": command,
        "constants": vars.to_json(),
        "enums": filtered_enums.to_json(),
        "referred_components": referred,
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/* ──────────────────────────── SQLite export ───────────────────────────── */

fn export_consts_sqlite(enums: &[Enum], vars: &[Constant], path: &std::path::Path) -> Result<()> {
    let mut total = 0;

    // Export standalone constants to their own table.
    let const_rows: Vec<Value> = vars.iter().map(|c| c.to_json()).collect();
    total += const_rows.len();
    if !const_rows.is_empty() {
        bb_sql::export_json_to_sqlite(path, "constants", &const_rows)?;
    }

    // Export enum constants to a separate table (includes parent enum name).
    let mut enum_const_rows: Vec<Value> = Vec::new();
    for e in enums {
        for c in e.get_constants() {
            let mut val = c.to_json();
            if let Some(obj) = val.as_object_mut() {
                obj.insert("enum".to_string(), Value::String(e.get_name().to_string()));
            }
            enum_const_rows.push(val);
        }
    }
    total += enum_const_rows.len();
    if !enum_const_rows.is_empty() {
        bb_sql::export_json_to_sqlite(path, "enum_constants", &enum_const_rows)?;
    }

    // Export enums themselves as a third table.
    let enum_rows: Vec<Value> = enums.iter().map(|e| e.to_json()).collect();
    if !enum_rows.is_empty() {
        bb_sql::export_json_to_sqlite(path, "enums", &enum_rows)?;
    }

    eprintln!("exported {total} constants to {}", path.display());
    Ok(())
}
