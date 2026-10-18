# SPDX-License-Identifier: GPL-3.0-or-later
"""Selected-text reconversion through real callbacks and a transactional host."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile

P = c.c_void_p
allocator = c.CDLL("ucrtbase") if os.name == "nt" else c.CDLL(None)
allocator.malloc.argtypes = [c.c_size_t]
allocator.malloc.restype = P
callback = c.CFUNCTYPE(P, c.c_char_p)
clause_callback = c.CFUNCTYPE(P, c.c_char_p, c.c_char_p)
engine = "identity"
queries = []
def allocate(text):
    data = text.encode() + b"\0"
    pointer = allocator.malloc(len(data))
    assert pointer
    c.memmove(pointer, data, len(data))
    return pointer
@callback
def convert(raw):
    reading = raw.decode()
    queries.append(reading)
    if engine == "null": return None
    if engine == "empty": return allocate("")
    return allocate("【" + reading + "】" if engine == "bracket" else reading)
@clause_callback
def clauses(raw, _context):
    reading = raw.decode()
    return allocate(reading + "\t" + ("【" + reading + "】" if engine == "bracket" else reading))

filename = "meltype_native.dll" if os.name == "nt" else "libmeltype_native.dylib" if sys.platform == "darwin" else "libmeltype_native.so"
api = c.CDLL(str(Path(sys.argv[1] if len(sys.argv)>1 else Path("target/release")/filename).resolve()))
api.meltype_init.argtypes = [P,P,P]
assert api.meltype_init(c.cast(clauses,P),None,None)==1
api.meltype_set_converter.argtypes = [callback]
assert api.meltype_set_converter(convert)==1
api.meltype_create_at.argtypes=[c.c_char_p]
api.meltype_create_at.restype=P
api.meltype_reconvert_selection.argtypes=[P,c.c_char_p,c.c_char_p]
api.meltype_reconvert_selection.restype=P
api.meltype_handle_key.argtypes=[P,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p]
api.meltype_handle_key.restype=P
api.meltype_reset.argtypes=[P]
api.meltype_reset.restype=P
api.meltype_set_direct.argtypes=[P,c.c_int]
api.meltype_set_private.argtypes=[P,c.c_int]
api.meltype_control.argtypes=[P,c.c_char_p]
api.meltype_control.restype=P
api.meltype_destroy.argtypes=[P]
api.meltype_free.argtypes=[P]
def result(pointer):
    assert pointer
    try: return json.loads(c.string_at(pointer))
    finally: api.meltype_free(pointer)

golden=json.loads((Path(__file__).resolve().parents[2]/"core/tests/data/upstream-reconversion.json").read_text(encoding="utf-8"))
with tempfile.TemporaryDirectory(prefix="meltype-reconversion-") as directory:
    root=Path(directory)
    (root/"config.json").write_text(json.dumps(dict(LiveConversion=False,PredictiveCandidates=False,LearningEnabled=False)),encoding="utf-8")
    for row in golden["cases"]:
        case=row["case"]
        engine=case["engine"]
        queries.clear()
        handle=api.meltype_create_at(str(root).encode())
        assert handle
        try:
            api.meltype_set_direct(handle,int(case["direct"]))
            value=result(api.meltype_reconvert_selection(handle,case["text"].encode(),case["reading"].encode() if case["reading"] else None))
            assert value["consumed"], (case["name"],value)
            assert value["selectionExpect"]==case["text"]
            assert not value["commits"], (case["name"],"committed before confirmation")
            expected=row["start"]
            for field,reference in [("text","Text"),("converting","Converting"),("clauses","Clauses"),("selectedIndex","SelectedIndex"),("selectedClause","SelectedClause")]:
                assert value["view"][field]==expected[reference], (case["name"],field,expected[reference],value["view"][field],queries)
            if case["text"].isascii():
                assert value["view"]["candidates"]==expected["Candidates"], (case["name"],expected,value)
            if case["action"]=="original":
                value=result(api.meltype_handle_key(handle,40,0,0,None,None))
                assert value["view"]["text"]==case["text"]
            if case["action"]=="stale":
                # A transactional host must abandon the composition when its
                # saved selection/range is invalidated; no stale edit is sent.
                result(api.meltype_reset(handle))
            value=result(api.meltype_handle_key(handle,27 if case["action"]=="escape" else 13,0,0,None,None))
            document=case["text"]
            if case["action"]!="stale":
                document="".join(edit["text"] for edit in value["commits"])
            assert document==row["document"], (case["name"],row["document"],document)
            assert value["view"] is None
        finally: api.meltype_destroy(handle)
    for engine in ["null", "empty"]:
        handle=api.meltype_create_at(str(root).encode())
        try:
            value=result(api.meltype_reconvert_selection(handle,b"nihongo",None))
            assert value["view"]["text"]=="にほんご", (engine,value)
            assert value["selectionExpect"]=="nihongo" and not value["commits"]
            value=result(api.meltype_handle_key(handle,27,0,0,None,None))
            assert value["commits"][0]["text"]=="nihongo"
        finally: api.meltype_destroy(handle)
    handle=api.meltype_create_at(str(root).encode())
    try:
        queries.clear()
        api.meltype_set_private(handle,1)
        value=result(api.meltype_reconvert_selection(handle,b"nihongo",None))
        assert not value["consumed"] and not queries and not value["commits"]
        api.meltype_set_private(handle,0)
        result(api.meltype_control(handle,b'{"op":"settings_set","name":"Enabled","value":false}'))
        value=result(api.meltype_reconvert_selection(handle,b"nihongo",None))
        assert not value["consumed"] and not queries
        result(api.meltype_control(handle,b'{"op":"settings_set","name":"Enabled","value":true}'))
        value=result(api.meltype_handle_key(handle,65,97,0,None,None))
        before=value["view"]
        for text,reading in [(b"nihongo",None),("橋".encode(),"はし".encode())]:
            value=result(api.meltype_reconvert_selection(handle,text,reading))
            assert not value["consumed"] and not queries
            assert value["view"]==before and not value["commits"]
        result(api.meltype_reset(handle))
        api.meltype_destroy(handle)
        handle=None
        assert not api.meltype_reconvert_selection(None,b"nihongo",None) and not queries
    finally:
        if handle: api.meltype_destroy(handle)
print(f"Reconversion native ABI: {len(golden['cases'])} original workflows passed")
print("Private/disabled/busy/invalid sessions: no converter calls or edits")
print("Null/empty standalone conversion replies retain the reading and original choice")
