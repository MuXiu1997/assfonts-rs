"""Prepare representative parity against privately held, render-verified outputs.

Usage: python scripts/prepare_memory_parity.py EVIDENCE BUILD OUTPUT_CONFIG
Commercial fonts and generated subtitles are never copied into tracked files.
"""
from collections import defaultdict
import hashlib
import json
from pathlib import Path
import sys


def main():
    evidence, build, output = [Path(p).resolve() for p in sys.argv[1:]]
    repaired = [f's021-{i:04d}' for i in range(1, 14)] + [f's049-{i:04d}' for i in [2, 5, 7, 9]]
    cases = ['s051-0061', 's051-0063', 's051-0062', 's009-0003', 's050-0012',
             's026-0013', 's006-0015', 's026-0028', *repaired]
    grouped = defaultdict(list)
    goldens, hashes = {}, {}

    def verify(path, expected=None):
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if expected is not None:
            assert actual == expected, path
        hashes[str(path)] = actual

    for case in cases:
        row = json.loads((evidence / 'cases' / case / 'processing.json').read_text())
        grouped[row['catalog_signature']].append((case, row))
    catalogs = []
    for signature, jobs in grouped.items():
        directory = evidence / 'catalogs-portable' / signature
        manifest = json.loads((directory / 'manifest.json').read_text())
        mapping = {f['sha256']: directory / 'fonts' / f['transfer_name'] for f in manifest['files']}
        fonts, inputs = [], []
        for font in jobs[0][1]['catalog_files']:
            path = mapping[font['sha256']]
            verify(path, font['sha256'])
            fonts.append({'path': str(path), 'label': font['rel']})
        for case, row in jobs:
            assert row['catalog_files'] == jobs[0][1]['catalog_files']
            source = evidence / 'cases' / case / 'prepared.ass'
            verify(source, row['prepared_sha256'])
            root = 'wasm-original-order' if case in repaired else 'wasm-1'
            golden = evidence / 'work/rejection-validation' / root / 'cases' / case / 'embedded.ass'
            verify(golden)
            inputs.append(str(source)); goldens[str(source)] = str(golden)
        catalogs.append({'fonts': fonts, 'inputs': inputs})
    config = {'build': str(build), 'catalogs': catalogs, 'goldens': goldens, 'loops': 1,
              'eachInputOnce': True, 'duplicates': 0, 'cycles': 1, 'gc': False,
              'clearResults': True, 'restarts': 1, 'concurrency': 1, 'missingGlyphPolicy': 'error'}
    output.write_text(json.dumps(config, indent=2) + '\n')
    output.with_suffix('.sha256.json').write_text(json.dumps(hashes, indent=2) + '\n')
    print(json.dumps({'cases': len(cases), 'catalogs': len(catalogs)}))


if __name__ == '__main__':
    main()
