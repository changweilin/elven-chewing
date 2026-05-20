//! Stage the real libchewing dictionaries into `OUT_DIR` so `src/lib.rs` can
//! `include_bytes!` them into the wasm bundle.
//!
//! The browser has no filesystem, so the engine cannot load `.dat` files at
//! runtime the way the desktop IME does — we compile them in instead. The
//! source images come from the libchewing-data release that `cargo xtask
//! download` unpacks into `build/installer/Dictionary/`. Staging through
//! `OUT_DIR` keeps the `include_bytes!` paths stable and turns a missing
//! download into a clear, actionable build error rather than a cryptic one.

use std::path::PathBuf;
use std::{env, fs};

/// Images embedded by `src/lib.rs`. `word.dat` (single-char readings) and
/// `tsi.dat` (phrases) give the sandbox desktop-equivalent vocabulary;
/// `symbols.dat` backs the `` ` `` symbol menu.
const DICTS: [&str; 3] = ["word.dat", "tsi.dat", "symbols.dat"];

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set"));
    // Default to the workspace's downloaded dictionary dir; allow an override
    // for unusual layouts or CI that stages the dicts elsewhere.
    println!("cargo:rerun-if-env-changed=CHEWING_DICT_DIR");
    let dict_dir = match env::var_os("CHEWING_DICT_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => manifest.join("../../build/installer/Dictionary"),
    };

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR set"));
    for name in DICTS {
        let src = dict_dir.join(name);
        if !src.exists() {
            panic!(
                "missing dictionary `{}`.\n\
                 The web demo embeds the real libchewing dictionaries; run \
                 `cargo xtask download` to fetch them into \
                 `build/installer/Dictionary/`, or point CHEWING_DICT_DIR at a \
                 directory containing {DICTS:?}.\n\
                 looked in: {}",
                name,
                dict_dir.display(),
            );
        }
        fs::copy(&src, out_dir.join(name))
            .unwrap_or_else(|e| panic!("failed to stage {}: {e}", src.display()));
        println!("cargo:rerun-if-changed={}", src.display());
    }
}
