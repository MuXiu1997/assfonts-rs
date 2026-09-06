"""Reproducible optional rendering test; only stdlib and an external libass oracle.

The two original fonts are user-supplied and never copied into the repository.
Run with --help. Use a new output directory for each run.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path,
                        default=Path(__file__).parent.parent / "fixtures/cff-collection.ass")
    for name in ("binary", "oracle", "ttc", "otf", "output"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    directory = args.output.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    shutil.copyfile(args.fixture, directory / "input.ass")
    subprocess.run([
        str(args.binary.resolve()), "-i", str(directory / "input.ass"),
        "-f", str(args.ttc.resolve()), "-f", str(args.otf.resolve()),
        "-o", str(directory), "--report", str(directory / "processing.json"),
    ], check=True)
    reports = json.loads((directory / "processing.json").read_text(encoding="utf-8"))["files"][0]["fonts"]
    assert len(reports) == 2, reports
    output = (directory / "input.assfonts.ass").read_text(encoding="utf-8")
    prefix, rest = output.split("[Fonts]\n", 1)
    attachments, events = rest.split("[Events]", 1)
    entries = {}
    for entry in attachments.split("fontname: ")[1:]:
        name, data = entry.split("\n", 1)
        entries[name] = data.strip()
    # Remove one attachment at a time independently of the Rust encoder.
    for source, destination in ((args.ttc, "ttc-only.ass"), (args.otf, "otf-only.ass")):
        report = next(r for r in reports if Path(r["source"]) == source.resolve())
        name = report["attachment"]
        if source == args.ttc:
            assert report["face_index"] == 2, report
        (directory / destination).write_text(
            f"{prefix}[Fonts]\nfontname: {name}\n{entries[name]}\n\n[Events]{events}", encoding="utf-8"
        )
    with (directory / "render.json").open("w") as report:
        subprocess.run([str(args.oracle.resolve()), str(directory), str(args.ttc.resolve()), str(args.otf.resolve())],
                       stdout=report, check=True)
    result = json.loads((directory / "render.json").read_text(encoding="utf-8"))
    assert result["passed"], result
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
