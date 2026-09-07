# /// script
# requires-python = ">=3.12"
# dependencies = []
# ///
"""Generate the pinned, reversible CP936 encoder; never run during a build.

Download bestfit936.txt from the URL below, then pass it with --source.
Despite the source filename, best-fit substitutions are explicitly excluded.
The source hash, counts, uniqueness and round trip are checked before writing.
"""
import argparse
import hashlib
import re
import struct
from pathlib import Path

SOURCE_URL = "https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WindowsBestFit/bestfit936.txt"
SOURCE_SHA256 = "e5070a2d6ad26619f5872ddbe64d3381c11620af5adbb04cda0f0abb1a91fdae"
DEFAULT_OUTPUT = Path(__file__).resolve().parents[1] / "crates/fonts/data/cp936.bin"


def generate(source: bytes) -> bytes:
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise ValueError("CP936 source checksum mismatch")
    mode, lead = None, 0
    decode, encode = {}, {}
    for line in source.decode("latin1").splitlines():
        if line.startswith("MBTABLE"):
            mode = "mb"
            continue
        if line.startswith("DBCSRANGE"):
            mode = None
            continue
        if line.startswith("DBCSTABLE"):
            lead = int(re.search(r"LeadByte = (0x[0-9a-fA-F]+)", line)[1], 16)
            mode = "db"
            continue
        if line.startswith("WCTABLE"):
            mode = "wc"
            continue
        if line.startswith("ENDCODEPAGE"):
            break
        words = line.split(";")[0].split()
        if len(words) != 2 or mode is None or not all(w.startswith("0x") for w in words):
            continue
        a, b = (int(w, 16) for w in words)
        if mode == "wc":
            if a in encode:
                raise ValueError("duplicate Unicode entry")
            encode[a] = b
        else:
            code = a if mode == "mb" else (lead << 8) | a
            if code in decode:
                raise ValueError("duplicate CP936 entry")
            decode[code] = b
    if (len(decode), len(encode)) != (24070, 24482):
        raise ValueError("unexpected source table sizes")
    exact = sorted((u, code) for u, code in encode.items() if decode.get(code) == u)
    if len(exact) != 24070 or len({code for _, code in exact}) != len(exact):
        raise ValueError("expected one-to-one reversible CP936 map")
    if any(not 0 <= u <= 0xffff or 0xd800 <= u <= 0xdfff for u, _ in exact):
        raise ValueError("invalid Unicode scalar in CP936 map")
    return b"".join(struct.pack(">HH", u, code) for u, code in exact)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True, help=SOURCE_URL)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--check", action="store_true", help="verify an existing generated file without writing")
    args = parser.parse_args()
    output = generate(args.source.read_bytes())
    if args.check:
        if args.output.read_bytes() != output:
            raise SystemExit("generated CP936 table is out of date")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_bytes(output)
    print(f"{len(output) // 4} mappings; sha256={hashlib.sha256(output).hexdigest()}")


if __name__ == "__main__":
    main()
