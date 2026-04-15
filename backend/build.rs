//! build.rs — Windrose Server Manager build script.
//!
//! Responsibilities
//! ────────────────
//! 1. Locate the `frontend/` directory (sibling of this crate).
//! 2. Run `npm ci && npm run build` to compile the React/Vite application
//!    into `../static/` so that `rust-embed` can embed those files at
//!    compile time.
//! 3. Emit `cargo:rerun-if-changed` directives so Cargo only re-runs this
//!    script when the frontend source or dependencies actually change.
//!
//! If Node/npm is not installed the build will fail with a clear message
//! instructing the developer to install the prerequisites.  A release build
//! must complete the full chain; there is no runtime filesystem fallback.

use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Resolve paths relative to this Cargo manifest.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .expect("backend/ should have a parent directory");

    let frontend_dir = repo_root.join("frontend");
    let static_dir = repo_root.join("static");

    // ── Tell Cargo when to re-run this script ────────────────────────────
    // Re-run when any frontend source file changes.
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("src").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("package.json").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("vite.config.ts").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        frontend_dir.join("index.html").display()
    );
    // Also re-run if the static output directory is missing so a clean
    // checkout always triggers a fresh frontend build.
    println!("cargo:rerun-if-changed={}", static_dir.display());

    // ── Check whether the frontend directory exists ───────────────────────
    if !frontend_dir.exists() {
        // No frontend source — assume the static/ dir was pre-populated.
        assert!(
            static_dir.join("index.html").exists(),
            "Frontend directory `{}` not found and `static/index.html` is \
             also missing. Please either:\n  \
               (a) Clone the full repository (includes frontend/ source), or\n  \
               (b) Place a pre-built frontend into the `static/` directory.",
            frontend_dir.display()
        );
        println!("cargo:warning=frontend/ not found — using pre-built static/ assets.");
        return;
    }

    // ── Detect npm ────────────────────────────────────────────────────────
    let npm_cmd = if cfg!(windows) { "npm.cmd" } else { "npm" };
    let npm_present = Command::new(npm_cmd)
        .arg("--version")
        .output()
        .is_ok();

    if !npm_present {
        // If static/index.html already exists (e.g. committed pre-built
        // assets) we can proceed without npm.
        if static_dir.join("index.html").exists() {
            println!(
                "cargo:warning=npm not found — using existing pre-built static/ assets. \
                 Install Node.js to enable automatic frontend rebuilds."
            );
            return;
        }
        panic!(
            "npm is required to build the Windrose frontend but was not found in PATH.\n\
             Please install Node.js 18+ from https://nodejs.org/ and re-run the build."
        );
    }

    // ── npm ci (install / restore exact lockfile) ─────────────────────────
    let ci_status = Command::new(npm_cmd)
        .args(["ci", "--prefer-offline"])
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to spawn `npm ci`");

    assert!(
        ci_status.success(),
        "`npm ci` failed (exit code {:?}). Check the output above for details.",
        ci_status.code()
    );

    // ── npm run build ─────────────────────────────────────────────────────
    let build_status = Command::new(npm_cmd)
        .args(["run", "build"])
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to spawn `npm run build`");

    assert!(
        build_status.success(),
        "`npm run build` failed (exit code {:?}). Check the output above for details.",
        build_status.code()
    );

    // ── Sanity check ──────────────────────────────────────────────────────
    assert!(
        static_dir.join("index.html").exists(),
        "`npm run build` completed but `static/index.html` was not produced. \
         Check vite.config.ts outDir configuration."
    );

    println!("cargo:warning=Frontend build complete — assets embedded into binary.");
}
