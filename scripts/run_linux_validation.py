"""Linux-only validation of the existing musl release in a new report directory."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    if sys.platform != "linux":
        raise SystemExit("test:linux executes an ELF binary; run this task on Linux")
    pins = json.loads((ROOT / "scripts/linux-toolchain.json").read_text())
    binary = ROOT / "target/zigbuild" / pins["target"] / "release/assfonts-rs"
    oracle = ROOT / ".validation/oracle/verify_libass"
    fonts = ROOT / ".validation/ci-fonts"
    for path, action in [(binary, "build:linux"), (oracle, "setup:renderer"),
                         (fonts / "NotoSans.ttc", "setup:fonts"),
                         (fonts / "NotoSansSC-Regular.otf", "setup:fonts")]:
        if not path.is_file():
            raise SystemExit(f"Missing {path}; run mise run {action}")
    root = Path(tempfile.mkdtemp(prefix="mise-linux-", dir=ROOT / ".validation"))
    check = [sys.executable, "scripts/check_linux_artifact.py", "--binary", str(binary),
             "--output", str(root / "artifact")]
    subprocess.run(check, cwd=ROOT, check=True)
    subprocess.run([sys.executable, "tests/render/verify.py", "--binary", str(binary),
                    "--oracle", str(oracle), "--fixture", "tests/fixtures/ci-fonts.ass",
                    "--ttc-face-index", "1", "--ttc", str(fonts / "NotoSans.ttc"),
                    "--otf", str(fonts / "NotoSansSC-Regular.otf"),
                    "--output", str(root / "render")], cwd=ROOT, check=True)
    subprocess.run(check, cwd=ROOT, check=True)
    print(f"Linux artifact and RGBA validation passed: {root}")


if __name__ == "__main__":
    main()
