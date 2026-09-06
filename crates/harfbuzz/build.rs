use std::{env, path::PathBuf};

fn main() {
    let root =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../vendor/harfbuzz/src");
    assert!(
        root.join("harfbuzz-subset.cc").exists(),
        "missing HarfBuzz sources: run git submodule update --init --recursive"
    );
    println!("cargo:rerun-if-changed={}", root.display());
    println!("cargo:rerun-if-changed=src/bridge.cc");
    let mut build = cc::Build::new();
    build
        .cpp(true)
        .std("c++17")
        .warnings(false)
        .include(&root)
        .file(root.join("harfbuzz-subset.cc"))
        .file("src/bridge.cc");
    // No pkg-config, FreeType, Fontconfig, CoreText, or dynamic HarfBuzz.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("musl") {
        build.cpp_link_stdlib("stdc++").cpp_link_stdlib_static(true);
    }
    // cc derives /MT vs /MD from Rust's target-feature=crt-static on MSVC.
    build.compile("assfonts_harfbuzz");
}
