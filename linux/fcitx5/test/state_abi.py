# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Real C ABI/state lifetime tests using only private, temporary profiles."""
import ctypes as c
import json
from pathlib import Path
import sys
import tempfile

api = c.CDLL(str(Path(sys.argv[1]).resolve()))
api.meltype_fcitx_create_at.argtypes = [c.c_char_p]
api.meltype_fcitx_create_at.restype = c.c_void_p
for name in ("meltype_fcitx_destroy", "meltype_destroy"):
    getattr(api, name).argtypes = [c.c_void_p]
    getattr(api, name).restype = None
api.meltype_fcitx_state.argtypes = [c.c_void_p]
api.meltype_fcitx_state.restype = c.c_int
for name in ("meltype_fcitx_set_direct", "meltype_fcitx_set_preedit"):
    getattr(api, name).argtypes = [c.c_void_p, c.c_int]
    getattr(api, name).restype = None
api.meltype_handle_key.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_char_p, c.c_char_p]
api.meltype_handle_key.restype = c.c_void_p
api.meltype_free.argtypes = [c.c_void_p]
api.meltype_free.restype = None


def key(handle):
    pointer = api.meltype_handle_key(handle, 65, 97, 0, None, None)
    assert pointer, "live core reply"
    try:
        return json.loads(c.string_at(pointer).decode("utf-8"))
    finally:
        api.meltype_free(pointer)


with tempfile.TemporaryDirectory(prefix="meltype-fcitx-state-") as directory:
    profile = Path(directory)
    (profile / "config.json").write_text('{"DetectionLevel":"Manual"}', encoding="utf-8")
    first = api.meltype_fcitx_create_at(str(profile).encode("utf-8"))
    second = api.meltype_fcitx_create_at(str(profile).encode("utf-8"))
    assert first and second and first != second
    try:
        assert api.meltype_fcitx_state(first) == api.meltype_fcitx_state(second) == 0
        api.meltype_fcitx_set_direct(first, 1)
        api.meltype_fcitx_set_preedit(first, 1)
        assert api.meltype_fcitx_state(first) == 3 and api.meltype_fcitx_state(second) == 0
        assert not key(first)["consumed"], "frontend mode reaches the actual core"
        assert key(second)["consumed"], "independent Japanese core"
        api.meltype_fcitx_set_direct(first, 0)
        assert api.meltype_fcitx_state(first) == 2
        api.meltype_fcitx_set_preedit(first, 0)
        assert api.meltype_fcitx_state(first) == 0
        assert key(first)["consumed"], "switch back to Japanese"
        api.meltype_destroy(first)
        assert api.meltype_fcitx_state(first) == -1, "generic destruction clears frontend ownership"
        api.meltype_fcitx_set_direct(first, 1)
        api.meltype_fcitx_set_preedit(first, 1)
        assert api.meltype_fcitx_state(first) == -1, "stale writes cannot resurrect state"
        assert api.meltype_fcitx_state(second) == 0
        api.meltype_fcitx_destroy(second)
        api.meltype_fcitx_destroy(second)
        assert api.meltype_fcitx_state(second) == api.meltype_fcitx_state(None) == -1
        # Recreating a context receives a fresh token and default state.
        for _ in range(100):
            current = api.meltype_fcitx_create_at(str(profile).encode("utf-8"))
            assert current and current not in (first, second)
            assert api.meltype_fcitx_state(current) == 0
            api.meltype_fcitx_set_preedit(current, 1)
            api.meltype_fcitx_destroy(current)
            assert api.meltype_fcitx_state(current) == -1
    finally:
        api.meltype_fcitx_destroy(first)
        api.meltype_fcitx_destroy(second)
print("fcitx Rust state: independent contexts/core modes, visibility, generic/repeated destroy, stale handles and 100 recreations passed")
