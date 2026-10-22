import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('collect_release', Path(__file__).with_name('collect-rust-release.py'))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class CollectionTests(unittest.TestCase):
    def fixture(self, source):
        paths = [
            'release-windows/Meltype-flash-Windows-x64.zip',
            'release-linux/Meltype-flash-Linux-x64.tar.gz',
            'release-macos-arm64/Meltype-flash-macOS-arm64.zip',
            'release-macos-x64/Meltype-flash-macOS-x64.zip',
            'Meltype-ChromeOS-preview/chromeos-preview.zip',
            'meltype-rust-android-debug/android/app/build/outputs/apk/debug/app-debug.apk',
        ]
        for path in paths:
            file = source / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(b'fixture')
        # Instrumentation APKs must never be distributed as the application.
        (source / 'meltype-rust-android-debug/app-debug-androidTest.apk').write_bytes(b'test')

    def test_all_platforms_and_checksums(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'inputs'
            output = Path(directory) / 'output'
            self.fixture(source)
            release.collect(source, output)
            self.assertEqual(len(list(output.iterdir())), 7)
            self.assertEqual(len((output / 'SHA256SUMS.txt').read_text().splitlines()), 6)
            self.assertEqual((output / 'Meltype-flash-Android-debug.apk').read_bytes(), b'fixture')

    def test_missing_platform_prevents_staging(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'inputs'
            output = Path(directory) / 'output'
            self.fixture(source)
            (source / 'release-macos-x64/Meltype-flash-macOS-x64.zip').unlink()
            with self.assertRaises(ValueError):
                release.collect(source, output)
            self.assertFalse(output.exists())

    def test_ambiguous_apk_prevents_staging(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / 'inputs'
            self.fixture(source)
            (source / 'meltype-rust-android-debug/app-debug.apk').write_bytes(b'wrong')
            with self.assertRaises(ValueError):
                release.collect(source, Path(directory) / 'output')


if __name__ == '__main__':
    unittest.main()
