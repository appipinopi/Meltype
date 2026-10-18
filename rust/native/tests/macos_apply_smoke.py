# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Actual macOS edit ABI: query/mutation order and stopped failing callbacks."""
import ctypes as c,json,sys
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
Host=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_uint32,c.c_uint64,c.c_uint64,c.POINTER(c.c_uint16),c.c_uint32,c.POINTER(c.c_uint64))
api.meltype_macos_apply_utf8.argtypes=[c.c_char_p,c.c_uint32,c.c_void_p,Host]
api.meltype_macos_apply_utf8.restype=c.c_int
reply=json.dumps({"commits":[{"deleteBefore":3,"text":"仮名","expect":"前😀"},{"deleteBefore":2,"text":"終","expect":"仮名"}],"view":None}).encode()
def run(payload,fail=None):
    document="前😀";trace=[];buffers=[];errors=[]
    @Host
    def callback(_,op,location,length,text,count,out):
        nonlocal document
        try:
            trace.append(op)
            if op==fail:return 0
            units=document.encode("utf-16-le")
            if op==0:out[0]=(1<<63)-1;out[1]=0
            elif op==1:out[0]=len(units)//2;out[1]=0
            elif op==2:
                actual=units[location*2:(location+length)*2]
                buffer=(c.c_uint16*max(1,len(actual)//2))()
                if actual:c.memmove(buffer,actual,len(actual))
                buffers.append(buffer);out[0]=c.addressof(buffer);out[1]=len(actual)//2
            elif op==3:
                value=c.string_at(text,count*2)
                if location==(1<<63)-1:units+=value
                else:units=units[:location*2]+value+units[(location+length)*2:]
                document=units.decode("utf-16-le")
            elif op!=4:raise AssertionError(op)
            return 1
        except BaseException as error:errors.append(error);return 0
    result=api.meltype_macos_apply_utf8(payload,len(payload),None,callback)
    assert not errors,errors
    return result,document,trace
result,document,trace=run(reply)
assert result==1 and document=="終" and trace==[0,1,2,3,0,1,2,3,4],(result,document,trace)
for fail in range(5):
    result,document,trace=run(reply,fail)
    assert result==0 and trace[-1]==fail,(fail,result,trace)
for payload in [b"{",b'{"commits":[{"deleteBefore":0,"text":"x"},{}]}',b'{"commits":[],"view":null} trailing']:
    assert run(payload)==(0,"前😀",[]),payload
mismatch=json.dumps({"commits":[{"deleteBefore":3,"text":"NG","expect":"違文字"},{"deleteBefore":0,"text":"later"}],"view":{"text":"NG"}}).encode()
assert run(mismatch)==(0,"前😀",[0,1,2])
print("macOS Rust apply ABI: dependent edits, live query order, five host-failure stops, full validation before callbacks and mismatch stopping later edits/view passed")
