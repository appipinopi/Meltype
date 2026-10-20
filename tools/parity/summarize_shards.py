# SPDX-License-Identifier: GPL-3.0-or-later
"""Validate a disjoint finite partition and aggregate its retained comparisons.

Only input names/counters/hashes are exported; raw OS responses stay local.
Use after batch_compare.py, or during a run for a conservative partial snapshot.
"""
import argparse
import hashlib
import json
from pathlib import Path


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def summarize(source, root, shards):
    original = json.loads(source.read_text(encoding='utf-8'))
    expected = {row['name']: row for row in original}
    assert len(expected) == len(original), 'duplicate source names'
    actual = {}
    totals = {}
    reports = []
    failures = []
    common = None
    architecture_names = None
    for shard in range(shards):
        cases = root / f'cases-{shard}.json'
        part = json.loads(cases.read_text(encoding='utf-8'))
        for row in part:
            assert row['name'] not in actual, 'overlapping shards'
            assert expected.get(row['name']) == row, 'shard differs from source'
            actual[row['name']] = row
        checkpoint = root / f'compare-{shard}/checkpoint.json'
        if not checkpoint.exists():
            continue
        state = json.loads(checkpoint.read_text(encoding='utf-8'))
        identity = state['identity']
        assert identity['files'][str(cases.resolve())] == digest(cases)
        comparable = json.loads(json.dumps(identity))
        del comparable['files'][str(cases.resolve())]
        if common is None:
            common = comparable
        else:
            assert comparable == common, 'shards used different binaries/scripts/lifecycles'
        offsets = set()
        for batch in state['batches']:
            if architecture_names is None:
                architecture_names = set(batch['results'])
            else:
                assert set(batch['results']) == architecture_names, 'inconsistent architecture declarations'
            offset = batch['offset']
            assert offset % identity['batch_size'] == 0 and offset not in offsets
            offsets.add(offset)
            count = min(identity['batch_size'], len(part)-offset)
            assert count > 0 and batch['requests'] == count
            folder = Path(batch['evidence'])
            assert json.loads((folder/'cases.json').read_text(encoding='utf-8')) == part[offset:offset+count]
            for filename, sha256 in batch['evidence_sha256'].items():
                assert digest(Path(filename)) == sha256, 'changed evidence: ' + filename
            for abi, result in batch['results'].items():
                total = totals.setdefault(abi, dict(checked=0, matched=0, available=0, timeout_retries=0, unsuccessful_processes=0))
                for key in ['checked', 'matched', 'available', 'timeout_retries']:
                    total[key] += result.get(key, 0)
                total['unsuccessful_processes'] += result['returncode'] != 0
                if 'report_sha256' not in result:
                    continue
                report = folder / (f'{abi}.json' if identity['suite']=='windows' else f'{abi}/report.json')
                assert digest(report) == result['report_sha256']
                data = json.loads(report.read_text(encoding='utf-8'))
                assert len(data['checks']) == result['checked']
                assert sum(row['matches'] for row in data['checks']) == result['matched']
                assert [row['name'] for row in data['checks']] == [row['name'] for row in part[offset:offset+result['checked']]]
                for row in data['checks']:
                    if not row['matches']:
                        failures.append(dict(shard=shard, abi=abi, name=row['name'],
                                             fields=row.get('fields', []), candidate_timed_out=row.get('candidate_timed_out')))
                reports.append(dict(shard=shard, offset=offset, abi=abi, sha256=result['report_sha256']))
    assert set(actual) == set(expected), 'partition omitted source inputs'
    for total in totals.values():
        assert 0 <= total['matched'] <= total['checked'] <= len(original)
        total.update(mismatched=total['checked']-total['matched'], complete=total['checked']==len(original))
    passed = bool(totals) and all(value['complete'] and not value['mismatched'] and not value['unsuccessful_processes'] for value in totals.values())
    return dict(requests=len(original), partition_valid=True, source_sha256=digest(source), concurrent_shards=shards,
                identity=common, architectures=totals, passed=passed, report_hashes=reports, failures=failures,
                all_68_cartesian_combinations_verified=False, all_possible_os_responses_verified=False)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('root', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--shards', type=int, default=4)
    args = parser.parse_args()
    assert args.shards > 0
    result = summarize(args.source, args.root, args.shards)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding='utf-8')
    print(json.dumps(result['architectures']))
