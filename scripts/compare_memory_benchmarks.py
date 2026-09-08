"""Summarize the memory study without publishing private font paths or contents.

Usage: python scripts/compare_memory_benchmarks.py .validation/memory > SUMMARY.json
"""
import json
from pathlib import Path
import statistics
import sys

from summarize_memory_benchmark import summarize


def rows(path):
    return [json.loads(line) for line in path.read_text().splitlines()]


def compact(directory, name):
    path = directory / (name + '.jsonl')
    raw = rows(path)
    value = summarize(path)
    config = value['metadata']['config']
    expected = ((sum(len(c['inputs']) for c in config['catalogs']) if config.get('eachInputOnce')
                 else len(config['catalogs']) * config['loops']) * config['cycles']
                * config.get('restarts', 1) * config.get('concurrency', 1))
    assert not value['fatal'] and value['process_ms']['count'] == expected, name
    assert sum(r['type'] == 'done' for r in raw) == config.get('restarts', 1) * config.get('concurrency', 1), name
    metrics = {key: value[key] for key in ['peak_rss_sampled_bytes', 'peak_linear_bytes',
               'peak_live_at_stage_bytes', 'process_ms', 'output_sha256']}
    metrics.update({
        'run': name,
        'build_sha256': value['metadata']['build']['artifacts']['assfonts-wasm.wasm']['sha256'],
        'catalog_bytes': [sum(f.get('bytes', Path(f['path']).stat().st_size) for f in c['fonts'])
                          for c in config['catalogs']],
        'catalog_files': [len(c['fonts']) for c in config['catalogs']],
        'catalog_faces': [max(r.get('faces', 0) for r in raw if r.get('catalog') == i)
                          for i, _ in enumerate(config['catalogs'])],
        'max_subtitle_input_bytes': max(r['input_bytes'] for r in raw if r.get('stage') == 'process_input_allocated'),
        'max_json_result_bytes': max(r.get('result_bytes', 0) for r in raw),
        'max_subset_bytes': max(r.get('subset_bytes', 0) for r in raw),
        'max_requested_character_entries': max(r.get('characters', 0) for r in raw),
        'engine_destroyed_live_bytes': sorted({r['live_bytes'] for r in value['engine_destroyed']}),
        'steady': value['steady'],
        'post_termination_rss_bytes': [r['rss'] for r in value['post_termination']],
    })
    durations = [r['elapsed_ms'] for r in raw if r.get('stage') == 'font_added' and 'duplicate' in r]
    if durations:
        metrics['duplicate_add_median_ms'] = statistics.median(durations)
    return metrics


def main():
    directory = Path(sys.argv[1]).resolve()
    primary = {}
    for case in ['large-output', 'large-catalog', 'duplicate-ttc']:
        before = compact(directory, 'paired-baseline-' + case)
        after = compact(directory, 'paired-optimized-' + case)
        assert before['output_sha256'] == after['output_sha256'], case
        primary[case] = {'before': before, 'after': after,
                         'linear_reduction_percent': 100 * (1 - after['peak_linear_bytes'] / before['peak_linear_bytes'])}
    extended = {}
    for case in ['large-ttc', 'multi-input', 'switch', 'long', 'restarts', 'gc', 'concurrency']:
        before = compact(directory, 'baseline-' + case)
        after = compact(directory, 'optimized-' + case)
        assert before['output_sha256'] == after['output_sha256'], case
        extended[case] = {'before': before, 'after': after}
    variants = {}
    for variant in ['growth-zero', 'emmalloc', 'staging-optimized', 'staging-growth-zero']:
        variants[variant] = {}
        for case in primary:
            result = compact(directory, variant + '-' + case)
            assert result['output_sha256'] == primary[case]['after']['output_sha256'], (variant, case)
            variants[variant][case] = result
    trace = {}
    for case in ['large-output', 'duplicate-ttc']:
        trace[case] = {}
        for variant in ['baseline', 'optimized']:
            samples = [r for r in rows(directory / f'trace-{variant}-{case}.jsonl') if r['type'] == 'sample']
            assert all(r.get('trace_unknown_frees', 0) == 0 and r.get('trace_duplicate_allocations', 0) == 0 for r in samples)
            trace[case][variant] = {
                'peak_requested_bytes': max(r['trace_peak_requested_bytes'] for r in samples),
                'after_destroy_requested_bytes': [r['trace_requested_bytes'] for r in samples if r['stage'] == 'engine_destroyed'],
                'after_destroy_blocks': [r['trace_blocks'] for r in samples if r['stage'] == 'engine_destroyed'],
            }
    retention = {}
    for mode in ['paths', 'subtitle']:
        retention[mode] = [{key: row.get(key) for key in ['stage', 'heapUsed', 'rss', 'live_bytes', 'retained_count']}
                           for row in rows(directory / f'retain-{mode}.jsonl')
                           if row.get('stage', '').startswith('retained_results')]
    parity = {}
    for variant in ['safe', 'final']:
        raw = rows(directory / f'parity-{variant}.jsonl')
        assert not any(r['type'] == 'fatal' for r in raw), variant
        verified = [r for r in raw if r.get('stage') == 'golden_verified']
        assert len(verified) == 25, variant
        parity[variant] = {'cases': len(verified), 'attachments': sum(r['attachments'] for r in verified),
                           'output_sha256': {Path(r['input']).parent.name: r['output_sha256'] for r in verified}}
    assert parity['safe'] == parity['final']
    result = {
        'schema_version': 1, 'baseline_git_revision': 'bba0cc459672d33eddcc87672a2136e3cb414716',
        'primary': primary, 'extended': extended, 'variants': variants,
        'allocation_trace': trace, 'retention_controls': retention, 'parity': parity,
        'clear_results': compact(directory, 'optimized-clear-results'),
        'mixed_restarts': compact(directory, 'mixed-restarts'),
        'limits': {key: value for key, value in json.loads((directory / 'limits-final.json').read_text()).items()
                   if key != 'manifest'},
        'final_build_sha256': json.loads((directory / 'final-build/build-manifest.json').read_text())['artifacts'],
        'measurement_notes': [
            'RSS maxima are sampled, not exact continuous maxima; RSS is shared by all Workers in a process.',
            'Stage live allocation peaks are lower bounds; separate allocation tracing measures transient requested bytes.',
            'V8 external includes WASM buffers: do not add it to linear capacity.',
            'Allocator statistics include headers/padding; emmalloc arena has different semantics from dlmalloc.',
            'Forced GC is a separate diagnostic control, not the basis of the stability claim.',
        ],
    }
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
