# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Finite, reproducible real-OS corpus from sampled or all bundled readings."""
import argparse,json,hashlib
from pathlib import Path
p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path);p.add_argument('--sample',type=int,default=128)
p.add_argument('--all',action='store_true',help='Include every bundled reading, not every possible OS input/state.')
a=p.parse_args();assert a.sample>1
source=Path('dictionaries/readings.txt');readings=sorted({line.split('\t')[0] for line in source.read_text(encoding='utf-8').splitlines() if line and not line.startswith('#')})
base=json.loads(Path('tools/parity/windows-converter/cases.json').read_text(encoding='utf-8'))
assert len(readings)>1
sample=readings if a.all else sorted({readings[n*(len(readings)-1)//(min(a.sample,len(readings))-1)] for n in range(min(a.sample,len(readings)))})
rows=base+[dict(name='system-reading:'+word,text=word) for word in sample]
a.output.mkdir(parents=True,exist_ok=False)
(a.output/'cases.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
(a.output/'coverage.json').write_text(json.dumps(dict(readings=len(readings),sampled_readings=len(sample),all_bundled_readings_included=len(sample)==len(readings),requests=len(rows),readings_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),all_possible_os_responses_verified=False),indent=2),encoding='utf-8')
print(len(rows),'requests from',len(readings),'system readings; not exhaustive OS coverage')
