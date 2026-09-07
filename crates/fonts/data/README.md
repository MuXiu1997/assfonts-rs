# Pinned CP936 conversion data

`cp936.bin` contains 24070 pairs of big-endian unsigned 16-bit integers:
Unicode scalar, then CP936 code (a single byte or two bytes packed big-endian).
Pairs are sorted by Unicode; both columns are unique. The encoder never
substitutes '?' or transliterates. It uses no system codec, locale, native FFI,
or network access, and the same bytes are compiled into native and WASM.

Source: Microsoft's CP936 mapping published by the Unicode Consortium:
https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WindowsBestFit/bestfit936.txt

- Source SHA-256: `e5070a2d6ad26619f5872ddbe64d3381c11620af5adbb04cda0f0abb1a91fdae`.
- Generated SHA-256: `95bee7f7f9af99c8610e8e70a8ada4de9d80303a202414491e3025350fb8dcc9`.
- License: [Unicode License V3](LICENSE-UNICODE), retained with the generated data.

The generator reads the decoding tables and the Unicode encoding table, then
keeps only entries for which decoding the encoded value returns the original
Unicode scalar. **Best-fit entries in the source file are not included.**
This implements the reversible CP936 / WC_NO_BEST_FIT_CHARS policy used by
libass's Windows conversion path, including the original private-use mappings.
It does not silently substitute the modern WHATWG GBK/GB18030 mapping (for
example, CP936 A8BC maps U+E7C7, not U+1E3F).

System iconv implementations may expose different CP936 aliases or best-fit
behavior. They are not runtime dependencies or authorities for this fixed data.
Renderer compatibility must still be tested in the actual fixed environment.

To reproduce offline after downloading the source file:

```sh
uv run --script --no-project scripts/generate_cp936.py --source /path/to/bestfit936.txt --check
```

Omit `--check` to regenerate the file. Normal Cargo/WASM builds neither run
the generator nor download mapping data.
