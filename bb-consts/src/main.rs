use std::collections::HashMap;

use anyhow::Result;
use bb_clang::{ConstLookup, Constant, Enum, render_constants};
use bb_sdk::{Arch, HeaderConfig, PhntVersion, SdkMode};
use bb_shared::glob_match;
use clang::{Clang, Entity, EntityKind, Index, TranslationUnit};
use clap::Parser;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-consts",
    about = "Parse Windows SDK or PHNT embedded headers and extract constants."
)]
struct Args {
    #[arg(long, help = "Use Windows SDK headers (optionally specify version)")]
    winsdk: Option<Option<String>>,

    #[arg(long, value_enum, help = "Use PHNT headers with specified version")]
    phnt: Option<Option<PhntVersion>>,

    #[arg(
        short,
        long,
        value_enum,
        default_value = "user",
        help = "Mode: user or kernel (defines _KERNEL_MODE for kernel)"
    )]
    mode: SdkMode,

    #[arg(long, help = "Output as JSON")]
    json: bool,

    #[arg(
        short,
        long = "arch",
        value_enum,
        default_value = "amd64",
        help = "Architecture to target (supports cross-compilation)"
    )]
    arch: Arch,

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

    #[arg(long, help = "Show clang diagnostics")]
    diagnostics: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Build header configuration.
    let config = get_header_config(&args)?;

    // Set up Clang.
    let clang_instance = Clang::new().expect("failed to initialize clang");
    let index = Index::new(&clang_instance, false, args.diagnostics);

    // Parse headers (with detailed preprocessing for macros).
    let tu = config.parse(&index, true)?;

    // Get (optional) enum from name, and (optional) constant pattern.
    // You must have at least one of either depending on how you invoke the command.
    let (enum_from_name, const_pattern) = parse_name_pattern(args.name.as_deref());

    let enum_pattern = args.enum_name.as_deref().or(enum_from_name);
    let header_filter = args.filter.as_ref().map(|h| h.to_lowercase());
    let case_sensitive = args.case_sensitive;

    // We are searching in the context of enums exclusively if there is a `::` in the `name` string,
    // or if there is an `enum` argument.
    let scoped_to_enum = enum_from_name.is_some() || args.enum_name.is_some();

    let enums = collect_enums(&tu, header_filter.as_deref(), enum_pattern, case_sensitive);
    let (mut vars, failed_macros) = collect_constants(
        &tu,
        header_filter.as_deref(),
        const_pattern,
        case_sensitive,
        scoped_to_enum,
    );

    // Preprocess all macros.
    let mut known = build_lookup_table(&enums, &vars);
    // Collect macros into `vars` and also resolve all the components.
    resolve_macros(&mut vars, &mut known, &failed_macros);

    if args.json {
        print_json(&enums, &vars, const_pattern, case_sensitive)?;
    } else {
        print_display(&enums, &vars, const_pattern, case_sensitive, &known);
    }

    Ok(())
}

/* ─────────────────────────────────── SDK ────────────────────────────────── */

/// Build a [`HeaderConfig`] from the command-line arguments.
///
/// - By default, `WinSDK` will be preferred if nothing is explicitly specified.
/// - By default, the `WinSDK` version is inferred from environment.
/// - By default, the PHNT version is Win11.
fn get_header_config(args: &Args) -> Result<HeaderConfig> {
    match (&args.winsdk, &args.phnt) {
        (Some(_), Some(_)) => anyhow::bail!("Cannot use both --winsdk and --phnt"),
        (Some(version), None) => match version {
            Some(v) => HeaderConfig::winsdk_version(v, args.arch, args.mode),
            None => HeaderConfig::winsdk(args.arch, args.mode),
        },
        (None, Some(version)) => {
            let phnt_version = (*version).unwrap_or_default();
            HeaderConfig::phnt(args.arch, phnt_version, args.mode)
        }
        (None, None) => HeaderConfig::winsdk(args.arch, args.mode),
    }
}

/* ────────────────────── Parse, iter, collect, filter ────────────────────── */

/// Parse name pattern.
///
/// When `name` contains `::`, it implies that the `name` command-line argument
/// is scoped to an enumeration.
///
/// If `::` is present, then, collect the pattern from the left-hand side of the string,
/// and use that to filter and collect enums, and take the right-hand side as the field
/// to look for in said enum.
///
/// # Examples
///
/// `name`: `Some("A::B")` -> `Some(("A", "B"))`
///
/// `name`: `Some("A")` -> `Some(("", "A"))`
///
/// `name`: `None` -> `None`
fn parse_name_pattern(name: Option<&str>) -> (Option<&str>, Option<&str>) {
    match name {
        Some(n) if n.contains("::") => {
            let (enum_part, const_part) = n.split_once("::").unwrap();
            (Some(enum_part), Some(const_part))
        }
        Some(n) => (None, Some(n)),
        None => (None, None),
    }
}

