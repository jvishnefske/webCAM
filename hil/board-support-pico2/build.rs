//! Build script for board-support-pico2.
//!
//! Copies the `memory.x` linker script to the output directory and
//! gzip-compresses any frontend assets found in `../hil-frontend/dist/`.

use std::env;
use std::fs;
use std::fs::File;
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo::rustc-check-cfg=cfg(has_frontend)");

    // Write a linker script fragment to place the IMAGE_DEF block at the
    // very start of flash, before the vector table.
    let link_rp = out.join("link-rp.x");
    File::create(&link_rp)
        .unwrap()
        .write_all(
            b"SECTIONS {\n\
              \x20\x20.start_block :\n\
              \x20\x20{\n\
              \x20\x20\x20\x20KEEP(*(.start_block));\n\
              \x20\x20} > IMAGE_DEF\n\
              }\n",
        )
        .unwrap();

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tlink-rp.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");

    compress_frontend(out);
}

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
