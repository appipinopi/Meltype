# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Call actual TSF COM vtables without registering/activating an input method.

Usage: python tsf_com_smoke.py path/to/meltype_native.dll [path/to/MeltypeTip.dll]
The optional second path also verifies the C++ provider delegates to Rust.
"""
import ctypes as c
import os
from pathlib import Path
import sys
import uuid

if os.name != "nt":
    raise SystemExit("This test requires Windows")

P = c.c_void_p
U32 = c.c_uint32
HR = c.c_int32
INVALID = -2147024809
NO_INTERFACE = -2147467262
NOT_IMPLEMENTED = -2147467263

class Guid(c.Structure):
    _fields_ = [("a", U32), ("b", c.c_uint16), ("c", c.c_uint16), ("d", c.c_ubyte * 8)]

    @classmethod
    def parse(cls, value):
        return cls.from_buffer_copy(uuid.UUID(value).bytes_le)

class Color(c.Structure):
    _fields_ = [("type", HR), ("value", U32)]

class Attribute(c.Structure):
    _fields_ = [("text", Color), ("background", Color), ("line", HR), ("bold", HR), ("line_color", Color), ("kind", HR)]

UNKNOWN = Guid.parse("00000000-0000-0000-c000-000000000046")
INFO = Guid.parse("70528852-2f26-4aea-8c96-215150578932")
ENUM = Guid.parse("7cef04d7-cb75-4e80-a7ab-5f5bc7d332de")
PROVIDER = Guid.parse("fee47777-163c-4769-996a-6e9c50ad8f54")
SPECS = [
    ("c8107d36-2806-4d8f-a183-fd0aeb8ed862", "Meltype: 入力中", 2, 0, 0),
    ("e17aa399-c5eb-42e6-be45-c44f0651f2e2", "Meltype: 変換した文節", 1, 0, 2),
    ("cef94813-c47f-48c1-b4ef-d4582d5d4054", "Meltype: 選んでいる文節", 1, 1, 1),
]

def invoke(pointer, slot, result, args, *values):
    table = c.cast(pointer, c.POINTER(c.POINTER(P))).contents
    return c.WINFUNCTYPE(result, P, *args)(table[slot])(pointer, *values)

def release(pointer):
    return invoke(pointer, 2, U32, [])

def assert_rust_vtable(pointer):
    # A C++ implementation with the same behavior must not satisfy this port test.
    table = c.cast(pointer, c.POINTER(c.POINTER(P))).contents
    kernel = c.WinDLL("kernel32", use_last_error=True)
    kernel.GetModuleHandleExW.argtypes = [U32, P, c.POINTER(P)]
    kernel.GetModuleFileNameW.argtypes = [P, c.c_wchar_p, U32]
    module = P()
    assert kernel.GetModuleHandleExW(6, table[3], c.byref(module))
    path = c.create_unicode_buffer(32768)
    assert kernel.GetModuleFileNameW(module, path, len(path))
    assert Path(path.value).name.lower() == "meltype_native.dll", path.value

def verify_info(pointer, index):
    assert_rust_vtable(pointer)
    expected, label, line, bold, kind = SPECS[index]
    identity = P()
    assert invoke(pointer, 0, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(UNKNOWN), c.byref(identity)) == 0
    assert identity.value == pointer
    release(identity)
    assert invoke(pointer, 0, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(INFO), c.byref(identity)) == 0
    release(identity)
    identity.value = 123
    assert invoke(pointer, 0, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(ENUM), c.byref(identity)) == NO_INTERFACE
    assert identity.value is None
    assert invoke(pointer, 0, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(INFO), None) == INVALID
    value = Guid()
    assert invoke(pointer, 3, HR, [c.POINTER(Guid)], c.byref(value)) == 0
    assert bytes(value) == uuid.UUID(expected).bytes_le
    assert invoke(pointer, 3, HR, [c.POINTER(Guid)], None) == INVALID
    text = P()
    assert invoke(pointer, 4, HR, [c.POINTER(P)], c.byref(text)) == 0
    allocator = c.WinDLL("oleaut32")
    allocator.SysStringLen.argtypes = [P]
    allocator.SysStringLen.restype = U32
    allocator.SysFreeString.argtypes = [P]
    try:
        assert c.wstring_at(text, allocator.SysStringLen(text)) == label
    finally:
        allocator.SysFreeString(text)
    assert invoke(pointer, 4, HR, [c.POINTER(P)], None) == INVALID
    attribute = Attribute()
    assert invoke(pointer, 5, HR, [c.POINTER(Attribute)], c.byref(attribute)) == 0
    assert (attribute.line, attribute.bold, attribute.kind) == (line, bold, kind)
    assert (attribute.text.type, attribute.background.type, attribute.line_color.type) == (0, 0, 0)
    assert invoke(pointer, 5, HR, [c.POINTER(Attribute)], None) == INVALID
    assert invoke(pointer, 6, HR, [c.POINTER(Attribute)], c.byref(attribute)) == NOT_IMPLEMENTED
    assert invoke(pointer, 7, HR, []) == 0

def verify_enum(pointer):
    assert_rust_vtable(pointer)
    identity = P()
    assert invoke(pointer, 0, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(ENUM), c.byref(identity)) == 0
    assert identity.value == pointer
    release(identity)
    values = (P * 4)()
    fetched = U32(77)
    assert invoke(pointer, 4, HR, [U32, c.POINTER(P), c.POINTER(U32)], 2, values, None) == INVALID
    assert invoke(pointer, 4, HR, [U32, c.POINTER(P), c.POINTER(U32)], 4, values, c.byref(fetched)) == 1
    assert fetched.value == 3
    for index in range(3):
        verify_info(values[index], index)
        assert release(values[index]) == 0
    assert invoke(pointer, 5, HR, []) == 0
    assert invoke(pointer, 6, HR, [U32], 1) == 0
    clone = P()
    assert invoke(pointer, 3, HR, [c.POINTER(P)], c.byref(clone)) == 0
    assert invoke(clone, 4, HR, [U32, c.POINTER(P), c.POINTER(U32)], 1, values, None) == 0
    verify_info(values[0], 1)
    release(values[0])
    release(clone)
    # Clone advances independently; skipping exactly to the end is S_OK.
    assert invoke(pointer, 6, HR, [U32], 2) == 0
    assert invoke(pointer, 6, HR, [U32], 0xffffffff) == 1
    assert invoke(pointer, 4, HR, [U32, c.POINTER(P), c.POINTER(U32)], 1, values, c.byref(fetched)) == 1
    assert fetched.value == 0
    assert release(pointer) == 0

def main():
    native = c.CDLL(str(Path(sys.argv[1]).resolve()))
    native.meltype_tsf_create_display_attribute.argtypes = [c.POINTER(Guid)]
    native.meltype_tsf_create_display_attribute.restype = P
    native.meltype_tsf_create_display_attribute_enum.restype = P
    assert not native.meltype_tsf_create_display_attribute(None)
    assert not native.meltype_tsf_create_display_attribute(c.byref(UNKNOWN))
    for index, spec in enumerate(SPECS):
        pointer = native.meltype_tsf_create_display_attribute(c.byref(Guid.parse(spec[0])))
        assert pointer
        verify_info(pointer, index)
        assert release(pointer) == 0
    verify_enum(native.meltype_tsf_create_display_attribute_enum())
    if len(sys.argv) > 2:
        tip = c.WinDLL(str(Path(sys.argv[2]).resolve()))
        tip.DllGetClassObject.argtypes = [c.POINTER(Guid), c.POINTER(Guid), c.POINTER(P)]
        tip.DllGetClassObject.restype = HR
        factory = P()
        assert tip.DllGetClassObject(c.byref(Guid.parse("417d801b-a9bd-4c26-bd16-356a825a6998")), c.byref(Guid.parse("00000001-0000-0000-c000-000000000046")), c.byref(factory)) == 0
        provider = P()
        assert invoke(factory, 3, HR, [P, c.POINTER(Guid), c.POINTER(P)], None, c.byref(PROVIDER), c.byref(provider)) == 0
        release(factory)
        enumerator = P()
        assert invoke(provider, 3, HR, [c.POINTER(P)], c.byref(enumerator)) == 0
        verify_enum(enumerator.value)
        pointer = P()
        assert invoke(provider, 4, HR, [c.POINTER(Guid), c.POINTER(P)], c.byref(Guid.parse(SPECS[2][0])), c.byref(pointer)) == 0
        release(provider)
        # Rust owns this object independently of the C++ text service.
        assert tip.DllCanUnloadNow() == 0
        verify_info(pointer.value, 2)
        release(pointer)
    print("TSF COM: three upstream display styles, identities, BSTR ownership, reference counts, partial enumeration, Clone/Reset/Skip and provider delegation passed")

if __name__ == "__main__":
    main()
