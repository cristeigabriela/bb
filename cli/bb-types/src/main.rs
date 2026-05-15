use std::path::PathBuf;

use anyhow::Result;
use bb_clang::TypedefIndex;
use bb_cli::{current_command_string, get_header_config};
use bb_sql::export_json_to_sqlite;
use bb_types_lib::{StructFilter, TypeResults, collect_results, suggest_alternatives};
use clang::{Clang, Index};
use clap::Parser;

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
        help = "Struct or union name pattern (supports * wildcard). \
                Matches typedef aliases too — `LARGE_INTEGER` finds the union `_LARGE_INTEGER`, \
                `OVERLAPPED` finds the struct `_OVERLAPPED`."
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

/* ────────────────────────────────── main ────────────────────────────────── */

fn main() -> Result<()> {
    let args = Args::parse();

    let config = get_header_config(&args.shared)?;
    let clang_instance = Clang::new().expect("failed to initialize clang");
    let index = Index::new(&clang_instance, false, args.shared.diagnostics);
    let tu = config.parse(&index, false)?;
    let typedef_index = TypedefIndex::build(&tu);

    let filter = StructFilter {
        name_pattern: args.struct_name.clone(),
        header_filter: args.filter.clone(),
        case_sensitive: args.case_sensitive,
    };
    let results = collect_results(&tu, &filter, &typedef_index);

    if results.is_empty() {
        suggest_alternatives(&tu, &typedef_index, args.struct_name.as_deref());
    }

    if let Some(ref path) = args.sqlite {
        export_json_to_sqlite(path, "types", &results.records_as_json_rows())?;
        export_json_to_sqlite(path, "typedefs", &results.typedefs_as_json_rows()?)?;
    } else if args.json {
        print_json(&results)?;
    } else {
        print_display(
            &results,
            args.depth,
            args.field_name.as_deref(),
            &typedef_index,
        );
    }

    Ok(())
}

/* ───────────────────────────────── Printing ─────────────────────────────── */

/// Plain-text render of every struct + union + typedef hit, in
/// `WinDbg` `dt` tree style.
fn print_display(
    results: &TypeResults,
    depth: usize,
    field_name: Option<&str>,
    typedef_index: &TypedefIndex,
) {
    print!(
        "{}",
        results.format_display(depth, field_name, Some(typedef_index))
    );
}

/// Pretty-printed JSON dump of the full query result.
fn print_json(results: &TypeResults) -> Result<()> {
    let output = results.to_json_value(current_command_string());
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
