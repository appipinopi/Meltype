# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import plistlib
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, ROOT / path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


linux = load('package_linux', 'rust/package-linux.py')
release = load('extend_release', 'tools/extend-flash-release.py')
ios = load('package_ios', 'ios/package.py')


class DistributionTests(unittest.TestCase):
    def test_checksums_reject_corruption(self):
        with tempfile.TemporaryDirectory() as directory:
            asset = Path(directory) / 'example.zip'
            asset.write_bytes(b'actual')
            sums = Path(directory) / 'SHA256SUMS.txt'
            sums.write_text(hashlib.sha256(b'actual').hexdigest() + '  example.zip\n')
            release.verify(asset, sums)
            asset.write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'SHA256'):
                release.verify(asset, sums)

    def test_linux_architecture_and_debian_contents(self):
        with tempfile.TemporaryDirectory() as directory:
            bundle = Path(directory) / 'bundle'
            bundle.mkdir()
            for name in ('meltype-ibus', 'meltype-flash', 'meltype-rs'):
                (bundle / name).write_bytes(b'\x7fELF\x02\x01' + b'\0' * 12 + (183).to_bytes(2, 'little'))
            for name in ('icon.png', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'install.sh'):
                (bundle / name).write_text('fixture')
            output = Path(directory) / 'output'
            with self.assertRaisesRegex(ValueError, 'architecture'):
                linux.package(bundle, output, '0.1.0-pre.2', 'amd64')
            self.assertFalse(output.exists())
            def inspect_deb(command, check):
                root = Path(command[-2])
                control = (root / 'DEBIAN/control').read_text()
                self.assertIn('Package: meltype-flash\n', control)
                self.assertIn('Architecture: arm64\n', control)
                self.assertIn('Version: 0.1.0~pre.2\n', control)
                xml = (root / 'usr/share/ibus/component/meltype-flash.xml').read_text()
                self.assertNotIn(str(bundle), xml)
                self.assertIn('/usr/libexec/meltype-flash/ibus/meltype-ibus', xml)
                self.assertTrue((root / 'usr/libexec/meltype-flash/ibus/LICENSE').exists())
            with patch.object(linux.subprocess, 'check_output', return_value=f'<exec>{bundle.resolve()}/meltype-ibus --ibus</exec>'), patch.object(linux.subprocess, 'run', side_effect=inspect_deb):
                linux.package(bundle, output, '0.1.0-pre.2', 'arm64')
            with zipfile.ZipFile(output / 'Meltype-flash-Linux-arm64.zip') as archive:
                self.assertIn('Meltype-flash-linux/install.sh', archive.namelist())

    def test_partial_release_is_never_uploaded(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(release.subprocess, 'run') as invoke:
            with self.assertRaisesRegex(ValueError, 'distribution'):
                release.publish('flash-v0.1.0-preview.2', directory)
            invoke.assert_not_called()

    def test_ipa_requires_device_and_preserves_keyboard_extension(self):
        with tempfile.TemporaryDirectory() as directory:
            app = Path(directory) / 'MeltypeFlash.app'
            extension = app / 'PlugIns/MeltypeFlashKeyboard.appex'
            extension.mkdir(parents=True)
            for bundle in (app, extension):
                info = {'DTPlatformName': 'iphoneos', 'CFBundleExecutable': 'binary'}
                if bundle == extension:
                    info['NSExtension'] = {'NSExtensionPointIdentifier': 'com.apple.keyboard-service', 'NSExtensionAttributes': {'RequestsOpenAccess': False}}
                (bundle / 'Info.plist').write_bytes(plistlib.dumps(info))
                (bundle / 'binary').write_bytes(bytes.fromhex('cffaedfe0c000001'))
            archive = Path(directory) / 'device.ipa'
            ios.package(app, archive)
            with zipfile.ZipFile(archive) as zipped:
                self.assertIn('Payload/MeltypeFlash.app/PlugIns/MeltypeFlashKeyboard.appex/binary', zipped.namelist())
            (extension / 'Info.plist').write_bytes(plistlib.dumps({'DTPlatformName': 'iphonesimulator'}))
            with self.assertRaisesRegex(ValueError, 'device build'):
                ios.package(app, Path(directory) / 'simulator.ipa')


if __name__ == '__main__':
    unittest.main()
