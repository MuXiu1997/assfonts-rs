"""Prepare private, hash-verified workloads without copying commercial fonts.

Run with Python 3.12: scripts/prepare_memory_benchmark.py EVIDENCE_DIRECTORY.
Outputs stay under this worktree's ignored .validation/memory directory.
"""
import hashlib
import json
import argparse
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('evidence', type=Path)
    parser.add_argument('--output', type=Path, default=ROOT / '.validation/memory')
    parser.add_argument('--build', type=Path, default=ROOT / 'target/wasm-memory')
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    verified = {}

    def catalog(case_ids):
        rows = [json.loads((evidence / "cases" / case / "processing.json").read_text())
                for case in case_ids]
        first = rows[0]
        assert all(row["catalog_files"] == first["catalog_files"] for row in rows)
        directory = evidence / "catalogs-portable" / first["catalog_signature"]
        manifest = json.loads((directory / "manifest.json").read_text())
        mapping = {f["sha256"]: directory / "fonts" / f["transfer_name"] for f in manifest["files"]}
        fonts = []
        for font in first["catalog_files"]:  # Original registration order, not portable filenames.
            path = mapping[font["sha256"]]
            if str(path) not in verified:
                assert hashlib.sha256(path.read_bytes()).hexdigest() == font["sha256"]
                verified[str(path)] = font["sha256"]
            fonts.append({"path": str(path), "label": font["rel"],
                          "bytes": font["bytes"], "sha256": font["sha256"]})
        inputs = []
        for case, row in zip(case_ids, rows):
            path = evidence / "cases" / case / "prepared.ass"
            assert hashlib.sha256(path.read_bytes()).hexdigest() == row["prepared_sha256"]
            verified[str(path)] = row["prepared_sha256"]
            inputs.append(str(path))
        return {"fonts": fonts, "inputs": inputs}

    workloads = {
        "large-output": [catalog(["s051-0061"])],
        "multi-input": [catalog(["s051-0061", "s051-0063", "s051-0062"])],
        "large-ttc": [catalog(["s009-0003"])],
        "large-catalog": [catalog(["s050-0012"])],
        "switch": [catalog(["s026-0013"]), catalog(["s051-0061"])],
    }
    for name, catalogs in workloads.items():
        config = {"build": str(args.build.resolve()), "catalogs": catalogs,
                  "loops": 30, "duplicates": 3, "cycles": 2 if name == "switch" else 1,
                  "gc": False, "restarts": 2, "concurrency": 1, "missingGlyphPolicy": "error"}
        (output / f"{name}.json").write_text(json.dumps(config, indent=2) + "\n")
    (output / "inputs-sha256.json").write_text(json.dumps(verified, indent=2) + "\n")
    print(json.dumps({name: {"catalog_bytes": [sum(f['bytes'] for f in c['fonts']) for c in cats]}
                      for name, cats in workloads.items()}, indent=2))


if __name__ == "__main__":
    main()
