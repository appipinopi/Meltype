#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Package an unsigned DEVICE app, including its required keyboard extension."""
from pathlib import Path
import plistlib
import sys
import zipfile


def package(app, archive):
    app, archive = Path(app), Path(archive)
    for bundle in (app, app / 'PlugIns/MeltypeFlashKeyboard.appex'):
        info = plistlib.loads((bundle / 'Info.plist').read_bytes())
        if info.get('DTPlatformName') != 'iphoneos':
            raise ValueError('IPA requires a device build, not a Simulator build')
        binary = bundle / info['CFBundleExecutable']
        header = binary.read_bytes()[:8]
        if header != bytes.fromhex('cffaedfe0c000001'):
            raise ValueError('Expected an arm64 Mach-O binary')
        if bundle != app:
            extension = info['NSExtension']
            if extension['NSExtensionPointIdentifier'] != 'com.apple.keyboard-service':
                raise ValueError('Keyboard extension missing')
            if extension['NSExtensionAttributes'].get('RequestsOpenAccess', True):
                raise ValueError('Keyboard must not require full access')
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, 'x', zipfile.ZIP_DEFLATED) as zipped:
        for path in sorted(app.rglob('*')):
            if path.is_file():
                zipped.write(path, 'Payload/' + app.name + '/' + path.relative_to(app).as_posix())
    with zipfile.ZipFile(archive) as zipped:
        if zipped.testzip() is not None:
            raise ValueError('Corrupted IPA')


if __name__ == '__main__':
    package(sys.argv[1], sys.argv[2])
