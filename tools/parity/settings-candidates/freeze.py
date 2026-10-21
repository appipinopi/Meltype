# SPDX-License-Identifier: GPL-3.0-or-later
"""Freeze unmodified C# replies only after every Rust comparison passes."""
import argparse,hashlib,json
from pathlib import Path
from check import projection
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('evidence',type=Path,nargs='+')
parser.add_argument('--output',type=Path,required=True)
parser.add_argument('--baseline',default='45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46',help='Exact unmodified C# source commit used for capture')
args=parser.parse_args();rows=[];captures=[]
for folder in args.evidence:
    report=json.loads((folder/'report.json').read_text(encoding='utf-8'))
    meta=json.loads((folder/'reference.json').read_text(encoding='utf-8'))
    assert meta['capture_sha256']==hashlib.sha256((folder/'csharp.jsonl').read_bytes()).hexdigest()
    assert all(r['matches'] for r in report['checks'])
    cases=json.loads((folder/'cases.json').read_text(encoding='utf-8'))
    replies=[json.loads(line) for line in (folder/'csharp.jsonl').read_text(encoding='utf-8-sig').splitlines()]
    for case,reply in zip(cases,replies,strict=True):
        assert case['name']==reply['name']
        assert len(case['keys'])==len(reply['expected'])
        case['expected']=[projection(r) for r in reply['expected']];rows.append(case)
    captures.append(dict(reference=json.loads((folder/'reference.json').read_text(encoding='utf-8')),cases_sha256=hashlib.sha256((folder/'cases.json').read_bytes()).hexdigest()))
args.output.write_text(json.dumps(dict(baseline=args.baseline,captures=captures,cases=rows),ensure_ascii=False,separators=(',',':'))+'\n',encoding='utf-8')
print('frozen',len(rows))
