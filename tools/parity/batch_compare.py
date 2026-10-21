# SPDX-License-Identifier: GPL-3.0-or-later
"""Checkpoint finite C#/Rust comparisons; preserve failures without claiming universal coverage.

Run from the repository root. Raw OS responses and test profiles belong in dist/.
Resume requires identical cases, binaries, scripts, and batch size. Completed
batches (including mismatches) are retained, never silently rerun until passing.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding='utf-8')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('suite', choices=['settings', 'windows'])
    parser.add_argument('--cases', required=True, type=Path)
    parser.add_argument('--csharp', required=True, type=Path)
    parser.add_argument('--dll', required=True, type=Path)
    parser.add_argument('--x86-dll', type=Path)
    parser.add_argument('--x86-python', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--batch-size', type=int, default=128)
    parser.add_argument('--reuse-sta', action='store_true', help='Windows only: use the real worker converter lifetime in each batch.')
    args = parser.parse_args()
    if args.batch_size <= 0 or bool(args.x86_dll) != bool(args.x86_python):
        parser.error('positive batch-size and both x86 arguments required')
    if args.reuse_sta and (args.suite != 'windows' or args.batch_size > 256):
        parser.error('reuse-sta requires windows and a batch-size of at most 256')
    rows = json.loads(args.cases.read_text(encoding='utf-8'))
    assert rows and len({row['name'] for row in rows}) == len(rows)
    script = Path('tools/parity/settings-candidates/check.py' if args.suite == 'settings' else 'rust/native/tests/windows_converter_smoke.py')
    files = [args.cases, args.csharp, args.dll, script, Path(__file__)]
    files.extend(sorted(args.csharp.parent.glob('*.dll')))
    if args.x86_dll:
        files.extend([args.x86_dll, args.x86_python])
    identity = dict(suite=args.suite, batch_size=args.batch_size, reuse_sta=args.reuse_sta, windows_version=platform.win32_ver(),
                    files={str(p.resolve()): digest(p) for p in files})
    # JSON round-trip gives resumed identities the same tuple/list representation.
    identity = json.loads(json.dumps(identity))
    args.output.mkdir(parents=True, exist_ok=True)
    state_path = args.output / 'checkpoint.json'
    if state_path.exists():
        state = json.loads(state_path.read_text(encoding='utf-8'))
        if state['identity'] != identity:
            raise SystemExit('Resume refused: cases/binaries/scripts/batch-size changed; use a new output folder.')
        for batch in state['batches']:
            for filename, sha256 in batch['evidence_sha256'].items():
                if not Path(filename).exists() or digest(filename) != sha256:
                    raise SystemExit('Resume refused: saved evidence changed or is missing: ' + filename)
    else:
        state = dict(identity=identity, requests=len(rows), batches=[], started_at=time.time())
        write_json(state_path, state)
    architectures = [('x64', sys.executable, args.dll)]
    if args.x86_dll:
        architectures.append(('x86', str(args.x86_python.resolve()), args.x86_dll))
    env = dict(os.environ, PYTHONUTF8='1', PYTHONIOENCODING='utf-8')

    def command(argv, log):
        with log.open('w', encoding='utf-8') as output:
            result = subprocess.run([str(a) for a in argv], stdout=output, stderr=subprocess.STDOUT, env=env)
        return result.returncode

    def summarize():
        summary = dict(suite=args.suite, requests=len(rows), batch_size=args.batch_size,
                       completed_batches=len(state['batches']), identity=identity,
                       all_68_cartesian_combinations_verified=False,
                       all_possible_os_responses_verified=False, architectures={})
        for arch, _, _ in architectures:
            checks = [batch['results'][arch] for batch in state['batches'] if arch in batch['results']]
            checked = sum(row['checked'] for row in checks)
            matched = sum(row['matched'] for row in checks)
            summary['architectures'][arch] = dict(checked=checked, matched=matched,
                mismatched=checked-matched, complete=checked==len(rows),
                process_errors=sum(row['returncode'] != 0 and row['checked']==0 for row in checks),
                unsuccessful_processes=sum(row['returncode'] != 0 for row in checks),
                available=sum(row.get('available', 0) for row in checks),
                timeout_retries=sum(row.get('timeout_retries', 0) for row in checks))
        summary['passed'] = all(row['complete'] and row['mismatched']==0 and row['unsuccessful_processes']==0 for row in summary['architectures'].values())
        write_json(args.output / 'summary.json', summary)
        return summary

    completed = {batch['offset'] for batch in state['batches']}
    for offset in range(0, len(rows), args.batch_size):
        if offset in completed:
            continue
        base = args.output / f'batch-{offset:06d}'
        attempt = 0
        folder = base
        while folder.exists():
            attempt += 1
            folder = args.output / f'{base.name}-attempt-{attempt}'
        folder.mkdir()
        cases = folder / 'cases.json'
        write_json(cases, rows[offset:offset+args.batch_size])
        batch = dict(offset=offset, requests=min(args.batch_size, len(rows)-offset), results={}, evidence=str(folder))
        reference = folder / 'reference.jsonl'
        if args.suite == 'windows':
            code = command(['dotnet', args.csharp.resolve(), cases.resolve()] + (['--reuse'] if args.reuse_sta else []), reference)
            batch['reference_returncode'] = code
        for arch, python, dll in architectures:
            report = folder / f'{arch}.json'
            if args.suite == 'windows':
                if batch['reference_returncode']:
                    batch['results'][arch] = dict(returncode=batch['reference_returncode'], checked=0, matched=0)
                    continue
                argv = [python, script, dll.resolve(), reference.resolve(), cases.resolve(), '--keep-going', '--report', report]
                if args.reuse_sta:
                    argv.append('--batch-probe')
            else:
                out = folder / arch
                argv = [python, script, '--csharp', args.csharp.resolve(), '--dll', dll.resolve(), '--cases', cases, '--output', out]
                if arch == 'x86' and (folder / 'x64/reference.json').exists():
                    argv.extend(['--reference', folder / 'x64'])
                report = out / 'report.json'
            code = command(argv, folder / f'{arch}.log')
            result = dict(returncode=code, checked=0, matched=0)
            if report.exists():
                data = json.loads(report.read_text(encoding='utf-8'))
                if args.suite == 'windows':
                    result.update({key: data[key] for key in ['checked', 'matched', 'available', 'timeout_retries']})
                else:
                    result.update(checked=len(data['checks']), matched=sum(check['matches'] for check in data['checks']))
                result['report_sha256'] = digest(report)
            batch['results'][arch] = result
            print(args.suite, offset, arch, result, flush=True)
        batch['evidence_sha256'] = {str(p): digest(p) for p in folder.rglob('*')
                                   if p.is_file() and (p.name in ['cases.json', 'reference.jsonl', 'report.json', 'x64.json', 'x86.json', 'reference.json', 'csharp.jsonl'])}
        state['batches'].append(batch)
        write_json(state_path, state)
        summarize()
    summary = summarize()
    print(json.dumps(summary['architectures']), flush=True)
    raise SystemExit(not summary['passed'])


if __name__ == '__main__':
    main()
