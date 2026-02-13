mod data;

use anyhow::Result;
use bb_cli::get_header_config;
use bb_consts_lib::{
    ConstFilter, build_lookup_table, collect_constants, collect_enums, parse_name_pattern,
    resolve_macros,
};
use clang::{Clang, Index};
use clap::Parser;

use data::ConstData;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-consts-tui",
    about = "TUI browser for Windows SDK / PHNT constants."
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
        long,
        help = "Constant name pattern (supports * wildcard, Enum::Const syntax)"
    )]
    name: Option<String>,

    #[arg(short, long = "enum", help = "Enum name pattern (supports * wildcard)")]
    enum_name: Option<String>,

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
    let (mut vars, failed_macros) = collect_constants(&tu, &filter);
    let mut known = build_lookup_table(&enums, &vars);
    resolve_macros(&mut vars, &mut known, &failed_macros);

    let initial_search = match args.name.as_deref() {
        Some(n) if !n.contains("::") => n,
        _ => "",
    };

    // Initialize and run the TUI.
    let data = ConstData::new(&enums, &vars, &known);
    let mut app = bb_tui::App::new(data, initial_search);
    bb_tui::event::run(&mut app)?;

    Ok(())
}
