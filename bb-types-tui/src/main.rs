mod data;

use anyhow::Result;
use bb_cli::get_header_config;
use bb_types_lib::{StructFilter, collect_structs};
use clang::{Clang, Index};
use clap::Parser;

use data::TypeData;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-types-tui",
    about = "TUI browser for Windows SDK / PHNT struct types."
)]
struct Args {
    #[command(flatten)]
    shared: bb_cli::SharedArgs,

    #[arg(
        short = 'H',
        long = "filter",
        help = "Filter by header file (e.g., winternl.h)"
    )]
    filter: Option<String>,

    #[arg(
        short,
        long = "struct",
        help = "Struct name pattern (supports * wildcard)"
    )]
    struct_name: Option<String>,

    #[arg(short, long = "case-sensitive", help = "Case-sensitive matching")]
    case_sensitive: bool,
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

    let initial_search = args.struct_name.as_deref().unwrap_or("");

    // Initialize and run the TUI.
    let data = TypeData::new(&structs);
    let mut app = bb_tui::App::new(data, initial_search);
    bb_tui::event::run(&mut app)?;

    Ok(())
}
