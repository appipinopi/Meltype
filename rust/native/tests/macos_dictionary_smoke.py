# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Production dictionary merge ABI; SDK parse comparison runs on macOS."""
import ctypes as c,json,sys,unicodedata
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
P=c.c_void_p
Directory=c.CFUNCTYPE(P,P)
api.meltype_macos_dictionary_dialog.argtypes=[P,Directory]
api.meltype_macos_dictionary_dialog.restype=c.c_int32
assert api.meltype_macos_dictionary_dialog(None,Directory())==0
if sys.platform!="darwin":
    @Directory
    def unavailable_directory(context):
        raise AssertionError("unavailable SDK queried directory")
    assert api.meltype_macos_dictionary_dialog(None,unavailable_directory)==0
api.meltype_macos_dictionary_import.argtypes=[P,c.POINTER(c.c_uint16),c.c_uint32,c.POINTER(P)]
api.meltype_macos_dictionary_import.restype=c.c_int64
failure=P(123)
assert api.meltype_macos_dictionary_import(None,None,0,c.byref(failure))==-2
assert not failure.value
api.meltype_macos_dictionary_merge.argtypes=[P,c.c_uint32];api.meltype_macos_dictionary_merge.restype=P
api.meltype_macos_dictionary_entries.argtypes=[P,c.c_uint32,c.POINTER(P)];api.meltype_macos_dictionary_entries.restype=P
api.meltype_free.argtypes=[P]
def merge(entries,existing):
    data=json.dumps(dict(entries=entries,existing=existing),ensure_ascii=False).encode()
    buffer=c.create_string_buffer(data)
    pointer=api.meltype_macos_dictionary_merge(buffer,len(data));assert pointer
    try:return json.loads(c.string_at(pointer))
    finally:api.meltype_free(pointer)
assert merge([],"old")==dict(text="old",count=0)
assert merge([["かな","単語"],["かな","単語"]],"")==dict(text="かな\t単語\n",count=1)
assert merge([["e\u0301x","漢"]],"éx\t漢\n")==dict(text="éx\t漢\n",count=0)
assert merge([["a\0b","😀"]],"old")==dict(text="old\na\0b\t😀\n",count=1)
assert merge([["new","追加"]],"old\r\n")==dict(text="old\r\n\nnew\t追加\n",count=1)
for bad in [b"{",b'{"entries":null,"existing":""}',b'{"entries":[["one"]],"existing":""}']:
    assert not api.meltype_macos_dictionary_merge(c.create_string_buffer(bad),len(bad))
assert not api.meltype_macos_dictionary_merge(None,1)
if sys.platform!="darwin":
    failure=P(123)
    assert not api.meltype_macos_dictionary_entries(None,0,c.byref(failure))
    assert not failure.value
print("macOS Rust dictionary ABI: canonical duplicates, append/newline/CRLF/NUL/emoji, malformed request and unavailable SDK rejection passed")
