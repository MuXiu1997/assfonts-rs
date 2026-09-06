"""On Linux, validate and execute the exact release artifact (without rebuilding)."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(binary.with_name("build-manifest.json").read_text())
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    if digest != manifest["sha256"]:
        raise RuntimeError("Release binary differs from the build manifest")
    reports = {}
    for name, flag in (("header", "-h"), ("program", "-l"), ("dynamic", "-d")):
        reports[name] = subprocess.check_output(["readelf", "-W", flag, str(binary)], text=True)
        (args.output / f"linkage-{name}.txt").write_text(reports[name])
    if not re.search(r"Machine:\s+Advanced Micro Devices X86-64", reports["header"]):
        raise RuntimeError("Unexpected ELF architecture")
    if "INTERP" in reports["program"] or "NEEDED" in reports["dynamic"]:
        raise RuntimeError("Release binary is not fully static")
    backends = subprocess.check_output([str(binary), "--list-backends"], text=True)
    if "harfbuzz" not in backends.lower():
        raise RuntimeError("Missing HarfBuzz backend")
    result = {"sha256": digest, "fully_static": True, "backends": backends.strip()}
    (args.output / "artifact-check.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
