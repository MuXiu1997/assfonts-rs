"""Reproducible optional rendering test; only stdlib and an external libass oracle.

The two full fonts are supplied by the caller (CI uses verified OFL downloads).
Run with --help. Use a new output directory for each run.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def require(condition, detail):
    if not condition:
        raise RuntimeError(detail)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path,
                        default=Path(__file__).parent.parent / "fixtures/cff-collection.ass")
    parser.add_argument("--ttc-face-index", type=int, default=2)
    for name in ("binary", "oracle", "ttc", "otf", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    directory = args.output.resolve()
    binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
    directory.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(args.fixture, directory / "input.ass")
    subprocess.run([
        str(args.binary.resolve()), "-i", str(directory / "input.ass"),
        "-f", str(args.ttc.resolve()), "-f", str(args.otf.resolve()),
        "-o", str(directory), "--report", str(directory / "processing.json"),
        "--missing-glyphs", "error",  # Keep this strict TTC/CFF regression explicit.
    ], check=True)
    reports = json.loads((directory / "processing.json").read_text(encoding="utf-8"))["files"][0]["fonts"]
    require(len(reports) == 2, reports)
    output = (directory / "input.assfonts.ass").read_text(encoding="utf-8")
    prefix, rest = output.split("[Fonts]\n", 1)
    attachments, events = rest.split("[Events]", 1)
    raw = (directory / "input.assfonts.ass").read_bytes()
    restored = raw.split(b"[Fonts]", 1)[0] + b"[Events]" + raw.split(b"[Events]", 1)[1]
    require(restored == (directory / "input.ass").read_bytes(), "ASS content changed")
    entries = {}
    for entry in attachments.split("fontname: ")[1:]:
        name, data = entry.split("\n", 1)
        entries[name] = data.strip()
    # Remove one attachment at a time independently of the Rust encoder.
    for source, destination in ((args.ttc, "ttc-only.ass"), (args.otf, "otf-only.ass")):
        report = next(r for r in reports if Path(r["source"]) == source.resolve())
        name = report["attachment"]
        if source == args.ttc:
            require(report["face_index"] == args.ttc_face_index, report)
        (directory / destination).write_text(
            f"{prefix}[Fonts]\nfontname: {name}\n{entries[name]}\n\n[Events]{events}", encoding="utf-8"
        )
    with (directory / "render.json").open("w") as report:
        subprocess.run([str(args.oracle.resolve()), str(directory), str(args.ttc.resolve()), str(args.otf.resolve())],
                       stdout=report, check=True)
    result = json.loads((directory / "render.json").read_text(encoding="utf-8"))
    require(result["passed"], result)
    for name in ("baseline", "embedded"):
        log = (directory / f"{name}.libass.log").read_text()
        require(not re.search(r"failed|error|glyph .*not found", log, re.IGNORECASE), log)
    require(hashlib.sha256(args.binary.read_bytes()).hexdigest() == binary_hash, "Binary changed during test")
    (directory / "inputs.json").write_text(json.dumps({
        "binary_sha256": binary_hash,
        "fixture_sha256": hashlib.sha256(args.fixture.read_bytes()).hexdigest(),
        "ttc_sha256": hashlib.sha256(args.ttc.read_bytes()).hexdigest(),
        "otf_sha256": hashlib.sha256(args.otf.read_bytes()).hexdigest(),
        "ttc_face_index": args.ttc_face_index,
    }, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
