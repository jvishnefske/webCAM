//! Build script for board-support-stm32.
//!
//! Selects the correct `memory-{chip}.x` linker script based on the enabled
//! chip feature and gzip-compresses any frontend assets found in
//! `../hil-frontend/dist/` for embedding in the firmware.

use std::env;
use std::fs;
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
    let chip = if cfg!(feature = "stm32f401cc") {
        "stm32f401cc"
    } else if cfg!(feature = "stm32f411ce") {
        "stm32f411ce"
    } else if cfg!(feature = "stm32h743vi") {
        "stm32h743vi"
    } else {
        panic!(
            "No STM32 chip feature selected. Enable one of: stm32f401cc, stm32f411ce, stm32h743vi"
        );
    };

    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());

    // Copy chip-specific linker script to OUT_DIR as memory.x
    let memory_src = manifest_dir.join(format!("memory-{chip}.x"));
    fs::copy(&memory_src, out.join("memory.x"))
        .unwrap_or_else(|e| panic!("Failed to copy {}: {e}", memory_src.display()));
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory-{chip}.x");
    println!("cargo::rustc-check-cfg=cfg(has_frontend)");

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    // Gzip frontend assets for embedded static file serving.
    // STM32F401CC (256 KB flash) is too small for the ~180 KB compressed frontend.
    if chip != "stm32f401cc" {
        compress_frontend(out);
    }
}

/// Finds Trunk build output in `../hil-frontend/dist/` and gzip-compresses
/// each asset into `OUT_DIR` with a fixed name so `include_bytes!` can
/// reference them at compile time.
///
/// Sets `cargo:rustc-cfg=has_frontend` when all four asset types
/// (HTML, JS, WASM, CSS) are found and compressed.
fn compress_frontend(out: &PathBuf) {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let frontend_dist = manifest_dir.join("../hil-frontend/dist");
    println!("cargo:rerun-if-changed={}", frontend_dist.display());

    if !frontend_dist.is_dir() {
        println!("cargo:warning=No hil-frontend/dist/ found, building without static assets");
        return;
    }

    let entries = match fs::read_dir(&frontend_dist) {
        Ok(e) => e,
        Err(_) => {
            println!(
                "cargo:warning=Cannot read hil-frontend/dist/, building without static assets"
            );
            return;
        }
    };

    let mut found_html = false;
    let mut found_js = false;
    let mut found_wasm = false;
    let mut found_css = false;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        let (out_name, found_flag) = if name.ends_with(".html") {
            ("index.html.gz", &mut found_html)
        } else if name.ends_with(".js") {
            ("app.js.gz", &mut found_js)
        } else if name.ends_with(".wasm") {
            ("app_bg.wasm.gz", &mut found_wasm)
        } else if name.ends_with(".css") {
            ("style.css.gz", &mut found_css)
        } else {
            continue;
        };

        let data = fs::read(&path).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        encoder.write_all(&data).unwrap();
        let compressed = encoder.finish().unwrap();

        fs::write(out.join(out_name), &compressed).unwrap();
        *found_flag = true;

        println!(
            "cargo:warning=Frontend: {name} {} -> {} bytes ({:.0}%)",
            data.len(),
            compressed.len(),
            (compressed.len() as f64 / data.len() as f64) * 100.0,
        );
    }

    if found_html && found_js && found_wasm && found_css {
        println!("cargo:rustc-cfg=has_frontend");
    } else {
        println!("cargo:warning=Frontend dist/ incomplete, building without static assets");
    }
}
