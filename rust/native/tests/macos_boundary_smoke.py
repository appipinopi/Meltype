# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""The original Swift boundary fixtures, exercised through actual Rust C ABI."""
import ctypes as c
import json
from pathlib import Path
import sys
import tempfile
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
api.meltype_macos_create_at.argtypes=[c.c_char_p];api.meltype_macos_create_at.restype=c.c_void_p
api.meltype_macos_handle_key.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_macos_handle_key.restype=c.c_void_p
api.meltype_commit.argtypes=[c.c_void_p];api.meltype_commit.restype=c.c_void_p
api.meltype_free.argtypes=[c.c_void_p];api.meltype_destroy.argtypes=[c.c_void_p]
def result(pointer):
    assert pointer
    try:return json.loads(c.string_at(pointer))
    finally:api.meltype_free(pointer)
with tempfile.TemporaryDirectory(prefix="meltype-macos-boundary-") as directory:
    Path(directory,"config.json").write_text('{"SpaceAroundEnglish":true,"SigilWordsDirect":false}',encoding="utf-8")
    for raw,first,mode,expected in [("e",0x301,None,"e"),("ka",0x3099,None,"か"),("ka",0xe0100,None,"か"),("e",0x301,0x75,"え"),("ka",None,None,"か "),("ka",None,None,"か "),("@kuraido",0x301,None,"@kuraido"),("",0x301,None,None)]:
        handle=api.meltype_macos_create_at(directory.encode())
        try:
            for ch in raw:result(api.meltype_macos_handle_key(handle,ord(ch.upper()) if ch.isascii() and ch.isalpha() else 7,ord(ch),0,None,b"world"))
            if mode:result(api.meltype_macos_handle_key(handle,mode,0,0,None,None))
            actual=result(api.meltype_macos_handle_key(handle,7,first,0,None,None) if first else api.meltype_commit(handle))
            print(json.dumps({"raw":raw,"first":first,"expected":expected,"actual":actual},ensure_ascii=True))
            assert actual["view"] is None
            assert [edit["text"] for edit in actual["commits"]] == ([] if expected is None else [expected])
        finally:api.meltype_destroy(handle)
print("8 macOS external-text boundary ABI fixtures passed")

with tempfile.TemporaryDirectory(prefix="meltype-macos-protected-") as directory:
    Path(directory,"config.json").write_text('{"SigilWordsDirect":false}',encoding="utf-8")
    for vk,character,suffix in [(32,32," "),(9,9,"")]:
        handle=api.meltype_macos_create_at(directory.encode())
        try:
            for ch in "@kuraido":result(api.meltype_macos_handle_key(handle,ord(ch.upper()) if ch.isalpha() else 7,ord(ch),0,None,None))
            actual=result(api.meltype_macos_handle_key(handle,vk,character,0,None,None))
            assert actual["view"] is None and [edit["text"] for edit in actual["commits"]]==["@kuraido"+suffix],actual
        finally:api.meltype_destroy(handle)
print("2 macOS protected Space/Tab ABI fixtures passed")

callbacks=[]
Clause=c.CFUNCTYPE(c.c_void_p,c.c_char_p,c.c_char_p)
Candidates=c.CFUNCTYPE(c.c_void_p,c.c_char_p)
@Clause
def clauses(reading,context):
    callbacks.append(reading)
    return None
@Candidates
def candidates(reading):
    callbacks.append(reading)
    return None
api.meltype_init.argtypes=[c.c_void_p]*3
api.meltype_init(clauses,candidates,None)
with tempfile.TemporaryDirectory(prefix="meltype-macos-protected-converter-") as directory:
    Path(directory,"config.json").write_text('{"SigilWordsDirect":false,"LiveConversion":false}',encoding="utf-8")
    handle=api.meltype_macos_create_at(directory.encode())
    try:
        for ch in "kyouha「@kuraido":result(api.meltype_macos_handle_key(handle,ord(ch.upper()) if ch.isascii() and ch.isalpha() else 7,ord(ch),0,None,None))
        callbacks.clear()
        actual=result(api.meltype_macos_handle_key(handle,32,32,0,None,None))
        assert not callbacks,"protected tail sent to external converter"
        assert actual["view"] is None and [edit["text"] for edit in actual["commits"]]==["きょうは「@kuraido "],actual
    finally:api.meltype_destroy(handle)
