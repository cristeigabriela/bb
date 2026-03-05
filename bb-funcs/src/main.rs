use std::collections::HashSet;

use anyhow::Result;
use bb_clang::Function;
use bb_cli::get_header_config;
use bb_funcs_lib::iter_funcs;
use clang::{Clang, Entity, EntityKind, Index};
use clap::Parser;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Parser, Debug)]
#[command(
    before_help = "Benowin Blanc (bb): Windows through a detective's lens...",
    name = "bb-consts",
    about = "Parse Windows SDK or PHNT embedded headers and extract functions."
)]
struct Args {
    #[command(flatten)]
    shared: bb_cli::SharedArgs,
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

    let mut set: HashSet<clang::CallingConvention> = HashSet::new();
    let ccs: Vec<_> = iter_funcs(&tu)
        .filter_map(|x| x.get_type())
        .filter_map(|x| x.get_calling_convention())
        .collect();
    dbg!(ccs.len());
    for entry in ccs {
        set.insert(entry);
    }

    dbg!(&set);

    let mut funcs = iter_funcs(&tu);
    let f = funcs.nth(1337).unwrap();
    dbg!(f.get_name());
    let t = f.get_type().unwrap();
    dbg!(t.get_calling_convention());

    // there can be other children like dllimport, typeref for return type, etc
    let args: Vec<Entity<'_>> = f.get_children();
    dbg!(&args);
    let all_children_kind = iter_funcs(&tu).flat_map(|x| x.get_children());
    let mut ek: HashSet<clang::EntityKind> = HashSet::new();
    for entry in all_children_kind {
        if entry.get_kind() == EntityKind::DllImport {
            dbg!(&entry);
        }
        ek.insert(entry.get_kind());
    }
    dbg!(&ek);

    let _f = Function::try_from(f).unwrap();
    //dbg!(&f);

    Ok(())
}
