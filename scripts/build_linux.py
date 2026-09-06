"""The Linux release entry point. Requires the pinned tools; never installs them."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def capture(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True).strip()


def main():
    pins = json.loads((ROOT / "scripts/linux-toolchain.json").read_text())
    zig = os.environ.get("CARGO_ZIGBUILD_ZIG_PATH") or shutil.which("zig")
    if not zig:
        import ziglang
        zig = str(Path(ziglang.__file__).parent / "zig")
    zig = str(Path(zig).resolve())
    if capture(zig, "version") != pins["ziglang"]:
        sys.exit(f"Zig version mismatch; required {pins['ziglang']}")
    if capture("cargo-zigbuild", "--version") != f"cargo-zigbuild {pins['cargo-zigbuild']}":
        sys.exit(f"cargo-zigbuild version mismatch; required {pins['cargo-zigbuild']}")
    rust = capture("rustc", f"+{pins['rust']}", "--version")
    if rust.split()[1] != pins["rust"]:
        sys.exit("Rust version mismatch")
    target_dir = ROOT / "target/zigbuild"
    env = dict(os.environ, CARGO_TARGET_DIR=str(target_dir),
               CARGO_ZIGBUILD_ZIG_PATH=zig, RUSTFLAGS="-C target-feature=+crt-static")
    # Cargo config/RUSTFLAGS can otherwise silently bypass Zig or crt-static.
    if os.environ.get("CARGO_ENCODED_RUSTFLAGS"):
        sys.exit("Unset CARGO_ENCODED_RUSTFLAGS before building the release")
    subprocess.run(["cargo", f"+{pins['rust']}", "zigbuild", "--release", "--locked",
                    "--target", pins["target"], "-p", "assfonts-cli"],
                   cwd=ROOT, env=env, check=True)
    binary = target_dir / pins["target"] / "release/assfonts-rs"
    # Carry the toolchain runtime notices alongside the statically linked file.
    zig_lib = Path(json.loads(capture(zig, "env"))["lib_dir"])
    notices = binary.with_name("runtime-licenses")
    notices.mkdir(exist_ok=True)
    for relative, name in (
        ("libcxx/LICENSE.TXT", "libcxx-LICENSE.txt"),
        ("libcxxabi/LICENSE.TXT", "libcxxabi-LICENSE.txt"),
        ("libunwind/LICENSE.TXT", "libunwind-LICENSE.txt"),
        ("libc/musl/COPYRIGHT", "musl-COPYRIGHT"),
    ):
        shutil.copyfile(zig_lib / relative, notices / name)
    shutil.copyfile(Path(zig).parent / "LICENSE", notices / "zig-LICENSE")
    manifest = {
        "toolchain": pins, "rustc": rust,
        "git_revision": capture("git", "rev-parse", "HEAD"),
        "working_tree_dirty": bool(capture("git", "status", "--porcelain")),
        "harfbuzz_revision": capture("git", "-C", "vendor/harfbuzz", "rev-parse", "HEAD"),
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "binary": binary.name,
    }
    binary.with_name("build-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(binary)


if __name__ == "__main__":
    main()
