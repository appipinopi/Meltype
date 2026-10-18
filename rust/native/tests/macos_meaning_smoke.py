# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Actual reservation ABI: canonical keys, independent sessions and lifetime."""
import ctypes as c,tempfile,sys,unicodedata
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
P=c.c_void_p;U=c.c_uint32
api.meltype_macos_meaning_delay_ms.restype=U
assert api.meltype_macos_meaning_delay_ms()==1500
api.meltype_macos_candidate_window.argtypes=[P,U,c.c_int64,c.POINTER(c.c_uint16),U]
api.meltype_macos_candidate_window.restype=c.c_int
assert not api.meltype_macos_candidate_window(None,0,0,None,0)
assert not api.meltype_macos_candidate_window(P(1),99,0,None,0)
if sys.platform!="darwin":
    for operation in range(6):
        assert not api.meltype_macos_candidate_window(P(1),operation,1,None,0)
api.meltype_macos_server_create.argtypes=[c.POINTER(c.c_uint16),U,c.POINTER(c.c_uint16),U]
api.meltype_macos_server_create.restype=P
api.meltype_macos_candidates_create.argtypes=[P,c.c_uint64]
api.meltype_macos_candidates_create.restype=P
assert not api.meltype_macos_server_create(None,0,None,0)
assert not api.meltype_macos_candidates_create(None,1)
if sys.platform!="darwin":
    assert not api.meltype_macos_candidates_create(P(1),1)
callback_type=c.CFUNCTYPE(None,P)
api.meltype_macos_meaning_schedule.argtypes=[P,callback_type]
api.meltype_macos_meaning_schedule.restype=c.c_int
called=[]
callback=callback_type(lambda context:called.append(context))
assert not api.meltype_macos_meaning_schedule(None,callback)
assert not api.meltype_macos_meaning_schedule(P(1),callback_type())
if sys.platform!="darwin":
    assert not api.meltype_macos_meaning_schedule(P(1),callback)
assert not called,"rejected scheduling invoked callback"
api.meltype_macos_create_at.argtypes=[c.c_char_p];api.meltype_macos_create_at.restype=P
api.meltype_macos_meaning_update.argtypes=[P,c.c_int64,c.POINTER(c.c_uint16),U,c.c_int];api.meltype_macos_meaning_update.restype=c.c_int
api.meltype_macos_meaning_matches.argtypes=[P,c.c_int64,c.POINTER(c.c_uint16),U];api.meltype_macos_meaning_matches.restype=c.c_int
api.meltype_destroy.argtypes=[P]
def units(text):
    value=(text or "").encode("utf-16-le");buffer=(c.c_uint16*max(1,len(value)//2))()
    if value:c.memmove(buffer,value,len(value))
    return buffer,len(value)//2
def update(h,index,text):
    buffer,length=units(text);return api.meltype_macos_meaning_update(h,index,buffer,length,int(text is not None))
def matches(h,index,text):
    buffer,length=units(text);return api.meltype_macos_meaning_matches(h,index,buffer,length)
def key(index,text):return None if text is None else (index,unicodedata.normalize("NFD",text))
count=0
with tempfile.TemporaryDirectory(prefix="meltype-macos-meaning-") as directory:
    Path(directory,"config.json").write_text('{"DetectionLevel":"Manual"}',encoding="utf-8")
    first=api.meltype_macos_create_at(directory.encode());second=api.meltype_macos_create_at(directory.encode())
    assert first and second
    try:
        current=None;timers=[]
        for index,text in [(0,None),(0,""),(0,""),(1,"意味😀"),(1,"意味😀"),(1,"é"),(1,"e\u0301"),(2,"e\u0301"),(2,"a\0b"),(0,None),(1,"意味😀")]:
            expected=key(index,text)
            assert update(first,index,text)==int(expected is not None and expected!=current)
            current=expected
            if text is not None:timers.append((index,text))
            for prior_index,prior_text in timers:
                assert matches(first,prior_index,prior_text)==int(key(prior_index,prior_text)==current)
                assert not matches(second,prior_index,prior_text),"reservation leaked to another client"
                count+=1
        assert update(second,1,"独立")
        api.meltype_destroy(first)
        assert not update(first,0,"stale") and not matches(first,1,"意味😀")
        assert matches(second,1,"独立")
        broken=(c.c_uint16*1)(0xd800)
        assert not api.meltype_macos_meaning_update(second,0,broken,1,1)
        assert matches(second,1,"独立"),"invalid text changed reservation"
        for _ in range(100):
            fresh=api.meltype_macos_create_at(directory.encode());assert fresh and fresh not in (first,second)
            assert update(fresh,1,"意味😀")
            assert not matches(first,1,"意味😀"),"old callback matched recreated session"
            api.meltype_destroy(fresh)
    finally:api.meltype_destroy(first);api.meltype_destroy(second)
print(f"macOS Rust meaning: {count} reference timer-key checks, Unicode/NUL/empty/absent, independent sessions, invalid UTF16 and 100 recreations passed")
