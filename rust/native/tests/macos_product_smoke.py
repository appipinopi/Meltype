# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Original product self-test regressions through the actual macOS Rust ABI."""
import ctypes as c,json,os,sys,tempfile
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
P=c.c_void_p
allocator=c.CDLL("ucrtbase") if os.name=="nt" else c.CDLL(None)
allocator.malloc.argtypes=[c.c_size_t];allocator.malloc.restype=P
Clauses=c.CFUNCTYPE(P,c.c_char_p,c.c_char_p)
Candidates=c.CFUNCTYPE(P,c.c_char_p)
Word=c.CFUNCTYPE(c.c_int,c.c_char_p)
def allocate(text):
    raw=text.encode()+b"\0";p=allocator.malloc(len(raw));assert p;c.memmove(p,raw,len(raw));return p
seen=[]
@Clauses
def clauses(reading,before):
    text=reading.decode();seen.append(text)
    values={"きょうは":"今日は","でけんさく":"で検索"}
    return allocate(text+"\t"+values[text]) if text in values else None
@Candidates
def candidates(reading):return None
@Word
def word(value):return 0
api.meltype_init.argtypes=[Clauses,Candidates,Word]
assert api.meltype_init(clauses,candidates,word)==1
api.meltype_macos_create_at.argtypes=[c.c_char_p];api.meltype_macos_create_at.restype=P
api.meltype_macos_handle_key.argtypes=[P,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_macos_handle_key.restype=P
api.meltype_macos_select_candidate.argtypes=[P,c.c_int];api.meltype_macos_select_candidate.restype=P
api.meltype_commit.argtypes=[P];api.meltype_commit.restype=P
api.meltype_destroy.argtypes=[P];api.meltype_free.argtypes=[P]
def read(pointer):
    assert pointer
    try:return json.loads(c.string_at(pointer))
    finally:api.meltype_free(pointer)
def type_text(handle,text):
    committed="";last=None
    for ch in text:
        vk=13 if ch=="\n" else 32 if ch==" " else ord(ch.upper())
        last=read(api.meltype_macos_handle_key(handle,vk,ord(ch),int(ch.isupper()),None,None))
        committed+="".join(edit["text"] for edit in last["commits"])
        if not last["consumed"]:committed+=ch
    return committed,last
with tempfile.TemporaryDirectory(prefix="meltype-product-abi-") as directory:
    root=Path(directory)
    (root/"config.json").write_text('{"SigilWordsDirect":false}',encoding="utf-8")
    def session():
        handle=api.meltype_macos_create_at(str(root).encode());assert handle;return handle
    handle=session()
    try:
        text,last=type_text(handle,"kyouhagoogledekensaku \n")
        assert text=="今日はgoogleで検索",(text,last)
        assert "きょうは" in seen and "でけんさく" in seen
        assert all("google" not in reading for reading in seen),seen
    finally:api.meltype_destroy(handle)
    handle=session()
    try:
        seen.clear()
        _,last=type_text(handle,"googlede")
        assert last["view"]["text"]=="googleで",last
        assert seen==[],("live conversion must exclude short Japanese gaps",seen)
    finally:api.meltype_destroy(handle)
    handle=session()
    try:
        text,last=type_text(handle,"seeyouagain\n")
        assert text=="see you again",(text,last)
    finally:api.meltype_destroy(handle)
    handle=session()
    try:
        _,last=type_text(handle,"go")
        assert last["view"]["candidates"]==["go","ご"],last
        selected=read(api.meltype_macos_select_candidate(handle,0))
        assert selected["view"]["text"]=="go" and not selected["commits"],selected
        assert read(api.meltype_commit(handle))["commits"][0]["text"]=="go"
        type_text(handle,"go")
        selected=read(api.meltype_macos_select_candidate(handle,1))
        assert selected["view"]["text"]=="ご" and not selected["commits"],selected
        assert read(api.meltype_commit(handle))["commits"][0]["text"]=="ご"
    finally:api.meltype_destroy(handle)
print("macOS product ABI: mixed google protected from converter, English phrase spacing, go preview and deferred English/hiragana selection passed (mock converter; actual product self-test runs separately)")
