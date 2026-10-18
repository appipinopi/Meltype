# SPDX-License-Identifier: GPL-3.0-or-later
"""Scored original C# traces through the native ABI, using private profiles."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile

filename = "meltype_native.dll" if os.name == "nt" else "libmeltype_native.dylib" if sys.platform == "darwin" else "libmeltype_native.so"
api = c.CDLL(str(Path(sys.argv[1] if len(sys.argv) > 1 else Path("target/release") / filename).resolve()))
P = c.c_void_p
api.meltype_init.argtypes = [P, P, P]
assert api.meltype_init(None, None, None) == 1
api.meltype_create_at.argtypes = [c.c_char_p]
api.meltype_create_at.restype = P
api.meltype_handle_key.argtypes = [P, c.c_int, c.c_int, c.c_int, c.c_char_p, c.c_char_p]
api.meltype_handle_key.restype = P
api.meltype_destroy.argtypes = [P]
api.meltype_free.argtypes = [P]
api.meltype_set_can_delete.argtypes = [P, c.c_int]

golden = json.loads((Path(__file__).resolve().parents[2] / "core/tests/data/upstream-scored-keys.json").read_text(encoding="utf-8"))
with tempfile.TemporaryDirectory(prefix="meltype-scored-") as directory:
    root = Path(directory)
    (root / "config.json").write_text(json.dumps(dict(ScoredSegmentation=True, LearningEnabled=False,
                                                     LiveConversion=False, PredictiveCandidates=False)), encoding="utf-8")
    for case in golden["cases"]:
        handle = api.meltype_create_at(str(root).encode())
        assert handle
        api.meltype_set_can_delete(handle, 1)
        assert len(case["keys"]) == len(case["expected"])
        try:
            for index, (key, expected) in enumerate(zip(case["keys"], case["expected"], strict=True)):
                pointer = api.meltype_handle_key(handle, *key, None, None)
                assert pointer, (case["name"], index, "null native response")
                try:
                    value = json.loads(c.string_at(pointer))
                finally:
                    api.meltype_free(pointer)
                view = value["view"]
                actual = dict(consumed=value["consumed"], commits=value["commits"], text=view["text"] if view else None,
                              converting=view["converting"] if view else False)
                assert actual == expected, (case["name"], index, expected, actual)
        finally:
            api.meltype_destroy(handle)
print(f"Scored segmentation native ABI: {len(golden['cases'])} original key traces passed")
