use anyhow::Result;
use bb::arch::Arch;
use bb::clang::Struct;
use bb::winsdk::SdkInfo;
use clang::{Clang, Entity, EntityKind, Index, TranslationUnit, Unsaved};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "bb",
    about = "Parse Windows SDK headers and extract struct information"
)]
struct Args {
    #[arg(
        short = 'H',
        long = "filter",
        help = "Filter by header file (e.g., winternl.h)"
    )]
    filter: Option<String>,

    #[arg(short, long = "arch", value_enum, default_value = "amd64")]
    arch: Arch,

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

    #[arg(
        short,
        long = "case-sensitive",
        default_value = "false",
        help = "Case-sensitive matching"
    )]
    case_sensitive: bool,

    #[arg(long = "cpp", default_value = "false", help = "Parse as C++")]
    cpp: bool,

    #[arg(long = "std", default_value = "c++17", help = "C++ standard version")]
    std: String,

    #[arg(
        short,
        long = "depth",
        default_value = "0",
        help = "Recursion depth for nested types"
    )]
    depth: usize,

    #[arg(
        short = 'V',
        long = "sdk-version",
        help = "Windows SDK version (e.g., 10.0.26100.0)"
    )]
    sdk_version: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let sdk = bb::winsdk::get_sdk_info(args.sdk_version.as_deref())?;

    let clang_instance = Clang::new().expect("failed to initialize clang");
    let index = Index::new(&clang_instance, false, false);

    let clang_args = build_clang_args(&args, &sdk);
    let tu = parse_sdk(&index, &sdk, &clang_args)?;

    let filter = StructFilter::new(&args);
    for entity in iter_structs(&tu) {
        if filter.matches(&entity) {
            if let Ok(s) = Struct::try_from(entity) {
                print!("{}", s.display(args.depth, args.field_name.as_deref()));
            }
        }
    }

    Ok(())
}

fn build_clang_args(args: &Args, sdk: &SdkInfo) -> Vec<String> {
    let mut clang_args = vec!["-target".into(), args.arch.target_triple().into()];

    for subdir in ["shared", "um", "ucrt", "km"] {
        clang_args.push("-I".into());
        clang_args.push(sdk.get_include_dir().join(subdir).to_string_lossy().into());
    }

    clang_args.extend(args.arch.defines().iter().map(|&s| s.into()));

    if args.cpp {
        clang_args.extend(["-x".into(), "c++".into(), format!("-std={}", args.std)]);
    }

    clang_args
}

fn parse_sdk<'a>(index: &'a Index, sdk: &SdkInfo, args: &[String]) -> Result<TranslationUnit<'a>> {
    let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let synthetic_path = sdk.get_include_dir().join("__bb_synthetic.h");
    let unsaved = Unsaved::new(&synthetic_path, bb::winsdk::sdk_header());

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
        .filter(|e| e.get_name().is_some())
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
