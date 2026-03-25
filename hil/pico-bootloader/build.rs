//! Build script for the A/B partition bootloader.
//!
//! Copies `memory.x` to the output directory and sets linker arguments.
//! Does **not** use `-Tlink-rp.x` because embassy-boot provides its own
//! flash layout via linker symbols.

use std::env;
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

    println!("cargo:rustc-link-arg-bins=--nmagic");
    println!("cargo:rustc-link-arg-bins=-Tlink.x");
    println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
}
