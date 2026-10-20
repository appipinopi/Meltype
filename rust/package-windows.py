#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Assemble and inspect a Windows developer preview; never installs an IME."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import zipfile

REPO = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('package_audit', REPO/'tools/check-rust-windows-package.py')
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--settings', type=Path, required=True)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--worker-x64', type=Path, required=True)
    parser.add_argument('--worker-x86', type=Path, required=True)
    parser.add_argument('--tip-source', type=Path, default=REPO/'native/tip/bin')
    parser.add_argument('--redist-root', type=Path, required=True,
                        help='VC/Redist/MSVC/<version>; use redistributable CRTs, not System32 DLLs.')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    archive = output.with_suffix('.zip')
    if output.exists() or archive.exists():
        parser.error('choose new output and ZIP paths; existing artifacts are preserved')
    sources = {
        'meltype-settings.exe': args.settings,
        'meltype-rs.exe': args.cli,
        'tip/x64/MeltypeTip.dll': args.tip_source/'rust-x64/MeltypeTip.dll',
        'tip/x86/MeltypeTip.dll': args.tip_source/'rust-x86/MeltypeTip.dll',
        'tip/x64/meltype-tip-worker.exe': args.worker_x64,
        'tip/x86/meltype-tip-worker.exe': args.worker_x86,
        'LICENSE': REPO/'LICENSE',
        'THIRD-PARTY-NOTICES.md': REPO/'THIRD-PARTY-NOTICES.md',
        'README-Windows.md': REPO/'rust/WINDOWS-PACKAGE.md',
        'Manage-Windows.ps1': REPO/'rust/Manage-Windows.ps1',
        'Start-Portable.ps1': REPO/'rust/Start-Portable.ps1',
    }
    for path in sources.values():
        if not path.is_file():
            parser.error('missing input: ' + str(path))
    output.mkdir(parents=True)
    for name, source in sources.items():
        target = output/name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    # App-local CRTs must match each consumer, including the x64 settings app.
    # Reinspect transitive imports; only known redistributable names are copied.
    for _ in range(8):
        report = audit.inspect(output)
        missing = []
        for name, info in report['binaries'].items():
            for dependency in info['imports']:
                if dependency not in info['resolved_imports']:
                    missing.append((name, info['machine'], dependency))
        if not missing:
            break
        copied = False
        for name, machine, dependency in missing:
            if not re.fullmatch(r'(vcruntime|msvcp|concrt)\d+[^/\\]*\.dll', dependency):
                continue
            arch = {0x8664: 'x64', 0x14c: 'x86'}[machine]
            choices = list(args.redist_root.glob(f'{arch}/Microsoft.VC*.CRT/{dependency}'))
            if len(choices) != 1:
                raise SystemExit(f'cannot resolve {arch} redistributable {dependency}')
            shutil.copyfile(choices[0], (output/name).parent/dependency)
            copied = True
        if not copied:
            break
    report = audit.inspect(output)
    (output/'package-audit.json').write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    if not report['passed']:
        raise SystemExit(json.dumps(report['errors'], ensure_ascii=False))
    manifest = {p.relative_to(output).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(output.rglob('*')) if p.is_file()}
    (output/'SHA256SUMS.json').write_text(json.dumps(manifest, indent=2)+'\n', encoding='utf-8')
    with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED) as zip_file:
        for path in sorted(output.rglob('*')):
            if path.is_file():
                zip_file.write(path, path.relative_to(output).as_posix())
    with zipfile.ZipFile(archive) as zip_file:
        assert zip_file.testzip() is None
        for name, digest in manifest.items():
            assert hashlib.sha256(zip_file.read(name)).hexdigest() == digest
    print(json.dumps(dict(package=str(output), zip=str(archive), binaries=len(report['binaries']),
                          passed=True, installed=False, registered=False), ensure_ascii=False))


if __name__ == '__main__':
    main()
