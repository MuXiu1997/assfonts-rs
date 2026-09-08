"""Run a bounded local Deno workload and independently sample process RSS/VSZ.

Usage: python scripts/run_memory_benchmark.py CONFIG OUTPUT_PREFIX [--expose-gc]
Creates PREFIX.jsonl, PREFIX.process.jsonl, PREFIX.log, PREFIX.environment.json.
No child process or network access is granted to the WASM host itself.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('config', type=Path)
    parser.add_argument('prefix', type=Path)
    parser.add_argument('--expose-gc', action='store_true')
    args = parser.parse_args()
    prefix = args.prefix.resolve()
    config = args.config.resolve()
    command = ['deno', 'run', '--allow-read', '--allow-write', '--deny-run', '--deny-net']
    if args.expose_gc:
        command += ['--v8-flags=--expose-gc']
    command += [str(ROOT / 'tests/wasm/memory.ts'), str(config), str(prefix) + '.jsonl']
    source_files = [ROOT / 'Cargo.lock', ROOT / 'Cargo.toml', ROOT / 'scripts/build_wasm.py',
                    Path(__file__).resolve(), ROOT / 'scripts/prepare_memory_benchmark.py',
                    *sorted((ROOT / 'crates').rglob('*.rs')),
                    *sorted((ROOT / 'crates').rglob('*.cc')),
                    *sorted((ROOT / 'crates').rglob('*.hh')),
                    *sorted((ROOT / 'crates').rglob('Cargo.toml')),
                    *sorted((ROOT / 'tests/wasm').glob('*.ts'))]
    metadata = {'command': command, 'platform': platform.platform(), 'machine': platform.machine(),
                'git_head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'workspace_source_sha256': {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest()
                                  for p in source_files},
                'rss_limit_bytes': 4 * 1024 ** 3, 'timeout_seconds': 600,
                'sampling': 'ps RSS and VSZ in KiB every >=50ms; Deno stages and parent RSS every >=20ms'}
    started = time.monotonic()
    with open(str(prefix) + '.log', 'x') as log, open(str(prefix) + '.process.jsonl', 'x') as samples:
        process = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT)
        metadata['pid'] = process.pid
        try:
            while process.poll() is None:
                result = subprocess.run(['ps', '-o', 'rss=,vsz=', '-p', str(process.pid)],
                                        capture_output=True, text=True, timeout=5)
                if result.returncode == 0 and result.stdout.strip():
                    rss, vsz = [int(n) * 1024 for n in result.stdout.split()]
                    samples.write(json.dumps({'timestamp_ms': time.time_ns() // 1_000_000,
                                              'rss_bytes': rss, 'virtual_bytes': vsz}) + '\n')
                    samples.flush()
                    if rss > metadata['rss_limit_bytes']:
                        raise RuntimeError('Controlled 4 GiB RSS limit exceeded')
                if time.monotonic() - started > metadata['timeout_seconds']:
                    raise RuntimeError('Controlled 600s process timeout exceeded')
                time.sleep(.05)
            metadata['exit_code'] = process.returncode
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
            metadata['elapsed_seconds'] = time.monotonic() - started
            with open(str(prefix) + '.environment.json', 'x') as out:
                json.dump(metadata, out, indent=2)
                out.write('\n')
        if process.returncode:
            raise RuntimeError(f'Deno exited {process.returncode}; inspect {prefix}.log')
    print(prefix)


if __name__ == '__main__':
    main()
