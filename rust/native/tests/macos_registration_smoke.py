# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Malformed/unavailable ABI only; never registers an OS input source."""
import ctypes as c
import sys
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
api.meltype_macos_input_sources_normalized.argtypes=[c.c_void_p]
api.meltype_macos_input_sources_normalized.restype=c.c_void_p
api.meltype_macos_input_sources_repair.argtypes=[c.c_void_p]
api.meltype_macos_input_sources_repair.restype=c.c_int32
api.meltype_macos_input_source_register.argtypes=[]
api.meltype_macos_input_source_register.restype=c.c_int32
assert not api.meltype_macos_input_sources_normalized(None)
assert api.meltype_macos_input_sources_repair(None)==0
if sys.platform!="darwin":
    assert api.meltype_macos_input_source_register()==1
print("macOS Rust registration ABI: null/unavailable SDK rejected; real OS register/enable NOT_RUN")
