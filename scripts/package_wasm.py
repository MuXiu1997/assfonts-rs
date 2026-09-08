"""Assemble the npm package from the existing verified WASM build; never rebuild it."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "packages/assfonts-rs-wasm"


def main():
    build = ROOT / "target/wasm"
    manifest = json.loads((build / "build-manifest.json").read_text())
    if manifest["memory_profile"] or manifest["safe_heap"] or manifest["allocation_trace"]:
        raise RuntimeError("Refusing to package a diagnostic build")
    for name, record in manifest["artifacts"].items():
        data = (build / name).read_bytes()
        if len(data) != record["bytes"] or hashlib.sha256(data).hexdigest() != record["sha256"]:
            raise RuntimeError(f"Artifact mismatch: {name}")
    for name, checksum in manifest["source_sha256"].items():
        if hashlib.sha256((ROOT / name).read_bytes()).hexdigest() != checksum:
            raise RuntimeError(f"WASM build is stale: {name}; rebuild and validate first")
    # Only these generated directories are replaced. Package sources remain intact.
    shutil.rmtree(PACKAGE / "dist", ignore_errors=True)
    subprocess.run(["npm", "run", "build"], cwd=PACKAGE, check=True)
    output = PACKAGE / "dist/wasm"
    output.mkdir()
    for name in ["engine.js", "assfonts-wasm.wasm", "build-manifest.json"]:
        shutil.copyfile(build / name, output / name)
    shutil.rmtree(PACKAGE / "licenses", ignore_errors=True)
    shutil.copytree(build / "licenses", PACKAGE / "licenses")
    # This notice lives in source, which is deliberately absent from the npm tarball.
    encoder = (ROOT / "crates/ass/src/attachment.rs").read_text()
    notice = encoder.split("// This encoder", 1)[1].split("\n\n", 1)[0]
    (PACKAGE / "licenses/aegisub-encoder-LICENSE").write_text(
        "This encoder" + notice.replace("\n// ", "\n").replace("\n//", "\n") + "\n")
    for name in ["LICENSE", "THIRD_PARTY.md"]:
        shutil.copyfile(ROOT / name, PACKAGE / name)
    print(PACKAGE)


if __name__ == "__main__":
    main()
