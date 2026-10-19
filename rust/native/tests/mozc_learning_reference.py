# SPDX-License-Identifier: GPL-3.0-or-later
"""Ensure the original controller still emits the committed learning fixture."""
import json
import sys
from pathlib import Path
fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-engine-learning.json'
expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
actual=[json.loads(line) for line in Path(sys.argv[1]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
assert len(actual)==len(expected)==20
for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
print('Original controller: 20 learning eligibility/run/context cases reproduced')
