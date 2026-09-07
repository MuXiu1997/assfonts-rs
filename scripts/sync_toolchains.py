"""Generate legacy build/CI version views from mise.toml; --check never writes."""
import argparse
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    config = tomllib.loads((ROOT / "mise.toml").read_text())
    tools = config["tools"]
    updates = {
        "linux-toolchain.json": {
            "rust": tools["rust"][0]["version"],
            "cargo-zigbuild": tools["cargo:cargo-zigbuild"],
            "ziglang": tools["zig"],
        },
        "wasm-toolchain.json": {
            "rust": tools["rust"][1]["version"],
            "deno": tools["deno"],
            "emsdk": config["vars"]["emsdk_version"],
        },
    }
    for name, values in updates.items():
        path = ROOT / "scripts" / name
        data = json.loads(path.read_text())
        expected = dict(data, **values)
        if args.check:
            if data != expected:
                raise SystemExit(f"{name} differs from mise.toml; run mise run sync-toolchains")
        else:
            path.write_text(json.dumps(expected, indent=2) + "\n")
    print("Toolchain versions match mise.toml")


if __name__ == "__main__":
    main()
