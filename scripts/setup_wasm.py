"""Explicitly install the pinned WASM tools. Does not change the default Rust toolchain."""
import argparse
import json
import os
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emsdk", type=Path,
                        default=Path(os.environ.get("EMSDK", ROOT / ".validation/emsdk")))
    args = parser.parse_args()
    sdk = args.emsdk.resolve()
    pins = json.loads((ROOT / "scripts/wasm-toolchain.json").read_text())
    if not sdk.exists():
        sdk.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(["git", "clone", "--depth", "1", "--branch", pins["emsdk"],
                        "https://github.com/emscripten-core/emsdk.git", str(sdk)], check=True)
    revision = subprocess.check_output(["git", "-C", str(sdk), "rev-parse", "HEAD"], text=True).strip()
    if revision != pins["emsdk_commit"]:
        raise RuntimeError("Existing Emsdk checkout has the wrong revision; choose another --emsdk directory")
    subprocess.run([str(sdk / "emsdk"), "install", pins["emsdk"]], check=True)
    subprocess.run([str(sdk / "emsdk"), "activate", pins["emsdk"]], check=True)
    subprocess.run(["rustup", "toolchain", "install", pins["rust"], "--profile", "minimal",
                    "--component", "rust-src"], check=True)


if __name__ == "__main__":
    main()
