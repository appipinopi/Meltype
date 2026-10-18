# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Actual candidate ABI: incremental updates and stopped SDK failures."""
import ctypes as c,json,sys
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
Host=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_uint32,c.c_int64)
api.meltype_macos_candidates_utf8.argtypes=[c.c_char_p,c.c_uint32,c.c_void_p,Host]
api.meltype_macos_candidates_utf8.restype=c.c_int
def run(value,fail=-1,supported=1):
    data=json.dumps(value,ensure_ascii=False).encode() if isinstance(value,dict) else value
    trace=[]
    @Host
    def callback(_,op,arg):
        trace.append((op,arg))
        return -1 if op==fail else supported if op==2 else 1
    result=api.meltype_macos_candidates_utf8(data,len(data),None,callback)
    return result,trace
value=dict(candidates=["一","二","😀"],selectedIndex=2,previous=[],visible=False,windowIndex=1)
expected=[(0,0),(1,0),(2,1),(3,1),(3,1),(4,2),(5,0)]
assert run(value)==(1,expected)
for fail in range(6):
    result,trace=run(value,fail)
    assert result==0 and trace[-1][0]==fail,(fail,result,trace)
assert run(value,supported=0)==(1,[(0,0),(1,0),(2,1),(5,0)])
canonical=dict(candidates=["é","二"],selectedIndex=1,previous=["e\u0301","二"],visible=True,windowIndex=1)
assert run(canonical)==(1,[(0,0),(2,-1),(4,1),(5,0)])
assert run(dict(value,candidates=[]))==(1,[(6,0)])
assert run(dict(value,candidates=[]),fail=6)==(0,[(6,0)])
for payload in [b"{",b'{"candidates":[false]}',b'null']:
    assert run(payload)==(0,[])
print("macOS Rust candidate ABI: incremental order, selector refusal, canonical list equality, hide, seven SDK-failure stops and malformed input passed")
