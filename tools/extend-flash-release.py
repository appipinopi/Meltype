#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Extend a Flash prerelease without replacing the existing platform binaries."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

REPO = 'appipinopi/Meltype'


def verify(path, checksums):
    expected = dict(line.split(None, 1)[::-1] for line in Path(checksums).read_text(encoding='utf-8-sig').splitlines() if line.strip())
    if hashlib.sha256(Path(path).read_bytes()).hexdigest() != expected.get(Path(path).name):
        raise ValueError('SHA256 mismatch: ' + str(path))


def publish(tag, directory):
    if not re.fullmatch(r'flash-v\d+\.\d+\.\d+-preview\.\d+', tag):
        raise ValueError('Invalid Flash tag')
    assets = sorted(Path(directory).iterdir())
    expected = {'Meltype-flash-Windows.zip', 'Meltype-flash-Windows-portable.zip',
                'Meltype-flash-0.1.0-pre.2-setup.exe', 'Meltype-flash-Linux-x64.zip',
                'Meltype-flash-Linux-arm64.zip', 'meltype-flash_0.1.0~pre.2_amd64.deb',
                'meltype-flash_0.1.0~pre.2_arm64.deb', 'Meltype-flash-iOS-Simulator-arm64.zip',
                'Meltype-flash-iOS-source.zip', 'Meltype-flash-iOS-unsigned.ipa', 'README-iOS.md'}
    if {p.name for p in assets} != expected or any(not p.is_file() or not p.stat().st_size for p in assets):
        raise ValueError('Missing, extra, or empty distribution assets')
    release = json.loads(subprocess.check_output(['gh', 'api', f'repos/{REPO}/releases/tags/{tag}']))
    if not release['prerelease'] or release['draft']:
        raise ValueError('Expected an existing published prerelease')
    names = {asset['name'] for asset in release['assets']}
    if names & expected:
        raise ValueError('Distribution assets already exist; refusing to overwrite')
    with tempfile.TemporaryDirectory() as temporary:
        subprocess.run(['gh', 'release', 'download', tag, '--repo', REPO, '--pattern', 'SHA256SUMS.txt', '--dir', temporary], check=True)
        checksums = Path(temporary) / 'SHA256SUMS.txt'
        old = checksums.read_text(encoding='utf-8')
        extra = ''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in assets)
        checksums.write_text(old.rstrip() + '\n' + extra, encoding='utf-8')
        subprocess.run(['gh', 'release', 'upload', tag, '--repo', REPO, *map(str, assets)], check=True)
        subprocess.run(['gh', 'release', 'upload', tag, '--repo', REPO, str(checksums), '--clobber'], check=True)
        sha = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
        notes = release['body'] + '\n\n## 追加の配布形式\n\n'
        notes += 'WindowsセットアップEXE・ポータブルZIP、Linux x64/ARM64 ZIP・debを追加しました。\n'
        notes += 'デスクトップのエンジンは同じPre2タグから作成しています。debはUbuntu 24.04以降向けです。\n'
        notes += 'iOSは初期プレビューです。unsigned.ipaはWindowsのAltStore Classicなどで自分のApple IDを使って署名して入れるための実機用IPAです。直接インストールはできず、キーボード拡張を残して署名する必要があります。Simulator ZIPはApple Silicon Mac用、source ZIPはXcode向けです。\n'
        notes += f'追加パッケージとiOSのソース: {REPO}@{sha}。手順は添付のREADME-iOS.mdを参照してください。\n'
        note_file = Path(temporary) / 'notes.md'
        note_file.write_text(notes, encoding='utf-8')
        subprocess.run(['gh', 'release', 'edit', tag, '--repo', REPO, '--notes-file', str(note_file)], check=True)


if __name__ == '__main__':
    if sys.argv[1] == 'verify':
        verify(sys.argv[2], sys.argv[3])
    elif sys.argv[1] == 'publish':
        publish(sys.argv[2], sys.argv[3])
    else:
        raise SystemExit('Expected verify or publish')
