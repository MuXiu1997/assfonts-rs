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
    println!("cargo:rerun-if-changed=src/mort.hh");
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
        // Linux release builds use Zig's target libc++; crt-static selects
        // its static runtime. Never mix in a host GCC libstdc++ archive.
        build.cpp_link_stdlib("c++");
    }
    // cc derives /MT vs /MD from Rust's target-feature=crt-static on MSVC.
    build.compile("assfonts_harfbuzz");
}
