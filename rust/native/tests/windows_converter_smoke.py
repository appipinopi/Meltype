# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare read-only Windows IFELanguage requests to the unmodified C# adapter."""
import ctypes as c
import json
from pathlib import Path
import sys

api=c.CDLL(str(Path(sys.argv[1]).resolve()))
api.meltype_windows_converter_probe.argtypes=[c.c_char_p]
api.meltype_windows_converter_probe.restype=c.c_void_p
api.meltype_free.argtypes=[c.c_void_p]
case_file=Path(sys.argv[3]) if len(sys.argv)>3 else Path(__file__).resolve().parents[3]/'tools/parity/windows-converter/cases.json'
cases=json.loads(case_file.read_text(encoding='utf-8'))
reference={row['name']:row for line in Path(sys.argv[2]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{') for row in [json.loads(line)]}
assert len(cases)==len(reference)
available=0;retries=0
for case in cases:
    expected={k:v for k,v in reference[case['name']].items() if k!='name'}
    for attempt in range(3):
        pointer=api.meltype_windows_converter_probe(json.dumps(dict(case,trace=True),ensure_ascii=False).encode())
        assert pointer,case['name']
        try:value=json.loads(c.string_at(pointer))
        finally:api.meltype_free(pointer)
        timed_out=value.pop('candidateTimedOut')
        # Both adapters deliberately abandon WinRT operations after 500 ms.
        # Retry only a reported timeout, never a conversion/segmentation/error
        # mismatch, and still require the complete original candidate list.
        if not timed_out or value['candidates']==expected['candidates']:break
        for field in ['converted','clauses']:assert value[field]==expected[field],(case['name'],field)
        retries+=1
    # Original IsAvailable is lazy; an empty request does not attempt Open.
    if not case['text']:expected.pop('available');value.pop('available')
    else:available+=bool(value['available'])
    for field in expected:
        assert value[field]==expected[field],(case['name'],field,'candidateTimedOut',timed_out,expected[field],value[field])
print(f"Windows converter: {len(cases)} original requests matched; {available} used an available Microsoft IME; {retries} bounded WinRT timeout retries")
