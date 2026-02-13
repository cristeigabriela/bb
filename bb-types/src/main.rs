use anyhow::Result;
use bb_clang::Struct;
use bb_cli::get_header_config;
use bb_types_lib::{StructFilter, collect_structs};
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

/// Print JSON of serialized [`Struct`]s and [`Field`]s as an array.
///
/// # Arguments
///
/// * `structs` - The [`Struct`] entities to analyze and serialize.
/// * `depth` - The depth of type expansion to be captured into the JSON array.
fn print_json(structs: &[Struct], depth: usize) -> anyhow::Result<()> {
    let mut all_structs: Vec<&Struct> = Vec::new();
    let nested: Vec<_> = structs
        .iter()
        .flat_map(|s| s.extract_nested_types(depth))
        .collect();
    all_structs.extend(structs.iter());
    all_structs.extend(nested.iter());
    println!("{}", serde_json::to_string_pretty(&all_structs)?);
    Ok(())
}
