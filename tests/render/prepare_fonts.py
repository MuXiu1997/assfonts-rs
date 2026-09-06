"""Fetch checksum-pinned OFL fonts and pack two complete TrueType faces into a TTC."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.request

from fontTools.ttLib import TTCollection, TTFont


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    fonts = json.loads(Path(__file__).with_name("fonts.json").read_text())
    for item in fonts:
        path = args.output / item["name"]
        if not path.exists():
            with urllib.request.urlopen(item["url"], timeout=120) as response:
                data = response.read()
            if hashlib.sha256(data).hexdigest() != item["sha256"]:
                raise RuntimeError(f"Download checksum mismatch: {item['name']}")
            path.write_bytes(data)
        if hashlib.sha256(path.read_bytes()).hexdigest() != item["sha256"]:
            raise RuntimeError(f"Cached font checksum mismatch: {path}")
    collection = TTCollection()
    collection.fonts = [TTFont(args.output / name, recalcTimestamp=False)
                        for name in ("NotoSans-Regular.ttf", "NotoSans-Bold.ttf")]
    # No subsetting: baseline and CLI receive the same complete collection.
    collection.save(args.output / "NotoSans.ttc")
    collection.close()
    manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(args.output.iterdir()) if p.suffix in (".ttf", ".otf", ".ttc")}
    (args.output / "font-hashes.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
