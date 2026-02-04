use anyhow::Result;
use bb::arch::Arch;
use bb::clang::Struct;
use bb::phnt::PhntVersion;
use bb::winsdk::SdkMode;
use clang::{Clang, Entity, EntityKind, Index, TranslationUnit, Unsaved};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bb",
    about = "Parse Windows SDK or PHNT embedded headers and extract struct information"
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

/// Header source configuration.
enum HeaderSource {
    WinSdk {
        sdk: bb::winsdk::SdkInfo,
        mode: SdkMode,
    },
    Phnt {
        sdk: bb::winsdk::SdkInfo, // Need SDK for base type includes
        version: PhntVersion,
        mode: SdkMode,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Determine header source.
    let source = get_header_source(&args)?;

    // Set up Clang.
    let clang_instance = Clang::new().expect("failed to initialize clang");
    let index = Index::new(&clang_instance, false, args.diagnostics);

    // Parse respective header.
    let tu = match &source {
        HeaderSource::WinSdk { sdk, mode } => {
            let clang_args = build_sdk_clang_args(&args, sdk);
            parse_winsdk(&index, sdk, &clang_args, *mode)?
        }
        HeaderSource::Phnt { sdk, version, mode } => {
            let clang_args = build_phnt_clang_args(&args, sdk);
            parse_phnt(&index, &clang_args, *version, *mode)?
        }
    };

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
    } else
    // WinDbg `dt` mode: print types as a pretty, rhs-only tree structure, that does expansion/recursion
    // inline, and also prints information like:
    //
    // - offsetof
    // - sizeof
    // - source as file:line:column
    {
        for s in structs {
            print!("{}", s.display(args.depth, args.field_name.as_deref()));
        }
    }

    Ok(())
}

/// Compute a header source from the command-line arguments.
///
/// - By default, WinSDK will be preferred, if nothing is explicitly specified instead.
/// - By default, the WinSDK version that will be used is inferred from environment, if nothing
/// is explicitly specified instead.
/// - By default, the PHNT version that will be used is Win11, if nothing is explicitly specified
/// instead.
fn get_header_source(args: &Args) -> anyhow::Result<HeaderSource> {
    match (&args.winsdk, &args.phnt) {
        (Some(_), Some(_)) => anyhow::bail!("Cannot use both --winsdk and --phnt"),
        (Some(version), None) => {
            let sdk = bb::winsdk::get_sdk_info(version.as_deref())?;
            if args.mode == SdkMode::Kernel {
                bb::winsdk::check_wdk_installed(&sdk)?;
            }

            Ok(HeaderSource::WinSdk {
                sdk,
                mode: args.mode,
            })
        }
        (None, Some(version)) => {
            // PHNT needs SDK include paths for base Windows types
            let sdk = bb::winsdk::get_sdk_info(None)?;
            Ok(HeaderSource::Phnt {
                sdk,
                version: (*version).unwrap_or_default(),
                mode: args.mode,
            })
        }
        (None, None) => {
            // Default to Windows SDK from environment
            let sdk = bb::winsdk::get_sdk_info(None)?;
            if args.mode == SdkMode::Kernel {
                bb::winsdk::check_wdk_installed(&sdk)?;
            }
            Ok(HeaderSource::WinSdk {
                sdk,
                mode: args.mode,
            })
        }
    }
}

fn build_sdk_clang_args(args: &Args, sdk: &bb::winsdk::SdkInfo) -> Vec<String> {
    let mut clang_args = vec!["-target".into(), args.arch.target_triple().into()];

    for subdir in ["shared", "um", "ucrt", "km"] {
        clang_args.push("-I".into());
        clang_args.push(sdk.get_include_dir().join(subdir).to_string_lossy().into());
    }

    clang_args.extend(args.arch.defines().iter().map(|&s| s.into()));

    clang_args
}

fn build_phnt_clang_args(args: &Args, sdk: &bb::winsdk::SdkInfo) -> Vec<String> {
    let mut clang_args = vec!["-target".into(), args.arch.target_triple().into()];

    // PHNT needs SDK include paths for base Windows types (ULONG, LIST_ENTRY, etc.)
    // Include "km" for kernel mode headers (ntdef.h)
    for subdir in ["shared", "um", "ucrt", "km"] {
        clang_args.push("-I".into());
        clang_args.push(sdk.get_include_dir().join(subdir).to_string_lossy().into());
    }

    clang_args.extend(args.arch.defines().iter().map(|&s| s.into()));

    clang_args
}

/// Build a synthetic header to parse for WinSDK using [`winsdk`] module.
fn parse_winsdk<'a>(
    index: &'a Index,
    sdk: &bb::winsdk::SdkInfo,
    args: &[String],
    mode: SdkMode,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = sdk.get_include_dir().join("__bb_synthetic.h");
    let unsaved = Unsaved::new(&synthetic_path, bb::winsdk::sdk_header(mode));

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}

/// Build a synthetic header to parse for PHNT using [`phnt`] module.
fn parse_phnt<'a>(
    index: &'a Index,
    args: &[String],
    version: PhntVersion,
    mode: SdkMode,
) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = PathBuf::from("__bb_phnt_synthetic.h");
    let header_content = bb::phnt::phnt_synthetic_header(version, mode == SdkMode::Kernel);
    let unsaved = Unsaved::new(&synthetic_path, &header_content);

    let tu = index
        .parser(synthetic_path.as_os_str())
        .arguments(&args_refs)
        .unsaved(&[unsaved])
        .keep_going(true)
        .parse()?;

    Ok(tu)
}

fn iter_structs<'a>(tu: &'a TranslationUnit<'a>) -> impl Iterator<Item = Entity<'a>> {
    tu.get_entity()
        .get_children()
        .into_iter()
        .filter(|e| matches!(e.get_kind(), EntityKind::StructDecl | EntityKind::ClassDecl))
}

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
            (Some(pattern), Some(name)) => {
                bb::matcher::glob_match(&name, pattern, self.case_sensitive)
            }
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
