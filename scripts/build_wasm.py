"""Build an experimental Emscripten single module with pinned tools; never installs them."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def capture(*args, env=None):
    return subprocess.check_output(args, cwd=ROOT, env=env, text=True).strip()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emsdk", type=Path,
                        default=Path(os.environ.get("EMSDK", ROOT / ".validation/emsdk")))
    args = parser.parse_args()
    sdk = args.emsdk.resolve()
    pins = json.loads((ROOT / "scripts/wasm-toolchain.json").read_text())
    if os.environ.get("CARGO_ENCODED_RUSTFLAGS"):
        raise RuntimeError("Unset CARGO_ENCODED_RUSTFLAGS before building WASM")
    if capture("git", "-C", str(sdk), "rev-parse", "HEAD") != pins["emsdk_commit"]:
        raise RuntimeError("Emsdk revision mismatch")
    if capture("git", "-C", "vendor/harfbuzz", "rev-parse", "HEAD") != pins["harfbuzz_commit"]:
        raise RuntimeError("HarfBuzz revision mismatch")
    rust = capture("rustc", f"+{pins['rust']}", "--version")
    if rust.split()[1] != pins["rustc_version"] or pins["rustc_commit"] not in rust:
        raise RuntimeError("Rust compiler version mismatch")
    # The SDK's supported environment script selects its matching clang, linker
    # and Node. Pass the path as an argument, not interpolated shell source.
    raw = subprocess.check_output([
        "bash", "-c", 'set -e; source "$1/emsdk_env.sh" >/dev/null; env -0', "bash", str(sdk),
    ], cwd=ROOT)
    env = dict(item.decode().split("=", 1) for item in raw.split(b"\0") if item)
    emcc = capture("emcc", "--version", env=env).splitlines()[0]
    if not re.search(rf"\b{re.escape(pins['emsdk'])}\b", emcc):
        raise RuntimeError("Active Emscripten compiler version mismatch")
    exports = ["_af_alloc", "_af_free", "_af_engine_new", "_af_engine_destroy",
               "_af_add_font", "_af_process", "_af_set_missing_glyph_policy", "_af_result_ptr", "_af_result_len"]
    flags = ["-C", "panic=abort"]
    for setting in [
        "MODULARIZE=1", "EXPORT_ES6=1", "ENVIRONMENT=web,worker", "INVOKE_RUN=0",
        "FILESYSTEM=0", 'EXPORTED_RUNTIME_METHODS=["HEAPU8"]', "ALLOW_MEMORY_GROWTH=1",
        "MAXIMUM_MEMORY=2147483648", "STACK_SIZE=2097152",
        "EXPORTED_FUNCTIONS=" + json.dumps(exports, separators=(",", ":")),
    ]:
        flags.extend(["-C", f"link-arg=-s{setting}"])
    target = ROOT / "target/emscripten"
    env.update(CARGO_TARGET_DIR=str(target), CARGO_PROFILE_RELEASE_LTO="false",
               CXXFLAGS="-fno-exceptions -fno-rtti", RUSTFLAGS=" ".join(flags))
    subprocess.run(["cargo", f"+{pins['rust']}", "build", "--locked",
                    "-Zbuild-std=std,panic_abort", "--release", "--target", pins["target"],
                    "-p", "assfonts-wasm"], cwd=ROOT, env=env, check=True)
    output = ROOT / "target/wasm"
    output.mkdir(parents=True, exist_ok=True)
    release = target / pins["target"] / "release"
    shutil.copyfile(release / "assfonts-wasm.js", output / "engine.js")
    shutil.copyfile(release / "assfonts_wasm.wasm", output / "assfonts-wasm.wasm")
    if (output / "assfonts-wasm.wasm").read_bytes()[:8] != b"\0asm\x01\0\0\0":
        raise RuntimeError("Unexpected WASM header")
    notices = output / "licenses"
    notices.mkdir(exist_ok=True)
    for source, name in [
        (ROOT / "vendor/harfbuzz/COPYING", "harfbuzz-COPYING"),
        (ROOT / "crates/fonts/data/LICENSE-UNICODE", "cp936-LICENSE-UNICODE"),
        (sdk / "upstream/emscripten/LICENSE", "emscripten-LICENSE"),
        (sdk / "LICENSE", "emsdk-LICENSE"),
    ]:
        shutil.copyfile(source, notices / name)
    for relative, name in [
        ("libcxx/LICENSE.TXT", "libcxx-LICENSE.txt"),
        ("libcxxabi/LICENSE.TXT", "libcxxabi-LICENSE.txt"),
        ("libunwind/LICENSE.TXT", "libunwind-LICENSE.txt"),
        ("compiler-rt/LICENSE.TXT", "compiler-rt-LICENSE.txt"),
        ("libc/musl/COPYRIGHT", "musl-COPYRIGHT"),
    ]:
        shutil.copyfile(sdk / "upstream/emscripten/system/lib" / relative, notices / name)
    manifest = {
        "experimental": True, "toolchain": pins, "rustc": rust, "emcc": emcc,
        "git_revision": capture("git", "rev-parse", "HEAD"),
        "working_tree_dirty": bool(capture("git", "status", "--porcelain")),
        "cargo_lock_sha256": digest(ROOT / "Cargo.lock"),
        "artifacts": {name: {"sha256": digest(output / name), "bytes": (output / name).stat().st_size}
                      for name in ("engine.js", "assfonts-wasm.wasm")},
        "maximum_memory_bytes": 2147483648, "stack_bytes": 2097152,
    }
    (output / "build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(output)


if __name__ == "__main__":
    main()
