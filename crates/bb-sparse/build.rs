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

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let gz_path = out_dir.join("sparse.json.gz");
    let stamp_path = out_dir.join("sparse.stamp");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // 1. BB_SPARSE_JSON env var always wins (explicit pre-generated file).
    if let Some(path) = env::var("BB_SPARSE_JSON")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists())
    {
        println!("cargo::rerun-if-changed={}", path.display());
        compress_json(&path, &gz_path);
        return;
    }

    // 2. Check for pre-generated sparse.json next to workspace root or crate.
    let pre_generated = find_workspace_root()
        .map(|root| root.join("sparse.json"))
        .filter(|p| p.exists())
        .or_else(|| {
            let p = manifest_dir.join("sparse.json");
            p.exists().then_some(p)
        });

    if let Some(path) = &pre_generated {
        println!("cargo::rerun-if-changed={}", path.display());
        compress_json(path, &gz_path);
        return;
    }

    // 3. Auto-generate from sparse submodule + sdk-api.
    let sparse_dir = manifest_dir.join("sparse");
    let sparse_py = sparse_dir.join("sparse.py");
    let sdk_api_dir = sparse_dir.join("sdk-api");
    let sdk_api_content = sdk_api_dir.join("sdk-api-src/content");

    if !sparse_py.exists() {
        println!(
            "cargo::warning=sparse submodule not found — embedding empty data. Run `git submodule update --init --recursive`"
        );
        write_empty(&gz_path);
        return;
    }

    // Initialize the nested sdk-api submodule if needed.
    if !sdk_api_content.exists() {
        eprintln!("bb-sparse: initializing sdk-api submodule (this may take a while)...");
        let status = Command::new("git")
            .args(["submodule", "update", "--init", "--recursive"])
            .current_dir(&sparse_dir)
            .status();

        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                println!(
                    "cargo::warning=git submodule init failed (exit {s}) — embedding empty sparse data"
                );
                write_empty(&gz_path);
                return;
            }
            Err(e) => {
                println!("cargo::warning=git not available ({e}) — embedding empty sparse data");
                write_empty(&gz_path);
                return;
            }
        }
    }

    if !sdk_api_content.exists() {
        println!(
            "cargo::warning=sdk-api content not found after submodule init — embedding empty sparse data"
        );
        write_empty(&gz_path);
        return;
    }

    // Check if we can skip regeneration: existing gz is non-empty and
    // sdk-api hasn't changed since last generation.
    let current_rev = sdk_api_rev(&sdk_api_dir);
    if is_up_to_date(&gz_path, &stamp_path, current_rev.as_deref()) {
        eprintln!("bb-sparse: sdk-api unchanged, reusing cached data");
        return;
    }

    // Run sparse.py to generate the JSON.
    eprintln!("bb-sparse: running sparse.py to generate API metadata...");
    let Some(python) = find_python() else {
        println!(
            "cargo::warning=python3 not found on PATH — embedding empty sparse data. Install Python 3 or set BB_SPARSE_JSON"
        );
        write_empty(&gz_path);
        return;
    };

    let generated_json = out_dir.join("sparse_generated.json");
    let mut cmd = Command::new(&python[0]);
    cmd.args(&python[1..]);
    cmd.args([
        sparse_py.to_str().unwrap(),
        "-o",
        generated_json.to_str().unwrap(),
        "--silent",
        sdk_api_content.to_str().unwrap(),
    ]);
    cmd.current_dir(&sparse_dir);

    let output = cmd.output();

    match output.as_ref().map(|o| o.status) {
        Ok(s) if s.success() && generated_json.exists() => {
            eprintln!("bb-sparse: sparse.py completed successfully");
            compress_json(&generated_json, &gz_path);
            // Write stamp so we can skip next time.
            if let Some(ref rev) = current_rev {
                let _ = fs::write(&stamp_path, rev);
            }
        }
        Ok(s) => {
            let stderr = output
                .as_ref()
                .map(|o| String::from_utf8_lossy(&o.stderr).to_string())
                .unwrap_or_default();
            // Show full traceback: one cargo::warning per line for visibility.
            println!("cargo::warning=sparse.py failed (exit {s}) — embedding empty data");
            for line in stderr.lines().take(20) {
                if !line.trim().is_empty() {
                    println!("cargo::warning=  {line}");
                }
            }
            write_empty(&gz_path);
        }
        Err(e) => {
            println!("cargo::warning=failed to run python ({e}) — embedding empty sparse data");
            write_empty(&gz_path);
        }
    }
}

/* ───────────────────────────────── Helpers ──────────────────────────────── */

/// Get the current git rev of the sdk-api submodule.
fn sdk_api_rev(sdk_api_dir: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(sdk_api_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Check if the existing compressed data is still valid:
/// - gz exists and is non-empty
/// - stamp file contains the same rev as current sdk-api HEAD
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
        "bb-sparse: compressed {} -> {} bytes ({:.0}% reduction)",
        raw.len(),
        compressed.len(),
        (1.0 - compressed.len() as f64 / raw.len() as f64) * 100.0,
    );

    fs::write(gz_path, &compressed).expect("failed to write compressed data");
}

fn write_empty(gz_path: &Path) {
    fs::write(gz_path, b"").expect("failed to write empty placeholder");
}

fn find_python() -> Option<Vec<String>> {
    // On Windows, `py -3` uses the Python launcher to find the latest 3.x.
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
