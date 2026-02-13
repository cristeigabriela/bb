use anyhow::Result;
use bb_clang::Struct;
use bb_cli::{get_header_config, print_suggestions};
use bb_types_lib::{StructFilter, collect_structs, iter_structs};
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
        help = "Struct name pattern (supports * wildcard)"
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

    let filter = StructFilter {
        name_pattern: args.struct_name.clone(),
        header_filter: args.filter.clone(),
        case_sensitive: args.case_sensitive,
    };
    let structs = collect_structs(&tu, &filter);

    // If no struct that matches our filter was found, try to print a suggestion.
    if structs.is_empty() {
        let names: Vec<String> = iter_structs(&tu).filter_map(|e| e.get_name()).collect();
        print_suggestions(
            "structs",
            args.struct_name.as_deref(),
            names.iter().map(String::as_str),
        );
    }

    if args.json {
        print_json(structs.as_slice(), args.depth)?;
    } else {
        print_display(structs.as_slice(), args.depth, &args.field_name);
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

/// Print using `WinDbg` `dt`, tree-like structure style.
///
/// # Arguments
///
/// * `structs` - The [`Struct`] entities to display.
/// * `depth` - The depth of type expansion to be shown inline.
/// * `field_name` - Particular field to filter for in [`Struct`].
fn print_display(structs: &[Struct], depth: usize, field_name: &Option<String>) {
    for s in structs {
        print!("{}", s.display(depth, field_name.as_deref()));
    }
}

/// Print JSON with a flat `types` array.
///
/// Each type entry includes `referenced_types` listing the names of expandable
/// child types. When `depth > 0`, those referenced types are also present as
/// entries in the same array, so the consumer can look them up by name.
fn print_json(structs: &[Struct], depth: usize) -> anyhow::Result<()> {
    use std::collections::HashSet;

    let mut all_types: Vec<serde_json::Value> = Vec::new();
    let mut seen = HashSet::new();

    // Helper: serialize a Struct and append its referenced_types names.
    let to_json = |s: &Struct| -> anyhow::Result<serde_json::Value> {
        let mut val = serde_json::to_value(s)?;
        val.as_object_mut().unwrap().insert(
            "referenced_types".to_string(),
            serde_json::to_value(s.referenced_type_names())?,
        );
        Ok(val)
    };

    // Queried types first.
    for s in structs {
        seen.insert(s.get_name().to_string());
        all_types.push(to_json(s)?);
    }

    // Expand nested types up to depth.
    for s in structs {
        for nested in s.extract_nested_types(depth) {
            if seen.insert(nested.get_name().to_string()) {
                all_types.push(to_json(&nested)?);
            }
        }
    }

    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let output = serde_json::json!({ "command": command, "types": all_types });
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
