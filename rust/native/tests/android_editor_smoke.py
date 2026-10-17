# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Run actual JNI against a private JVM editor; no Android device is modified."""
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parents[3]
library = Path(sys.argv[1]).resolve()
with tempfile.TemporaryDirectory(prefix="meltype-jni-host-") as output:
    subprocess.run([
        "javac", "-encoding", "UTF-8", "--release", "17", "-d", output,
        str(root / "android/app/src/main/java/moe/meltype/rust/Native.java"),
        str(root / "rust/native/tests/android/EditorHost.java"),
    ], check=True)
    subprocess.run([
        "java", "-Xcheck:jni", f"-Djava.library.path={library}", "-cp", output,
        "moe.meltype.rust.EditorHost",
    ], check=True)
