//! Builds the crate in `builtins/` for wasm32 and leaves `builtins.wasm` in
//! `OUT_DIR`, where the linker of the compiler includes it from.

use std::path::PathBuf;
use std::process::Command;

const TARGET: &str = "wasm32-unknown-unknown";

fn main() {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"))
        .join("builtins");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it"));
    for path in ["src", "Cargo.toml", "Cargo.lock", ".cargo/config.toml"] {
        println!("cargo:rerun-if-changed={}", dir.join(path).display());
    }

    let target_dir = out.join("target");
    let cargo = std::env::var_os("CARGO").expect("cargo sets it");
    let mut command = Command::new(cargo);
    command
        .current_dir(&dir)
        .args(["build", "--release", "--target", TARGET, "--target-dir"])
        .arg(&target_dir);
    // What the outer build asks for, such as the target of a release or the
    // driver of clippy, is not for this build.
    for var in [
        "CARGO_BUILD_TARGET",
        "CARGO_TARGET_DIR",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        command.env_remove(var);
    }
    let status = command.status().expect("cargo should run");
    assert!(
        status.success(),
        "could not build compiler/builtins for {TARGET}"
    );

    let wasm = target_dir
        .join(TARGET)
        .join("release")
        .join("builtins.wasm");
    std::fs::copy(&wasm, out.join("builtins.wasm")).expect("cargo should write the wasm");
}
