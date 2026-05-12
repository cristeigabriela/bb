use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::Compression;
use flate2::write::GzEncoder;

/* ────────────────────────────────── Main ────────────────────────────────── */

fn main() {
    println!("cargo::rerun-if-env-changed=BB_SPARSE_JSON");
    println!("cargo::rerun-if-env-changed=BB_SPARSE_SDK_JSON");
    println!("cargo::rerun-if-env-changed=BB_SPARSE_DRIVER_JSON");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let sparse_dir = manifest_dir.join("sparse");

    build_mode(Mode::Sdk, &out_dir, &sparse_dir);
    build_mode(Mode::Driver, &out_dir, &sparse_dir);
}

/* ───────────────────────────────── Modes ────────────────────────────────── */

#[derive(Clone, Copy)]
enum Mode {
    Sdk,
    Driver,
}

impl Mode {
    fn gz_name(self) -> &'static str {
        match self {
            Self::Sdk => "sparse_sdk.json.gz",
            Self::Driver => "sparse_driver.json.gz",
        }
    }

    fn stamp_name(self) -> &'static str {
        match self {
            Self::Sdk => "sparse_sdk.stamp",
            Self::Driver => "sparse_driver.stamp",
        }
    }

    fn override_env(self) -> &'static [&'static str] {
        match self {
            // Legacy BB_SPARSE_JSON is honored for backwards compatibility
            // (it pointed at the SDK-mode JSON, which was the only one
            // bb-sparse used to embed).
            Self::Sdk => &["BB_SPARSE_SDK_JSON", "BB_SPARSE_JSON"],
            Self::Driver => &["BB_SPARSE_DRIVER_JSON"],
        }
    }

    fn workspace_json_name(self) -> &'static str {
        match self {
            Self::Sdk => "sdk-api.json",
            Self::Driver => "driver-docs.json",
        }
    }

    fn submodule_subpath(self) -> &'static str {
        match self {
            Self::Sdk => "sdk-api",
            Self::Driver => "windows-driver-docs-ddi",
        }
    }

    fn content_subpath(self) -> &'static str {
        match self {
            Self::Sdk => "sdk-api/sdk-api-src/content",
            Self::Driver => "windows-driver-docs-ddi/wdk-ddi-src/content",
        }
    }

    fn sparse_mode_flag(self) -> &'static str {
        match self {
            Self::Sdk => "sdk",
            Self::Driver => "driver",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Sdk => "sdk",
            Self::Driver => "driver",
        }
    }
}

/* ─────────────────────────── Per-mode pipeline ──────────────────────────── */

fn build_mode(mode: Mode, out_dir: &Path, sparse_dir: &Path) {
    let gz_path = out_dir.join(mode.gz_name());
    let stamp_path = out_dir.join(mode.stamp_name());

    // 1. Explicit override env vars always win.
    for var in mode.override_env() {
        if let Some(path) = env::var(var)
            .map(PathBuf::from)
            .ok()
            .filter(|p| p.exists())
        {
            println!("cargo::rerun-if-changed={}", path.display());
            compress_json(&path, &gz_path);
            return;
        }
    }

    // 2. Workspace-root or crate-root pre-generated file.
    let pre_generated = find_workspace_root()
        .map(|root| root.join(mode.workspace_json_name()))
        .filter(|p| p.exists());

    if let Some(path) = pre_generated {
        println!("cargo::rerun-if-changed={}", path.display());
        compress_json(&path, &gz_path);
        return;
    }

    // 3. Auto-generate from sparse submodule.
    let sparse_py = sparse_dir.join("sparse.py");
    let content_dir = sparse_dir.join(mode.content_subpath());

    if !sparse_py.exists() {
        println!(
            "cargo::warning=sparse submodule not found — embedding empty {} data. Run `git submodule update --init --recursive`",
            mode.label()
        );
        write_empty(&gz_path);
        return;
    }

    let submodule_root = sparse_dir.join(mode.submodule_subpath());

    // Initialize the relevant nested submodule if needed.
    if !content_dir.exists() {
        eprintln!(
            "bb-sparse: initializing nested submodule {} (this may take a while)...",
            mode.submodule_subpath()
        );
        let status = Command::new("git")
            .args(["submodule", "update", "--init", mode.submodule_subpath()])
            .current_dir(sparse_dir)
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                println!(
                    "cargo::warning=git submodule init for {} failed (exit {s}) — embedding empty data",
                    mode.label()
                );
                write_empty(&gz_path);
                return;
            }
            Err(e) => {
                println!(
                    "cargo::warning=git not available ({e}) — embedding empty {} data",
                    mode.label()
                );
                write_empty(&gz_path);
                return;
            }
        }
    }

    if !content_dir.exists() {
        println!(
            "cargo::warning={} content not found after submodule init — embedding empty data",
            mode.label()
        );
        write_empty(&gz_path);
        return;
    }

    // Skip regeneration when the underlying submodule hasn't moved.
    let current_rev = sub_rev(&submodule_root);
    if is_up_to_date(&gz_path, &stamp_path, current_rev.as_deref()) {
        eprintln!(
            "bb-sparse: {} submodule unchanged, reusing cached data",
            mode.label()
        );
        return;
    }

    // Locate a runner: `uv` is preferred (matches sparse's own setup);
    // plain Python is a fallback for hosts where uv isn't installed.
    let Some(runner) = find_runner() else {
        println!(
            "cargo::warning=neither `uv` nor python3 found on PATH — embedding empty {} data. Install uv (https://docs.astral.sh/uv/) or Python 3, or set {}",
            mode.label(),
            mode.override_env()[0]
        );
        write_empty(&gz_path);
        return;
    };

    let generated_json = out_dir.join(format!("sparse_{}_generated.json", mode.label()));

    eprintln!(
        "bb-sparse: running sparse ({} mode) to generate API metadata...",
        mode.label()
    );

    // `uv run` needs the project synced first; cheap and idempotent.
    if matches!(runner, Runner::Uv) {
        let _ = Command::new("uv")
            .args(["sync", "--frozen"])
            .current_dir(sparse_dir)
            .status();
    }

    let mut cmd = runner.command(sparse_dir);
    cmd.args([
        sparse_py.to_str().unwrap(),
        "--mode",
        mode.sparse_mode_flag(),
        "-o",
        generated_json.to_str().unwrap(),
        "--silent",
        content_dir.to_str().unwrap(),
    ]);
    cmd.current_dir(sparse_dir);

    let output = cmd.output();

    match output.as_ref().map(|o| o.status) {
        Ok(s) if s.success() && generated_json.exists() => {
            eprintln!(
                "bb-sparse: sparse {} mode completed successfully",
                mode.label()
            );
            compress_json(&generated_json, &gz_path);
            if let Some(ref rev) = current_rev {
                let _ = fs::write(&stamp_path, rev);
            }
        }
        Ok(s) => {
            let stderr = output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_default();
            println!(
                "cargo::warning=sparse {} mode failed (exit {s}) — embedding empty data",
                mode.label()
            );
            for line in stderr.lines().take(20) {
                if !line.trim().is_empty() {
                    println!("cargo::warning=  {line}");
                }
            }
            write_empty(&gz_path);
        }
        Err(e) => {
            println!(
                "cargo::warning=failed to run sparse runner ({e}) — embedding empty {} data",
                mode.label()
            );
            write_empty(&gz_path);
        }
    }
}

