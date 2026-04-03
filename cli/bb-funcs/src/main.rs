use std::path::PathBuf;

use anyhow::Result;
use bb_clang::Function;
use bb_clang::display::render_function_list;
use bb_cli::{current_command_string, get_header_config, print_suggestions};
use bb_funcs_lib::enriched::{
    ConstantLookup, build_constant_lookup_from_tu, function_to_enriched_json,
    functions_to_enriched_json, render_enriched_detail,
};
use bb_funcs_lib::{
    FuncFilter, FuncSort, ParamCountFilter, SortDir, collect_funcs_filtered, iter_funcs,
};
use bb_sql::export_json_to_sqlite;
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

    #[arg(long = "exported", help = "Show only exported (dllimport) functions")]
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
        long = "signature",
        help = "Parameter type signature pattern. Comma-separated positional slots; _ = any type; ... = any number of params. E.g., HANDLE,...,DWORD,..."
    )]
    signature: Option<String>,

    #[arg(
        short = 'r',
        long = "return",
        help = "Filter by return type (supports * wildcard, e.g., BOOL, void, *STATUS*)"
    )]
    return_type: Option<String>,

    #[arg(long = "has-body", help = "Show only functions with a body")]
    has_body: bool,

    #[arg(
        long = "sort",
        value_enum,
        help = "Sort results (params, name, stack-size)"
    )]
    sort: Option<FuncSort>,

    #[arg(
        long = "sort-dir",
        value_enum,
        default_value = "asc",
        help = "Sort direction (asc, desc)"
    )]
    sort_dir: SortDir,

    #[arg(
        short = 'w',
        long = "where",
        long_help = "SQL WHERE clause for advanced filtering.\n\n\
            Columns: name, return_type, params, stack_size, arch, \
            calling_convention, is_exported, has_body, header.\n\n\
            Operators: =, !=, <, >, <=, >=, AND, OR, NOT, LIKE, IN, BETWEEN.\n\n\
            Examples:\n  \
            --where \"params > 3 AND return_type = 'BOOL'\"\n  \
            --where \"name LIKE '%File%'\"\n  \
            --where \"params BETWEEN 2 AND 5\"\n  \
            --where \"header IN ('fileapi.h', 'handleapi.h')\"",
        help = "SQL WHERE clause for filtering (see --help for column list)"
    )]
    where_clause: Option<String>,

    #[arg(
        short = 'f',
        long = "first",
        num_args = 0..=1,
        default_missing_value = "1",
        help = "Show only the first N results (default: 1 if flag given without value)"
    )]
    first: Option<usize>,

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

    // Parse headers (without macro preprocessing for function collection).
    let tu = config.parse(&index, false)?;

    let func_filter = FuncFilter {
        name_pattern: args.name.clone(),
        header_filter: args.filter.clone(),
        case_sensitive: args.case_sensitive,
        dllimport_only: args.exported,
        param_count: args.params,
        param_type_pattern: args.signature.clone(),
        return_type_pattern: args.return_type.clone(),
        has_body: if args.has_body { Some(true) } else { None },
        sort: args.sort,
        sort_dir: args.sort_dir,
        where_clause: args.where_clause.clone(),
        first: args.first,
    };
    let funcs = collect_funcs_filtered(&tu, &func_filter).map_err(|e| anyhow::anyhow!(e))?;

    // If no function matched, try to print a suggestion.
    if funcs.is_empty() {
        let names: Vec<String> = iter_funcs(&tu).filter_map(|e| e.get_name()).collect();
        print_suggestions(
            "functions",
            args.name.as_deref(),
            names.iter().map(String::as_str),
        );
    }

    // Build constant lookup if sparse data is available.
    let const_lookup = if bb_sparse::is_available() {
        let tu_macro = config.parse(&index, true)?;
        Some(build_constant_lookup_from_tu(&tu_macro))
    } else {
        None
    };

    let detail = args.detail || funcs.len() == 1;

    if let Some(ref path) = args.sqlite {
        let json_rows: Vec<Value> = funcs
            .iter()
            .map(|f| function_to_enriched_json(f, const_lookup.as_ref()))
            .collect();
        export_json_to_sqlite(path, "functions", &json_rows)?;
    } else if args.json {
        print_json(funcs.as_slice(), const_lookup.as_ref())?;
    } else {
        print_display(funcs.as_slice(), detail, const_lookup.as_ref());
    }

    Ok(())
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

fn print_display(funcs: &[Function], detail: bool, const_lookup: Option<&ConstantLookup>) {
    if detail {
        for (i, f) in funcs.iter().enumerate() {
            print!("{}", render_enriched_detail(f, const_lookup));
            if i < funcs.len() - 1 {
                println!();
            }
        }
    } else {
        print!("{}", render_function_list(funcs));
    }
}

fn print_json(funcs: &[Function], const_lookup: Option<&ConstantLookup>) -> Result<()> {
    let command = current_command_string();
    let mut output = serde_json::json!({
        "functions": functions_to_enriched_json(funcs, const_lookup),
    });
    output
        .as_object_mut()
        .unwrap()
        .insert("command".to_string(), Value::String(command));
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
