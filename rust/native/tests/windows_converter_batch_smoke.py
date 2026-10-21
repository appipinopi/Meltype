# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise batch FFI bounds, response order, cleanup, and scalar compatibility."""
import ctypes as c
import json
from pathlib import Path
import sys

api = c.CDLL(str(Path(sys.argv[1]).resolve()))
api.meltype_windows_converter_probe.argtypes = [c.c_char_p]
api.meltype_windows_converter_probe.restype = c.c_void_p
api.meltype_free.argtypes = [c.c_void_p]


def probe(request):
    pointer = api.meltype_windows_converter_probe(json.dumps(request).encode())
    if not pointer:
        return None
    try:
        return json.loads(c.string_at(pointer))
    finally:
        api.meltype_free(pointer)


empty = dict(text='', trace=True)
scalar = probe(empty)
assert scalar and 'responses' not in scalar
for invalid in [dict(requests=[]), dict(requests=[empty]*257),
                dict(requests=[empty, dict(text='😀'*4097)])]:
    assert probe(invalid) is None
    assert probe(empty) == scalar, 'failed batch must release COM ownership and preserve the scalar API'
responses = probe(dict(requests=[empty, dict(text='')]))['responses']
assert len(responses) == 2 and responses[0] == scalar
assert 'candidateTimedOut' not in responses[1], 'each request retains its own trace flag and position'
assert all(not response['candidates'] for response in probe(dict(requests=[empty]*256))['responses'])
print('Windows batch probe: bounds, Unicode limits, per-request trace/order, cleanup and scalar compatibility passed')
