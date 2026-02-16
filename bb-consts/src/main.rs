use anyhow::Result;
use bb_clang::{ConstLookup, Constant, Enum, ToJson, render_constants};
use bb_cli::{get_header_config, print_suggestions};
use bb_consts_lib::{
    ConstFilter, build_lookup_table, collect_constants, collect_enums, filter_constants_by_name,
    iter_enums, parse_name_pattern, resolve_macros,
};
use bb_shared::glob_match;
use clang::{Clang, Index};
use clap::Parser;

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
    // You must have at least one of either depending on how you invoke the command.
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

    // Build lookup from ALL collected constants (unfiltered by name) so that
    // macros like `#define IMAGEHLP_SYMBOL_INFO_TLSRELATIVE SYMF_TLSREL`
    // can resolve even when the referenced constant doesn't match the pattern.
    let mut known = build_lookup_table(&enums, &vars);
    resolve_macros(&mut vars, &mut known, &failed_macros);

    // Apply name filter AFTER resolution.
    let vars = filter_constants_by_name(vars, &filter);

    // Suggest close constant names when nothing matched the const pattern.
    // Check both standalone vars AND enum children — if neither has a hit,
    // the user likely typo'd.
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

    if args.json {
        print_json(&enums, &vars, &filter)?;
    } else {
        print_display(&enums, &vars, &filter, &known);
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

/// Print enums first, then print non-scoped constants (vars, macros, ...)
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

/// Collect enums, their contents, and non-scoped constants (vars, macros, ...) into a JSON,
/// and pretty-print it.
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

    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let output = serde_json::json!({
        "command": command,
        "enums": filtered_enums.to_json(),
        "constants": vars.to_json(),
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
