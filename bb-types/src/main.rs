use anyhow::Result;
use bb_clang::Struct;
use bb_sdk::{Arch, HeaderConfig, PhntVersion, SdkMode};
use bb_shared::glob_match;
use clang::{Clang, Entity, EntityKind, Index, TranslationUnit};
use clap::Parser;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-types",
    about = "Parse Windows SDK or PHNT embedded headers and extract struct information."
)]
struct Args {
    // Header source selection (mutually exclusive via validation)
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

    // Common options
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
        long = "struct",
        help = "Struct name pattern (supports * wildcard)"
    )]
    struct_name: Option<String>,

    #[arg(
        short,
        long = "field",
        help = "Field name pattern (supports * wildcard)"
    )]
    field_name: Option<String>,

    #[arg(short, long = "case-sensitive", help = "Case-sensitive matching")]
    case_sensitive: bool,

    #[arg(
        short,
        long = "depth",
        default_value = "0",
        help = "Recursion depth for nested types"
    )]
    depth: usize,

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

    // Parse headers.
    let tu = config.parse(&index, false)?;

    let filter = StructFilter::new(&args);
    let structs: Vec<_> = iter_structs(&tu)
        .filter(|e| filter.matches(e))
        .filter_map(|e| Struct::try_from(e).ok())
        .collect();

    // JSON mode: print an array of structures and their fields. Respects expansion/recursion depth.
    if args.json {
        let mut all_structs: Vec<&Struct> = Vec::new();
        let nested: Vec<_> = structs
            .iter()
            .flat_map(|s| s.extract_nested_types(args.depth))
            .collect();
        all_structs.extend(structs.iter());
        all_structs.extend(nested.iter());
        println!("{}", serde_json::to_string_pretty(&all_structs)?);
    } else {
        // WinDbg `dt` mode: print types as a pretty, rhs-only tree structure
        for s in structs {
            print!("{}", s.display(args.depth, args.field_name.as_deref()));
        }
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
        (Some(version), None) => {
            // WinSDK with optional version override
            match version {
                Some(v) => HeaderConfig::winsdk_version(v, args.arch, args.mode),
                None => HeaderConfig::winsdk(args.arch, args.mode),
            }
        }
        (None, Some(version)) => {
            // PHNT with optional version
            let phnt_version = (*version).unwrap_or_default();
            HeaderConfig::phnt(args.arch, phnt_version, args.mode)
        }
        (None, None) => {
            // Default to Windows SDK
            HeaderConfig::winsdk(args.arch, args.mode)
        }
    }
}

/* ────────────────────────────────── Iter ────────────────────────────────── */

/// Iterate over struct declarations in a [`TranslationUnit`].
pub fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::StructDecl | EntityKind::ClassDecl))
}

/* ────────────────────────────────── Match ───────────────────────────────── */

struct StructFilter {
    name_pattern: Option<String>,
    header_filter: Option<String>,
    case_sensitive: bool,
}

impl StructFilter {
    fn new(args: &Args) -> Self {
        Self {
            name_pattern: args.struct_name.clone(),
            header_filter: args.filter.as_ref().map(|h| h.to_lowercase()),
            case_sensitive: args.case_sensitive,
        }
    }

    fn matches(&self, entity: &Entity) -> bool {
        self.matches_name(entity) && self.matches_header(entity)
    }

    fn matches_name(&self, entity: &Entity) -> bool {
        match (&self.name_pattern, entity.get_name()) {
            (Some(pattern), Some(name)) => glob_match(&name, pattern, self.case_sensitive),
            (Some(_), None) => false,
            (None, _) => true,
        }
    }

    fn matches_header(&self, entity: &Entity) -> bool {
        let Some(filter) = &self.header_filter else {
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
}
