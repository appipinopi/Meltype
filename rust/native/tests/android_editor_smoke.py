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
        str(root / "rust/native/tests/android/InputHost.java"),
        str(root / "rust/native/tests/android/ContextHost.java"),
        str(root / "rust/native/tests/android/KeyEvent.java"),
        str(root / "rust/native/tests/android/LifecycleHost.java"),
        str(root / "rust/native/tests/android/StateHost.java"),
        str(root / "rust/native/tests/android/WidgetHost.java"),
        *(str(path) for path in sorted((root / "rust/native/tests/android/widgets").glob("*.java"))),
    ], check=True)
    for host in sys.argv[2:] or ["WidgetHost", "StateHost", "LifecycleHost", "ContextHost", "InputHost", "EditorHost"]:
        subprocess.run([
            "java", "-Xcheck:jni", f"-Djava.library.path={library}", "-cp", output,
            f"moe.meltype.rust.{host}",
        ], check=True)
