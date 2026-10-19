# SPDX-License-Identifier: GPL-3.0-or-later
"""Reproduce original results and separately preserved normalization failures."""
import json
import sys
from pathlib import Path
fixture = Path(__file__).resolve().parents[2] / 'core/tests/data/upstream-misspellings.json'
golden = json.loads(fixture.read_text(encoding='utf-8'))
actual = [json.loads(line) for line in Path(sys.argv[1]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
assert len(actual) == len(golden['cases']) == 596
for got, want in zip(actual, golden['cases']):
    assert got == want, (want['reading'], want, got)
errors = sum(row['error'] is not None for row in actual)
assert errors == golden['original_errors'] == 137
print('Original misspellings: 459 normal results and 137 preserved ArgumentException observations reproduced')
