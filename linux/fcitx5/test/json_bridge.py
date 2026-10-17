# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Build the production event adapter and compare real Rust/library values."""
from pathlib import Path
import subprocess
import sys
import tempfile

source = Path(__file__).resolve().with_suffix(".cpp")
library = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="meltype-fcitx-json-") as temporary:
    root = Path(temporary)
    binary = root / "json-bridge"
    profile = root / "profile"
    profile.mkdir()
    (profile / "config.json").write_text('{"DetectionLevel":"Manual"}', encoding="utf-8")
    subprocess.run(["g++", "-std=c++17", "-Wall", "-Wextra", "-Werror", str(source), "-ldl", "-o", str(binary)], check=True)
    subprocess.run([str(binary), str(library), str(profile)], check=True)