print("macOS protected-tail converter bypass passed")

with tempfile.TemporaryDirectory(prefix="meltype-macos-converter-fallback-") as directory:
    Path(directory,"config.json").write_text('{"SigilWordsDirect":false,"LiveConversion":false}',encoding="utf-8")
    handle=api.meltype_macos_create_at(directory.encode())
    try:
        for ch in "kyouha「@kuraido」ashita":result(api.meltype_macos_handle_key(handle,ord(ch.upper()) if ch.isascii() and ch.isalpha() else 7,ord(ch),0,None,None))
        callbacks.clear()
        result(api.meltype_macos_handle_key(handle,32,32,0,None,None))
        assert callbacks and all(b"@" not in reading for reading in callbacks), callbacks
        actual=result(api.meltype_macos_handle_key(handle,13,0,0,None,None))
        assert [edit["text"] for edit in actual["commits"]]==["きょうは「@kuraido」あした"],actual
    finally:api.meltype_destroy(handle)
print("macOS missing-converter complete-reading fallback passed")

api.meltype_init(None,None,None)

with tempfile.TemporaryDirectory(prefix="meltype-macos-explicit-") as directory:
    Path(directory,"config.json").write_text('{"SigilWordsDirect":false,"LiveConversion":false}',encoding="utf-8")
    for raw,vk,expected in [("kyouha「@kuraido」ashita",9,"きょうは「@kuraido」あした"),("@kuraido",0x78,"＠ｋｕｒａｉｄｏ")]:
        handle=api.meltype_macos_create_at(directory.encode())
        try:
            for ch in raw:result(api.meltype_macos_handle_key(handle,ord(ch.upper()) if ch.isascii() and ch.isalpha() else 7,ord(ch),0,None,None))
            actual=result(api.meltype_macos_handle_key(handle,vk,0,0,None,None))
            if vk==0x78:actual=result(api.meltype_macos_handle_key(handle,13,0,0,None,None))
            assert actual["view"] is None and [edit["text"] for edit in actual["commits"]]==[expected],actual
        finally:api.meltype_destroy(handle)
print("macOS mixed-protection Tab and explicit F9 fixtures passed")

# Registered callbacks must be reached by Tab conversion as well as Space.
import os
allocator=c.CDLL("ucrtbase") if os.name=="nt" else c.CDLL(None)
allocator.malloc.argtypes=[c.c_size_t];allocator.malloc.restype=c.c_void_p
calls=[]
def owned(value):
    value=value.encode()+b"\0"
    pointer=allocator.malloc(len(value))
    assert pointer
    c.memmove(pointer,value,len(value))
    return pointer
@Clause
def tab_clauses(reading,context):
    calls.append(("clauses",reading))
    return owned("はし\t橋") if reading=="はし".encode() else None
@Candidates
def tab_candidates(reading):
    calls.append(("candidates",reading))
    return owned("橋\n箸") if reading=="はし".encode() else None
api.meltype_init(tab_clauses,tab_candidates,None)
with tempfile.TemporaryDirectory(prefix="meltype-macos-tab-converter-") as directory:
    Path(directory,"config.json").write_text('{"DetectionLevel":"Manual","PredictiveCandidates":false,"LiveConversion":false,"TabConversion":true}',encoding="utf-8")
    handle=api.meltype_macos_create_at(directory.encode())
    try:
        for ch in "hashi":result(api.meltype_macos_handle_key(handle,ord(ch.upper()),ord(ch),0,None,None))
        actual=result(api.meltype_macos_handle_key(handle,9,9,0,None,None))
        assert actual["view"] and actual["view"]["text"]=="橋",actual
        assert ("clauses","はし".encode()) in calls,calls
        calls.clear()
        moved=result(api.meltype_macos_handle_key(handle,9,9,0,None,None))
        assert moved["view"]["converting"] and not calls,"candidate Tab refetched conversion"
        moved=result(api.meltype_macos_handle_key(handle,9,9,1,None,None))
        assert moved["view"]["text"]=="橋" and not calls,"candidate Shift+Tab refetched conversion"
        actual=result(api.meltype_macos_handle_key(handle,13,0,0,None,None))
        assert [edit["text"] for edit in actual["commits"]]==["橋"],actual
    finally:api.meltype_destroy(handle)
api.meltype_init(None,None,None)
print("macOS Tab conversion reached registered native callbacks and committed their result")
