use anyhow::Result;
use bb_clang::{Function, ToJson};
use bb_cli::{get_header_config, print_suggestions};
use bb_funcs_lib::{FuncFilter, FuncSort, ParamCountFilter, collect_funcs_filtered, iter_funcs};
use clang::{Clang, Index};
use clap::Parser;
use serde_json::Value;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-funcs",
    about = "Parse Windows SDK or PHNT embedded headers and extract function declarations."
)]
struct Args {
    #[command(flatten)]
    shared: bb_cli::SharedArgs,

    #[arg(long, help = "Output as JSON")]
    json: bool,

    #[arg(
        short = 'H',
        long = "filter",
        help = "Filter by header file (e.g., processthreadsapi.h)"
    )]
    filter: Option<String>,

    #[arg(
        short = 'n',
        long = "name",
        help = "Function name pattern (supports * wildcard)"
    )]
    name: Option<String>,

    #[arg(short = 'c', long = "case-sensitive", help = "Case-sensitive matching")]
    case_sensitive: bool,

    #[arg(
        long = "exported",
        help = "Show only exported (dllimport) functions"
    )]
    exported: bool,

    #[arg(
        short = 'd',
        long = "detail",
        help = "Force detailed ABI breakdown for all results (auto for single result)"
    )]
    detail: bool,

    #[arg(
        short = 'p',
        long = "params",
        help = "Filter by parameter count (e.g., 3, 0, 3..7, 3..)"
    )]
    params: Option<ParamCountFilter>,

    #[arg(
        long = "param-type",
        help = "Parameter type pattern. Comma-separated positional slots; _ = any type; ... = any number of params. E.g., HANDLE,...,DWORD,..."
    )]
    param_type: Option<String>,

    #[arg(
        short = 'r',
        long = "return",
        help = "Filter by return type (supports * wildcard, e.g., BOOL, void, *STATUS*)"
    )]
    return_type: Option<String>,

    #[arg(long = "has-body", help = "Show only functions with a body")]
    has_body: bool,

    #[arg(long = "sort", value_enum, help = "Sort results (params, name)")]
    sort: Option<FuncSort>,
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

    let func_filter = FuncFilter {
        name_pattern: args.name.clone(),
        header_filter: args.filter.clone(),
        case_sensitive: args.case_sensitive,
        dllimport_only: args.exported,
        param_count: args.params,
        param_type_pattern: args.param_type.clone(),
        return_type_pattern: args.return_type.clone(),
        has_body: if args.has_body { Some(true) } else { None },
        sort: args.sort,
    };
    let funcs = collect_funcs_filtered(&tu, &func_filter);

    // If no function matched, try to print a suggestion.
    if funcs.is_empty() {
        let names: Vec<String> = iter_funcs(&tu).filter_map(|e| e.get_name()).collect();
        print_suggestions(
            "functions",
            args.name.as_deref(),
            names.iter().map(String::as_str),
        );
    }

    if args.json {
        print_json(funcs.as_slice())?;
    } else {
        // Auto-detail when there's exactly 1 result.
        let detail = args.detail || funcs.len() == 1;
        print_display(funcs.as_slice(), detail);
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

fn print_display(funcs: &[Function], detail: bool) {
    if detail {
        for (i, f) in funcs.iter().enumerate() {
            print!("{}", f.display_detail());
            if i < funcs.len() - 1 {
                println!();
            }
        }
    } else {
        print!("{}", bb_clang::display::render_function_list(funcs));
    }
}

fn print_json(funcs: &[Function]) -> anyhow::Result<()> {
    let command = std::env::args().collect::<Vec<_>>().join(" ");
    let mut output = serde_json::json!({
        "functions": funcs.to_json(),
    });
    output
        .as_object_mut()
        .unwrap()
        .insert("command".to_string(), Value::String(command));
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
