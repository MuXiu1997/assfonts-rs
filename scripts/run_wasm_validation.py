"""Validate the existing WASM artifact in a fresh directory; never overwrites prior reports."""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    fonts = ROOT / ".validation/ci-fonts"
    oracle = ROOT / ".validation/oracle/verify_libass"
    for path, action in [(fonts / "NotoSans.ttc", "setup:fonts"),
                         (fonts / "NotoSansSC-Regular.otf", "setup:fonts"),
                         (oracle, "setup:renderer"),
                         (ROOT / "target/wasm/build-manifest.json", "build:wasm")]:
        if not path.is_file():
            raise SystemExit(f"Missing {path}; run mise run {action}")
    parent = Path(tempfile.mkdtemp(prefix="mise-wasm-", dir=ROOT / ".validation"))
    output = parent / "render"
    subprocess.run(["deno", "run", "--allow-read", "--allow-write", "--deny-run", "--deny-net",
                    "--no-remote", "tests/wasm/verify.ts", str(fonts), str(output)], cwd=ROOT, check=True)
    with (output / "render.json").open("w") as report:
        subprocess.run([str(oracle), str(output), str(fonts / "NotoSans.ttc"),
                        str(fonts / "NotoSansSC-Regular.otf")], cwd=ROOT, stdout=report, check=True)
    print(f"WASM and RGBA validation passed: {output}")


if __name__ == "__main__":
    main()
