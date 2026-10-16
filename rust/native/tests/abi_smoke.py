# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Exercise real C callbacks/ownership and multi-edit results in a fresh process."""
import ctypes as c
import json
import os
import sys
import tempfile


def main(library):
    api = c.CDLL(library)
    allocator = c.CDLL("ucrtbase") if os.name == "nt" else c.CDLL(None)
    allocator.malloc.argtypes = [c.c_size_t]
    allocator.malloc.restype = c.c_void_p
    clauses_type = c.CFUNCTYPE(c.c_void_p, c.c_char_p, c.c_char_p)
    candidates_type = c.CFUNCTYPE(c.c_void_p, c.c_char_p)
    word_type = c.CFUNCTYPE(c.c_int, c.c_char_p)

    def allocate(text):
        value = text.encode("utf-8") + b"\0"
        ptr = allocator.malloc(len(value))
        if not ptr:
            return None
        c.memmove(ptr, value, len(value))
        return ptr

    @clauses_type
    def clauses(reading, _context):
        return allocate("はし\t橋") if reading == "はし".encode() else None

    @candidates_type
    def candidates(reading):
        return allocate("橋\n箸") if reading == "はし".encode() else None

    @word_type
    def is_word(_word):
        return 0

    api.meltype_init.argtypes = [clauses_type, candidates_type, word_type]
    api.meltype_init.restype = c.c_int
    api.meltype_create.restype = c.c_void_p
    api.meltype_destroy.argtypes = [c.c_void_p]
    api.meltype_free.argtypes = [c.c_void_p]
    api.meltype_handle_key.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_char_p, c.c_char_p]
    api.meltype_handle_key.restype = c.c_void_p
    api.meltype_reconvert_selection.argtypes = [c.c_void_p,c.c_char_p,c.c_char_p]
    api.meltype_reconvert_selection.restype = c.c_void_p
    api.meltype_commit.argtypes = [c.c_void_p]
    api.meltype_commit.restype = c.c_void_p
    api.meltype_control.argtypes = [c.c_void_p, c.c_char_p]
    api.meltype_control.restype = c.c_void_p
    api.meltype_set_can_delete.argtypes = [c.c_void_p, c.c_int]
    api.meltype_set_private.argtypes = [c.c_void_p, c.c_int]
    api.meltype_set_direct.argtypes = [c.c_void_p, c.c_int]
    api.meltype_set_direct_detection.argtypes = [c.c_void_p, c.c_int]
    api.meltype_undo_commit.argtypes = [c.c_void_p, c.c_char_p]
    api.meltype_undo_commit.restype = c.c_void_p

    def result(ptr):
        assert ptr, "null ABI response"
        try:
            return json.loads(c.string_at(ptr).decode("utf-8"))
        finally:
            api.meltype_free(ptr)

    with tempfile.TemporaryDirectory(prefix="meltype-abi-") as directory:
        os.environ["MELTYPE_RUST_DATA_DIR"] = directory
        assert api.meltype_init(clauses, candidates, is_word) == 1
        handle = api.meltype_create()
        assert handle
        try:
            def key(vk=0, scalar=0, modifiers=0):
                return result(api.meltype_handle_key(handle, vk, scalar, modifiers, None, None))

            for letter in "hashi":
                value = key(scalar=ord(letter))
            assert value["view"]["text"] == "橋"
            assert not value["view"]["converting"]
            assert key(32, 32)["view"]["clauses"] == ["橋"]
            value = key(scalar=ord("k"))
            assert value["commits"][0]["text"] == "橋"
            assert value["view"]["text"] == "k", "new preedit lost after old commit"
            assert key(scalar=ord("a"))["view"]["text"] == "か"
            assert result(api.meltype_commit(handle))["commits"][0]["text"] == "か"
            api.meltype_set_can_delete(handle, 1)
            value = result(api.meltype_undo_commit(handle, "橋か".encode()))
            assert value["commits"] == [{"deleteBefore": 1, "text": "", "expect": "か"}]
            assert value["view"]["text"] == "か"
            result(api.meltype_commit(handle))
            value = result(api.meltype_reconvert_selection(handle,"橋".encode(),None))
            assert value["selectionExpect"] == "橋"
            assert value["view"]["text"] == "橋"
            assert key(27)["commits"][0]["text"] == "橋"
            for letter in "hashi":
                key(scalar=ord(letter))
            value = key(32,32,1)
            assert [edit["text"] for edit in value["commits"]] == ["橋","　"]
            assert value["view"] is None
            api.meltype_set_private(handle, 1)
            assert not key(scalar=ord("a"))["consumed"]
        finally:
            api.meltype_destroy(handle)
        handle = api.meltype_create()
        try:
            api.meltype_set_direct(handle, 1)
            api.meltype_set_direct_detection(handle, 1)
            value = result(api.meltype_handle_key(handle, 78,110,0,b"kon",b""))
            assert not value["consumed"], "auto detection requires deletion capability"
            api.meltype_set_can_delete(handle, 1)
            value = result(api.meltype_handle_key(handle, 78,110,0,b"kon",b""))
            assert value["commits"] == [{"deleteBefore":3,"text":"","expect":"kon"}]
            assert value["view"]["text"] == "こん"
        finally:
            api.meltype_destroy(handle)
        handle = api.meltype_create()
        try:
            api.meltype_set_can_delete(handle, 1)
            for letter in "site PR":
                key(32 if letter == " " else 0, ord(letter))
            value = result(api.meltype_commit(handle))
            assert value["commits"] == [{"deleteBefore":2,"text":"して PR","expect":"して"}]
            def setting(name, value):
                response = result(api.meltype_control(handle, json.dumps({"op":"settings_set", "name":name, "value":value}).encode()))
                assert response.get("saved"), response

            setting("DetectionLevel", "Manual")
            setting("CorrectTypos", False)
            setting("TabConversion", False)
            for letter in "google":
                value = key(scalar=ord(letter))
            assert value["view"]["text"] == "ごおgぇ"
            assert "Tab → google" in value["view"]["hint"]
            assert value["view"]["suggestion"] == "google"
            value = key(9)
            assert value["consumed"] and value["view"]["text"] == "google"
            assert result(api.meltype_commit(handle))["commits"][0]["text"] == "google"
            setting("DetectionLevel", "Conservative")
            for letter in "amazon":
                value = key(scalar=ord(letter))
            assert value["view"]["text"] == "あまぞn"
            result(api.meltype_commit(handle))
            for letter in "google":
                value = key(scalar=ord(letter))
            assert value["view"]["text"] == "google"
        finally:
            api.meltype_destroy(handle)
        handle = api.meltype_create()
        try:
            setting("DetectionLevel", "Balanced")
            setting("SigilWordsDirect", True)
            for letter in "@src/app.ts":
                value = key(scalar=ord(letter))
                assert not value["consumed"] and value["view"] is None
                assert value["commits"] == [], "sigil names must reach the app once, without synthetic commits"
            for letter in "kyou":
                value = result(api.meltype_handle_key(handle, 0, ord(letter), 0, b"@src/app.ts ", b""))
            assert value["view"]["text"] == "きょう", "host-only whitespace must end sigil passthrough"
            result(api.meltype_commit(handle))
        finally:
            api.meltype_destroy(handle)
    print("Native ABI: callbacks, live conversion, multi-edit, guarded undo/correction, selection reconversion, detection levels, manual Tab, sigil passthrough and privacy passed")


if __name__ == "__main__":
    filename = "meltype_native.dll" if os.name == "nt" else (
        "libmeltype_native.dylib" if sys.platform == "darwin" else "libmeltype_native.so")
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.abspath(os.path.join("target", "release", filename)))
