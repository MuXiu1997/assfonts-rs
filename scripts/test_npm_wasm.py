"""Pack, install and test the exact npm tarball in an isolated Deno consumer."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "packages/assfonts-rs-wasm"


def run(*args, cwd):
    subprocess.run(args, cwd=cwd, check=True)


def main():
    # Source unit tests resolve TS's NodeNext .js specifiers to .ts; installed consumers need no flag.
    run("deno", "test", "--no-remote", "--unstable-sloppy-imports", "tests/engine_test.ts", cwd=PACKAGE)
    destination = ROOT / "target/npm"
    destination.mkdir(parents=True, exist_ok=True)
    packed = json.loads(subprocess.check_output(
        ["npm", "pack", "--json", "--pack-destination", str(destination)], cwd=PACKAGE, text=True,
    ))[0]
    tarball = destination / packed["filename"]
    files = {item["path"] for item in packed["files"]}
    required = {"package.json", "dist/index.js", "dist/index.d.ts", "dist/wasm/engine.js",
                "dist/wasm/assfonts-wasm.wasm", "dist/wasm/build-manifest.json", "LICENSE",
                "THIRD_PARTY.md", "licenses/harfbuzz-COPYING", "licenses/cp936-LICENSE-UNICODE"}
    if not required <= files or any(name.startswith(("src/", "node_modules/", "tests/")) for name in files):
        raise RuntimeError("Incorrect npm package contents")
    with tempfile.TemporaryDirectory(prefix="assfonts-npm-") as directory:
        consumer = Path(directory)
        (consumer / "package.json").write_text('{"private":true,"type":"module"}\n')
        run("npm", "install", "--ignore-scripts", "--no-audit", "--no-fund", str(tarball), cwd=consumer)
        installed = consumer / "node_modules/@muxiu1997/assfonts-rs-wasm"
        for name, record in json.loads((installed / "dist/wasm/build-manifest.json").read_text())["artifacts"].items():
            if hashlib.sha256((installed / "dist/wasm" / name).read_bytes()).hexdigest() != record["sha256"]:
                raise RuntimeError(f"Installed artifact mismatch: {name}")
        for name in ["consumer.ts", "worker.ts"]:
            shutil.copyfile(PACKAGE / "tests" / name, consumer / name)
        shutil.copyfile(ROOT / "examples/basic.ass", consumer / "input.ass")
        shutil.copyfile(ROOT / "vendor/harfbuzz/test/api/fonts/OpenSans-Regular.ttf", consumer / "font.ttf")
        shutil.copyfile(installed / "dist/wasm/assfonts-wasm.wasm", consumer / "binary.wasm")
        run("deno", "check", "consumer.ts", "worker.ts", cwd=consumer)
        run("deno", "run", "--allow-read", "--deny-net", "--deny-run", "--no-remote", "consumer.ts", cwd=consumer)
    print(f"Verified publishable tarball: {tarball}")


if __name__ == "__main__":
    main()
