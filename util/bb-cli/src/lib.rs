//! The unifying struct between all `bin` crates in the Benowin-Blanc project,
//! [`SharedArgs`] is responsible with handling [`bb_sdk`] and [`bb_clang`]
//! related responsibilities.
//!
//! To introduce in other `clap`-based CLIs, consider the following
//! implementation:
//!
//! ```rust
//! use bb_cli::get_header_config;
//! use clap::*;
//!
//! #[derive(Parser, Debug)]
//! #[command(name = "my-cli")]
//! struct Args {
//!     /// Will be flattened into [`Args`] for execution.
//!     #[command(flatten)]
//!     shared: bb_cli::SharedArgs,
//!
//!     // Other fields...
//! }
//!
//! // Parse arguments.
//! let args = Args::parse();
//!     
//! // Build header configuration.
//! let config = get_header_config(&args.shared);
//!
//! // ...
//! ```

use bb_sdk::{Arch, HeaderConfig, PhntVersion, SdkMode};
use bb_shared::suggest_closest;
use clap::{Args, arg};
use colored::Colorize;

/* ─────────────────────────────────── CLI ────────────────────────────────── */

#[derive(Args, Debug)]
pub struct SharedArgs {
    #[arg(long, help = "Use Windows SDK headers (optionally specify version)")]
    pub winsdk: Option<Option<String>>,

    #[arg(long, value_enum, help = "Use PHNT headers with specified version")]
    pub phnt: Option<Option<PhntVersion>>,

    #[arg(
        short,
        long,
        value_enum,
        default_value = "user",
        help = "Mode: user or kernel (defines _KERNEL_MODE for kernel)"
    )]
    pub mode: SdkMode,

    #[arg(
        short,
        long = "arch",
        value_enum,
        default_value = "amd64",
        help = "Architecture to target (supports cross-compilation)"
    )]
    pub arch: Arch,

    #[arg(long, help = "Show clang diagnostics")]
    pub diagnostics: bool,
}

/* ────────────────────────────── Suggestions ──────────────────────────────── */

/// Print a "did you mean?" hint to stderr when a non-glob pattern matches nothing.
///
/// Does nothing if `pattern` is `None` or contains a `*` wildcard.
pub fn print_suggestions<'a>(
    entity_kind: &str,
    pattern: Option<&str>,
    candidates: impl Iterator<Item = &'a str>,
) {
    let Some(pat) = pattern else { return };
    if pat.contains('*') {
        return;
    }

    let suggestions = suggest_closest(pat, candidates, 5);

    eprintln!(
        "{}{} no {} matching '{}'",
        "error".red().bold(),
        ":".bold(),
        entity_kind,
        pat.yellow(),
    );
    if !suggestions.is_empty() {
        eprintln!("\n  {}\n", "did you mean?".bold());
        for s in suggestions {
            eprintln!("    {}", s.green());
        }
        eprintln!();
    }
}

/* ─────────────────────────────────── SDK ────────────────────────────────── */

/// Build a [`HeaderConfig`] from the command-line arguments.
///
/// - By default, `WinSDK` will be preferred if nothing is explicitly specified.
/// - By default, the `WinSDK` version is inferred from environment.
/// - By default, the PHNT version is Win11.
pub fn get_header_config(args: &SharedArgs) -> anyhow::Result<HeaderConfig> {
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
