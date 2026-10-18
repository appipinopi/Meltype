# SPDX-License-Identifier: GPL-3.0-or-later
"""Original hook queues and feedback through the real ABI; no OS hooks/injection."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile

filename="meltype_native.dll" if os.name=="nt" else "libmeltype_native.dylib" if sys.platform=="darwin" else "libmeltype_native.so"
api=c.CDLL(str(Path(sys.argv[1] if len(sys.argv)>1 else Path("target/release")/filename).resolve()))
P=c.c_void_p
api.meltype_create_at.argtypes=[c.c_char_p];api.meltype_create_at.restype=P
api.meltype_control.argtypes=[P,c.c_char_p];api.meltype_control.restype=P
api.meltype_destroy.argtypes=[P];api.meltype_free.argtypes=[P]
api.meltype_set_private.argtypes=[P,c.c_int]
def request(handle,value):
    pointer=api.meltype_control(handle,json.dumps(value,ensure_ascii=False).encode())
    assert pointer,"null control response"
    try:return json.loads(c.string_at(pointer))
    finally:api.meltype_free(pointer)
def project(row):
    for decision in [row["decision"]]+[f["result"] for f in row["flushes"]]:
        if decision:
            for contribution in decision["contributions"]:contribution.pop("reason",None)
    return row

golden=json.loads((Path(__file__).resolve().parents[2]/"core/tests/data/upstream-hook-session.json").read_text(encoding="utf8"))
with tempfile.TemporaryDirectory(prefix="meltype-hook-") as directory:
    root=Path(directory)
    for case in golden["cases"]:
        (root/"config.json").write_text(json.dumps(case["settings"]),encoding="utf8")
        model={key:dict(value,lastUsed="2026-10-18T00:00:00Z") for key,value in case["model"].items() if value is not None}
        (root/"state.json").write_text(json.dumps(dict(prefix_model=dict(version=1,prefixes=model))),encoding="utf8")
        handle=api.meltype_create_at(str(root).encode());assert handle
        try:
            assert len(case["steps"])==len(case["expected"])
            for i,(step,expected) in enumerate(zip(case["steps"],case["expected"],strict=True)):
                actual=request(handle,dict(op="hook_session",command=step))
                assert "error" not in actual,(case["name"],i,actual)
                assert project(actual)==expected,(case["name"],i,expected,actual)
            before=request(handle,dict(op="hook_session",command=dict(op="snapshot")))
            assert "error" in request(handle,dict(op="hook_session",command=dict(op="bad")))
            assert request(handle,dict(op="hook_session",command=dict(op="snapshot")))==before
        finally:api.meltype_destroy(handle)
    # Persist feedback through control, then reopen and verify the learned veto.
    (root/"config.json").write_text("{}",encoding="utf8")
    (root/"state.json").write_text("{}",encoding="utf8")
    handle=api.meltype_create_at(str(root).encode());assert handle
    try:
        assert request(handle,dict(op="prefix_learn",letters="kyo",outcome="JapaneseRejected"))["saved"]
        assert request(handle,dict(op="learning_list"))["prefixes"]["kyo"]["english"]==2
        api.meltype_set_private(handle,1)
        assert not request(handle,dict(op="prefix_learn",letters="private",outcome="JapaneseAccepted"))["saved"]
        assert "privat" not in request(handle,dict(op="learning_list"))["prefixes"]
    finally:api.meltype_destroy(handle)
    handle=api.meltype_create_at(str(root).encode());assert handle
    try:
        learned=request(handle,dict(op="detection_evaluate",text="kyou",final=False))
        assert any(v["source"]=="User" and v["english"]>0 for v in learned["contributions"]),learned
        assert request(handle,dict(op="learning_clear"))["saved"]
        assert not request(handle,dict(op="learning_list"))["prefixes"]
    finally:api.meltype_destroy(handle)
print(f"Hook native ABI: {len(golden['cases'])} original traces, durable feedback, privacy and invalid-command isolation passed")
