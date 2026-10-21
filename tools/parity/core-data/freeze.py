# SPDX-License-Identifier: GPL-3.0-or-later
"""Freeze checked upstream replies; never derive expected values from Rust."""
import argparse
import hashlib
import json
from pathlib import Path
from check import settings_projection

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('evidence', type=Path)
args = parser.parse_args()
report = json.loads((args.evidence / 'report.json').read_text(encoding='utf-8'))
assert all(row['matches'] for section in ['data', 'keys', 'settings'] for row in report[section])

def read(name):
    return [json.loads(line) for line in (args.evidence / name).read_text(encoding='utf-8-sig').splitlines()]

cases = json.loads((args.evidence / 'cases.json').read_text(encoding='utf-8'))
original = read('csharp-keys.jsonl')
assert len(cases) == len(original)
for case, reply in zip(cases, original, strict=True):
    assert case['name'] == reply['name']
    assert len(case['keys']) == len(reply['expected'])
    case['expected'] = [dict(consumed=r['consumed'], commits=r['commits'], view={k:r['view'][k] for k in ['text','converting','suggestion']} if r['view'] else None) for r in reply['expected']]
settings = json.loads((args.evidence / 'settings-cases.json').read_text(encoding='utf-8'))
for case, reply in zip(settings, read('csharp-settings.jsonl'), strict=True):
    assert case['name'] == reply['name']
    case['expected'] = settings_projection(reply)['expected']
hashes = {name: hashlib.sha256((args.evidence / name).read_bytes()).hexdigest() for name in ['cases.json','csharp-keys.jsonl','settings-cases.json','csharp-settings.jsonl']}
fixture = dict(baseline='45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46', source_sha256=report['source_sha256'], capture_sha256=hashes, cases=cases, settings=settings)
Path('rust/core/tests/data/upstream-core-data.json').write_text(json.dumps(fixture, ensure_ascii=False, separators=(',',':'))+'\n', encoding='utf-8')
print(len(cases), 'key cases;', len(settings), 'settings cases frozen')
