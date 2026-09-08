"""Summarize memory.ts JSONL files; stdout is machine-readable JSON."""
from collections import defaultdict
import json
from pathlib import Path
import statistics
import sys


def slope(values):
    if len(values) < 2:
        return None
    x = range(len(values))
    center = (len(values) - 1) / 2
    return sum((i - center) * y for i, y in zip(x, values)) / sum((i - center) ** 2 for i in x)


def summarize(path):
    rows = [json.loads(line) for line in path.read_text().splitlines()]
    samples = [r for r in rows if r['type'] == 'sample']
    groups = defaultdict(list)
    for row in samples:
        if row['stage'] == 'input_and_js_result_released' and 'iteration' in row:
            groups[(row['worker'], row['cycle'], row['catalog'])].append(row)
    steady = []
    for key, group in groups.items():
        tail = group[-min(10, len(group)):]
        row = dict(zip(('worker', 'cycle', 'catalog'), key))
        row['iterations'] = len(group)
        for metric in ['linear_bytes', 'live_bytes', 'rss', 'heapUsed', 'external']:
            values = [r[metric] for r in tail if r.get(metric) is not None]
            row[metric] = {'first': group[0].get(metric), 'last': group[-1].get(metric),
                           'tail_min': min(values) if values else None,
                           'tail_max': max(values) if values else None,
                           'tail_slope_bytes_per_iteration': slope(values)}
        steady.append(row)
    durations = [r['elapsed_ms'] for r in samples if r['stage'] == 'process_returned' and r['ok']]
    phases = defaultdict(list)
    for row in samples:
        phases[row['stage']].append(row)
    metrics = ['linear_bytes', 'live_bytes', 'rss', 'heapUsed', 'external', 'result_bytes']
    phase_summary = {}
    for phase, values in phases.items():
        phase_summary[phase] = {
            metric: {'max': max(v[metric] for v in values if v.get(metric) is not None),
                     'last': values[-1].get(metric)}
            for metric in metrics if any(v.get(metric) is not None for v in values)}
    return {
        'file': str(path), 'metadata': rows[0], 'samples': len(samples),
        'fatal': [r for r in rows if r['type'] == 'fatal'],
        'peak_rss_sampled_bytes': max((r.get('rss', 0) for r in rows), default=0),
        'peak_linear_bytes': max((r['linear_bytes'] for r in samples), default=0),
        'peak_live_at_stage_bytes': max((r['live_bytes'] or 0 for r in samples), default=0),
        'process_ms': {'count': len(durations),
                       'median': statistics.median(durations) if durations else None,
                       'max': max(durations) if durations else None,
                       'sum': sum(durations)},
        'steady': steady, 'phases': phase_summary,
        'output_sha256': sorted({r['output_sha256'] for r in samples if 'output_sha256' in r}),
        'post_termination': [r for r in rows if r['type'] == 'post_termination_idle'],
        'engine_destroyed': [r for r in samples if r['stage'] == 'engine_destroyed'],
    }


if __name__ == '__main__':
    print(json.dumps([summarize(Path(path)) for path in sys.argv[1:]], indent=2))
