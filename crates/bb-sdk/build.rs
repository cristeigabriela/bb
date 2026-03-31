use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/* ────────────────────────────────── Main ────────────────────────────────── */

/// Resolve and copy the PHNT header to `OUT_DIR` for `include_str!`.
///
/// Resolution order:
/// 1. `BB_PHNT_HEADER` env var → explicit custom header path
/// 2. `phnt.h` next to this crate → local override
/// 3. Generate from the phnt-single-header submodule
fn main() {
    println!("cargo::rerun-if-env-changed=BB_PHNT_HEADER");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let out_phnt = out_dir.join("phnt.h");
    let stamp_path = out_dir.join("phnt.stamp");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    // 1. BB_PHNT_HEADER env var.
    if let Some(path) = env::var("BB_PHNT_HEADER")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists())
    {
        println!("cargo::rerun-if-changed={}", path.display());
        eprintln!("bb-sdk: using BB_PHNT_HEADER={}", path.display());
        fs::copy(&path, &out_phnt).expect("failed to copy BB_PHNT_HEADER");
        return;
    }

    // 2. phnt.h next to the crate.
    let local = manifest_dir.join("phnt.h");
    if local.exists() {
        println!("cargo::rerun-if-changed={}", local.display());
        fs::copy(&local, &out_phnt).expect("failed to copy local phnt.h");
        return;
    }

    // 3. Generate from phnt-single-header submodule.
    let phnt_dir = manifest_dir.join("phnt");
    let amalgamate_py = phnt_dir.join("amalgamate.py");
    let generated = phnt_dir.join("out/phnt.h");

    // Init the submodule if it's not there.
    if !amalgamate_py.exists() {
        eprintln!("bb-sdk: initializing phnt submodule...");
        run_or_warn(
            Command::new("git")
                .args(["submodule", "update", "--init", "crates/bb-sdk/phnt"])
                .current_dir(find_workspace_root().unwrap_or(manifest_dir)),
            "git submodule init for phnt",
        );
    }

    assert!(amalgamate_py.exists(), 
            "bb-sdk: phnt submodule not found at {}\n\
             hint: run `git submodule update --init crates/bb-sdk/phnt`\n\
             or set BB_PHNT_HEADER to a custom phnt.h path",
            phnt_dir.display()
        );

    // If the generated output already exists and is up-to-date, reuse it.
    let si_dir = phnt_dir.join("systeminformer");
    let current_rev = submodule_rev(&si_dir);
    if generated.exists() && is_up_to_date(&out_phnt, &stamp_path, current_rev.as_deref()) {
        eprintln!("bb-sdk: phnt unchanged, reusing cached header");
        return;
    }

    // If out/phnt.h already exists (pre-generated or from a previous run), use it.
    if generated.exists() {
        eprintln!("bb-sdk: using pre-generated {}", generated.display());
        println!("cargo::rerun-if-changed={}", generated.display());
        fs::copy(&generated, &out_phnt).expect("failed to copy generated phnt.h");
        if let Some(ref rev) = current_rev {
            let _ = fs::write(&stamp_path, rev);
        }
        return;
    }

    // Need to generate — init systeminformer submodule + run amalgamate.py.
    if !si_dir.join("phnt").exists() {
        eprintln!("bb-sdk: initializing systeminformer submodule (this may take a while)...");
        run_or_warn(
            Command::new("git")
                .args(["submodule", "update", "--init", "systeminformer"])
                .current_dir(&phnt_dir),
            "git submodule init for systeminformer",
        );
    }

    // Pre-download cpp-amalgamate.exe if missing or empty.
    // amalgamate.py uses urllib which can fail silently on some Python versions.
    let cpp_amalgamate = phnt_dir.join("cpp-amalgamate.exe");
    let needs_download =
        !cpp_amalgamate.exists() || cpp_amalgamate.metadata().is_ok_and(|m| m.len() == 0);
    if needs_download {
        eprintln!("bb-sdk: downloading cpp-amalgamate.exe...");
        let url = "https://github.com/Felerius/cpp-amalgamate/releases/download/1.0.1/cpp-amalgamate-x86_64-pc-windows-gnu.exe";
        if cpp_amalgamate.exists() {
            let _ = fs::remove_file(&cpp_amalgamate);
        }
        // Use curl or powershell — both handle GitHub redirects correctly.
        let dl_ok = Command::new("curl")
            .args(["-sL", "-o", cpp_amalgamate.to_str().unwrap(), url])
            .status()
            .is_ok_and(|s| s.success());
        if !dl_ok {
            // Fallback to powershell on Windows.
            let _ = Command::new("powershell")
                .args([
                    "-Command",
                    &format!(
                        "Invoke-WebRequest -Uri '{}' -OutFile '{}' -UseBasicParsing",
                        url,
                        cpp_amalgamate.display()
                    ),
                ])
                .status();
        }
    }

    eprintln!("bb-sdk: running amalgamate.py to generate phnt.h...");
    let Some(python) = find_python() else {
        panic!(
            "bb-sdk: python3 not found on PATH\n\
             hint: install Python 3, or run amalgamate.py manually, \
             or set BB_PHNT_HEADER"
        );
    };

    let output = Command::new(&python[0])
        .args(&python[1..])
        .arg(amalgamate_py.to_str().unwrap())
        .current_dir(&phnt_dir)
        .output();

    match &output {
        Ok(o) if o.status.success() && generated.exists() => {
            eprintln!("bb-sdk: amalgamate.py completed successfully");
            fs::copy(&generated, &out_phnt).expect("failed to copy generated phnt.h");
            if let Some(ref rev) = current_rev {
                let _ = fs::write(&stamp_path, rev);
            }
        }
        Ok(o) => {
            for line in String::from_utf8_lossy(&o.stderr).lines() {
                eprintln!("bb-sdk: py: {line}");
            }
            panic!("bb-sdk: amalgamate.py failed (exit {})", o.status);
        }
        Err(e) => panic!("bb-sdk: failed to run python ({e})"),
    }
}

/* ───────────────────────────────── Helpers ──────────────────────────────── */

fn run_or_warn(cmd: &mut Command, desc: &str) {
    match cmd.status() {
        Ok(s) if s.success() => {}
        Ok(s) => eprintln!("bb-sdk: {desc} failed (exit {s}), continuing..."),
        Err(e) => eprintln!("bb-sdk: {desc}: {e}, continuing..."),
    }
}

fn submodule_rev(dir: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn is_up_to_date(out_phnt: &Path, stamp_path: &Path, current_rev: Option<&str>) -> bool {
    if !out_phnt.exists() {
        return false;
    }
    let Some(rev) = current_rev else {
        return false;
    };
    fs::read_to_string(stamp_path).is_ok_and(|stored| stored.trim() == rev)
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
