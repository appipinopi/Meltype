# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare read-only Windows requests to the unmodified C# adapter."""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('dll', type=Path)
    parser.add_argument('reference', type=Path)
    parser.add_argument('cases', type=Path, nargs='?', default=Path(__file__).resolve().parents[3] / 'tools/parity/windows-converter/cases.json')
    parser.add_argument('--report', type=Path)
    parser.add_argument('--keep-going', action='store_true', help='Record mismatches and check remaining requests; still exit unsuccessfully.')
    parser.add_argument('--batch-probe', action='store_true', help='Reuse the worker-style STA within a diagnostic batch (max 256).')
    args = parser.parse_args()
    api = c.CDLL(str(args.dll.resolve()))
    api.meltype_windows_converter_probe.argtypes = [c.c_char_p]
    api.meltype_windows_converter_probe.restype = c.c_void_p
    api.meltype_free.argtypes = [c.c_void_p]
    cases = json.loads(args.cases.read_text(encoding='utf-8'))
    reference_rows = [json.loads(line) for line in args.reference.read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    reference = {row['name']: row for row in reference_rows}
    assert len(cases) == len(reference_rows) == len(reference)
    assert len({case['name'] for case in cases}) == len(cases)
    assert {case['name'] for case in cases} == set(reference)
    available = retries = 0
    checks = []
    started = time.monotonic()

    def probe(request):
        pointer = api.meltype_windows_converter_probe(json.dumps(request, ensure_ascii=False).encode())
        assert pointer, 'probe failed'
        try:
            return json.loads(c.string_at(pointer))
        finally:
            api.meltype_free(pointer)

    initial = None
    if args.batch_probe:
        assert 0 < len(cases) <= 256
        initial = probe(dict(requests=[dict(case, trace=True) for case in cases]))['responses']
        assert len(initial) == len(cases)
    for index, case in enumerate(cases):
        expected = {k: v for k, v in reference[case['name']].items() if k != 'name'}
        for attempt in range(3):
            value = initial[index] if attempt == 0 and initial is not None else probe(dict(case, trace=True))
            timed_out = value.pop('candidateTimedOut')
            # Retry only explicit timeouts with matching conversion/clauses.
            # Never retry substantive mismatches; the final full list must match.
            stable = all(value[field] == expected[field] for field in ['converted', 'clauses'])
            if not timed_out or value['candidates'] == expected['candidates'] or not stable or attempt == 2:
                break
            retries += 1
        if not case['text']:
            expected.pop('available')
            value.pop('available')
        else:
            available += bool(value['available'])
        fields = [field for field in expected if value[field] != expected[field]]
        check = dict(name=case['name'], matches=not fields, fields=fields, candidate_timed_out=timed_out)
        if fields:
            check.update(expected=expected, actual=value)
        checks.append(check)
        if fields and not args.keep_going:
            break
    matched = sum(check['matches'] for check in checks)
    report = dict(requests=len(cases), checked=len(checks), matched=matched, mismatched=len(checks)-matched,
                  available=available, timeout_retries=retries, seconds=time.monotonic()-started,
                  complete=len(checks)==len(cases), all_possible_os_responses_verified=False,
                  lifecycle='shared-sta-per-batch' if args.batch_probe else 'fresh-sta-per-request',
                  sha256={str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in [args.dll, args.reference, args.cases, Path(__file__)]},
                  checks=checks)
    if args.report:
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
    print(f'Windows converter: {matched}/{len(cases)} original requests matched; {available} used an available Microsoft IME; {retries} bounded WinRT timeout retries', flush=True)
    for check in checks:
        if not check['matches']:
            print(check['name'], 'mismatched fields:', ', '.join(check['fields']), flush=True)
    raise SystemExit(not (report['complete'] and not report['mismatched']))


if __name__ == '__main__':
    main()
