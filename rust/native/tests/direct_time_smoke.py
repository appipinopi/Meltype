# SPDX-License-Identifier: GPL-3.0-or-later
"""Native key API uses the real monotonic clock; isolated synthetic input only."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile
import time

filename = "meltype_native.dll" if os.name == "nt" else "libmeltype_native.dylib" if sys.platform == "darwin" else "libmeltype_native.so"
api = c.CDLL(str(Path(sys.argv[1] if len(sys.argv) > 1 else Path("target/release") / filename).resolve()))
P = c.c_void_p
api.meltype_create_at.argtypes = [c.c_char_p]
api.meltype_create_at.restype = P
api.meltype_handle_key.argtypes = [P, c.c_int, c.c_int, c.c_int, c.c_char_p, c.c_char_p]
api.meltype_handle_key.restype = P
for name in ("meltype_set_direct", "meltype_set_direct_detection", "meltype_set_can_delete"):
    getattr(api, name).argtypes = [P, c.c_int]
api.meltype_destroy.argtypes = [P]
api.meltype_free.argtypes = [P]


def key(handle, ch, before):
    pointer = api.meltype_handle_key(handle, ord(ch.upper()), ord(ch), 0, before.encode(), None)
    assert pointer
    try:
        return json.loads(c.string_at(pointer))
    finally:
        api.meltype_free(pointer)


with tempfile.TemporaryDirectory(prefix="meltype-direct-time-") as directory:
    root = Path(directory)
    (root / "config.json").write_text(json.dumps(dict(LiveConversion=False, LearningEnabled=False,
        PredictiveCandidates=False, DirectModeAutoDetect=True)), encoding="utf-8")
    # Warm dictionaries before measuring a real inter-key interval.
    warm = api.meltype_create_at(str(root).encode())
    assert warm
    try:
        for ch in "konn":
            key(warm, ch, "")
    finally:
        api.meltype_destroy(warm)
    for expired in (False, True):
        handle = api.meltype_create_at(str(root).encode())
        assert handle
        api.meltype_set_direct_detection(handle, 1)
        api.meltype_set_can_delete(handle, 1)
        api.meltype_set_direct(handle, 1)
        document = ""
        try:
            for ch in "kon":
                value = key(handle, ch, document)
                assert not value["consumed"] and not value["commits"]
                document += ch
            if expired:
                time.sleep(0.75)
            # Frontends repeat mode-setting on every key. This must preserve
            # the pending timer/latch until a real mode transition occurs.
            api.meltype_set_direct(handle, 1)
            value = key(handle, "n", document)
            if expired:
                assert not value["consumed"] and not value["commits"] and value["view"] is None
            else:
                assert value["consumed"]
                assert value["commits"] == [dict(deleteBefore=3, text="", expect="kon")]
                assert value["view"]["text"] == "こん"
        finally:
            api.meltype_destroy(handle)
print("Native timed direct input: real-clock expiry, guarded replacement and repeated mode setters passed")
