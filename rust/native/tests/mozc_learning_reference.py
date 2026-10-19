# SPDX-License-Identifier: GPL-3.0-or-later
"""Ensure the original controller still emits the committed learning fixture."""
import json
import sys
from pathlib import Path
fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-engine-learning.json'
expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
actual=[json.loads(line) for line in Path(sys.argv[1]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
assert len(actual)==len(expected)==22
for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
print('Original controller: 22 learning eligibility/run/context cases reproduced')

if len(sys.argv)>2:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-clause-raw.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[2]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==36
    for got,want in zip(actual,expected):assert got==want,(want['case'],want,got)
    print('Original CompositionText: 36 inputs and all original UTF-16 RawForReading ranges reproduced')

if len(sys.argv)>3:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-clause-language.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[3]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==16
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original controller: 16 clause language learning cases reproduced')

if len(sys.argv)>4:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-clause-history.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[4]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==25
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original controller: 25 normal/translation history cases reproduced')

if len(sys.argv)>5:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-single-choice.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[5]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==16
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original public session: 16 single-conversion/preview choice learning cases reproduced')

if len(sys.argv)>6:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-engine-keys.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[6]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==50
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original public session: 50 function-key conversion/commit/engine-learning operations reproduced')

if len(sys.argv)>7:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-case-keys.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[7]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==96
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original public session: 96 F9/F10 visible case-switch sequences reproduced')

if len(sys.argv)>8:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-number-keys.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[8]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==39
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original controller: 39 expanded-state numeric selection reproduced')

if len(sys.argv)>9:
    fixture=Path(__file__).resolve().parents[2]/'core/tests/data/upstream-public-number-keys.json'
    expected=json.loads(fixture.read_text(encoding='utf-8'))['cases']
    actual=[json.loads(line) for line in Path(sys.argv[9]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
    assert len(actual)==len(expected)==48
    for got,want in zip(actual,expected):assert got==want,(want['case']['name'],want,got)
    print('Original controller: 48 public numeric candidate-selection key traces reproduced')