/// Iterate over enum declarations in [`TranslationUnit`] and collect ones that
/// match filter settings.
fn collect_enums<'a>(
    tu: &'a TranslationUnit<'a>,
    header_filter: Option<&str>,
    enum_pattern: Option<&str>,
    case_sensitive: bool,
) -> Vec<Enum<'a>> {
    iter_enums(tu)
        .filter(|e| matches_header(e, header_filter))
        .filter(|e| {
            enum_pattern.is_none_or(|pat| {
                e.get_name()
                    .is_some_and(|name| glob_match(&name, pat, case_sensitive))
            })
        })
        .filter_map(|e| Enum::try_from(e).ok())
        .collect()
}

/// Two-pass constant collection over [`TranslationUnit`].
///
/// Returns directly-evaluated constants and failed macro entities (for later
/// resolution with a lookup).
///
/// Collects constants that match filter settings.
fn collect_constants<'a>(
    tu: &'a TranslationUnit<'a>,
    header_filter: Option<&str>,
    const_pattern: Option<&str>,
    case_sensitive: bool,
    scoped_to_enum: bool,
) -> (Vec<Constant<'a>>, Vec<Entity<'a>>) {
    if scoped_to_enum {
        return (Vec::new(), Vec::new());
    }

    let entities: Vec<_> = iter_constants(tu)
        .filter(|e| matches_header(e, header_filter))
        .filter(|e| {
            const_pattern.is_none_or(|pat| {
                e.get_name()
                    .is_some_and(|name| glob_match(&name, pat, case_sensitive))
            })
        })
        .collect();

    let mut vars = Vec::new();
    let mut failed = Vec::new();

    for e in entities {
        match Constant::try_from(e) {
            Ok(c) => vars.push(c),
            Err(_) if e.get_kind() == EntityKind::MacroDefinition => failed.push(e),
            Err(_) => {}
        }
    }

    (vars, failed)
}

/// Iterate over enum declarations in a [`TranslationUnit`].
fn iter_enums<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::EnumDecl))
}

/// Iterate over constant declarations in a [`TranslationUnit`].
fn iter_constants<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity().get_children().into_iter().filter(|e| {
        matches!(
            e.get_kind(),
            EntityKind::VarDecl | EntityKind::MacroDefinition
        )
    })
}

/* ────────────────────────────── Macros lookup ───────────────────────────── */

/// Build a name -> value lookup table from all known constants (macros and vars).
fn build_lookup_table(enums: &[Enum], vars: &[Constant]) -> ConstLookup {
    let mut known = HashMap::new();
    for e in enums {
        for c in e.get_constants() {
            known.insert(c.get_name().to_string(), *c.get_value());
        }
    }
    for c in vars {
        known.insert(c.get_name().to_string(), *c.get_value());
    }
    known
}

/// Resolve failed macros that reference known constants by name.
fn resolve_macros<'a>(
    vars: &mut Vec<Constant<'a>>,
    known: &mut ConstLookup,
    failed: &[Entity<'a>],
) {
    for &e in failed {
        if let Ok(c) = Constant::try_from_macro_with_lookup(e, known) {
            known.insert(c.get_name().to_string(), *c.get_value());
            vars.push(c);
        }
    }
}

/* ──────────────────────────────── Printing ──────────────────────────────── */

/// Print enums first, then print non-scoped constants (vars, macros, ...)
fn print_display(
    enums: &[Enum],
    vars: &[Constant],
    const_pattern: Option<&str>,
    case_sensitive: bool,
    lookup: &ConstLookup,
) {
    for e in enums {
        match const_pattern {
            Some(pat) => print!("{}", e.display_filtered(pat, case_sensitive)),
            None => print!("{}", e.display()),
        }
    }

    if !vars.is_empty() {
        print!("{}", render_constants(vars, false, Some(lookup)));
    }
}

/// Collect enums, their contents, and non-scoped constants (vars, macros, ...) into a JSON,
/// and pretty-print it.
fn print_json(
    enums: &[Enum],
    vars: &[Constant],
    const_pattern: Option<&str>,
    case_sensitive: bool,
) -> Result<()> {
    #[derive(serde::Serialize)]
    struct Output<'a> {
        #[serde(skip_serializing_if = "Vec::is_empty")]
        enums: Vec<&'a Enum<'a>>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        constants: Vec<&'a Constant<'a>>,
    }

    let filtered_enums: Vec<&Enum> = enums
        .iter()
        .filter(|e| {
            const_pattern.is_none_or(|pat| {
                e.get_constants()
                    .iter()
                    .any(|c| glob_match(c.get_name(), pat, case_sensitive))
            })
        })
        .collect();

    let output = Output {
        enums: filtered_enums,
        constants: vars.iter().collect(),
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/* ────────────────────────────── Header match ────────────────────────────── */

fn matches_header(entity: &Entity, filter: Option<&str>) -> bool {
    let Some(filter) = filter else {
        return true;
    };

    entity
        .get_location()
        .and_then(|loc| loc.get_file_location().file)
        .is_some_and(|f| {
            f.get_path()
                .to_string_lossy()
                .to_lowercase()
                .ends_with(filter)
        })
}