/* ───────────────────────────── Runner detection ─────────────────────────── */

enum Runner {
    Uv,
    Python(Vec<String>),
}

impl Runner {
    fn command(&self, sparse_dir: &Path) -> Command {
        match self {
            Self::Uv => {
                let mut cmd = Command::new("uv");
                cmd.args(["run", "--project", sparse_dir.to_str().unwrap(), "python"]);
                cmd
            }
            Self::Python(parts) => {
                let mut cmd = Command::new(&parts[0]);
                cmd.args(&parts[1..]);
                cmd
            }
        }
    }
}

fn find_runner() -> Option<Runner> {
    if Command::new("uv")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
    {
        return Some(Runner::Uv);
    }
    find_python().map(Runner::Python)
}

fn find_python() -> Option<Vec<String>> {
    if cfg!(windows)
        && Command::new("py")
            .args(["-3", "--version"])
            .output()
            .is_ok_and(|o| o.status.success())
    {
        return Some(vec!["py".into(), "-3".into()]);
    }
    for name in ["python3", "python"] {
        if Command::new(name)
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
        {
            return Some(vec![name.into()]);
        }
    }
    None
}

/* ───────────────────────────────── Helpers ──────────────────────────────── */

fn sub_rev(submodule_dir: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(submodule_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn is_up_to_date(gz_path: &Path, stamp_path: &Path, current_rev: Option<&str>) -> bool {
    let gz_ok = gz_path.metadata().is_ok_and(|m| m.len() > 0);
    if !gz_ok {
        return false;
    }
    let Some(rev) = current_rev else {
        return false;
    };
    fs::read_to_string(stamp_path).is_ok_and(|stored| stored.trim() == rev)
}

fn compress_json(json_path: &Path, gz_path: &Path) {
    let raw = fs::read(json_path).expect("failed to read sparse JSON");
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&raw).expect("failed to compress");
    let compressed = encoder.finish().expect("failed to finish compression");

    eprintln!(
        "bb-sparse: {} -> {} ({} bytes, {:.0}% reduction)",
        json_path.display(),
        gz_path.display(),
        compressed.len(),
        (1.0 - compressed.len() as f64 / raw.len() as f64) * 100.0,
    );

    fs::write(gz_path, &compressed).expect("failed to write compressed data");
}

fn write_empty(gz_path: &Path) {
    fs::write(gz_path, b"").expect("failed to write empty placeholder");
}

fn find_workspace_root() -> Option<PathBuf> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").ok()?);
    let mut dir = manifest.as_path();
    loop {
        let candidate = dir.join("Cargo.toml");
        if candidate.exists() {
            if let Ok(content) = fs::read_to_string(&candidate) {
                if content.contains("[workspace]") {
                    return Some(dir.to_path_buf());
                }
            }
        }
        dir = dir.parent()?;
    }
}
