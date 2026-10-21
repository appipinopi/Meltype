# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Real composition C ABI, publication ordering and fail-before-host parsing."""
import ctypes as c,json,sys
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
Host=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_uint32,c.c_uint64,c.c_uint64,c.c_uint32)
api.meltype_macos_composition_utf8.argtypes=[c.c_char_p,c.c_uint32,c.c_void_p,Host]
api.meltype_macos_composition_utf8.restype=c.c_int
reply=json.dumps({"text":"前😀後","converting":True,"clauses":["前","😀","後"],"selectedClause":1}).encode()
def run(payload,stop=-1):
    trace=[]
    @Host
    def callback(_,op,location,length,style):
        trace.append((op,location,length,style))
        return 0 if len(trace)==stop else 1
    result=api.meltype_macos_composition_utf8(payload,len(payload),None,callback)
    return result,trace
expected=[(0,0,1,1),(0,1,2,2),(0,3,1,1),(1,4,0,0)]
assert run(reply)==(1,expected)
for stop in range(1,5):assert run(reply,stop)==(0,expected[:stop])
for payload in [b"{",b'{"text":"x","converting":false,"clauses":[],"selectedClause":0} trailing',b'{"text":"x","converting":false,"clauses":[false],"selectedClause":0}']:
    assert run(payload)==(0,[])
print("macOS Rust composition ABI: UTF16 marks/publication order, four callback-stop boundaries and malformed-input rejection passed")
