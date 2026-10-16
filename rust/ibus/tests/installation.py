#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Install/remove only in temporary XDG roots; never change desktop IBus."""
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET

class Installation(unittest.TestCase):
    binary = Path(sys.argv[1]).resolve()

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="meltype-ibus-install-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.bundle = self.root / "bundle"
        self.bundle.mkdir()
        shutil.copy2(self.binary, self.bundle / "meltype-ibus")
        self.script = self.bundle / "install.sh"
        shutil.copy2(Path(__file__).resolve().parents[1] / "install.sh", self.script)
        for name in ["meltype-settings", "meltype-rs"]:
            (self.bundle / name).write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        self.data = self.root / "日本語 & quoted' path"
        self.target = self.data / "MeltypeRust/ibus"
        self.component = self.data / "ibus/component/meltype-rust.xml"
        self.env = {**os.environ, "XDG_DATA_HOME":str(self.data)}

    def run_script(self, *args, check=True):
        return subprocess.run(["bash", str(self.script), *args], env=self.env, text=True, capture_output=True, check=check)

    def test_install_component_command_round_trips_paths_and_uninstall_preserves_user_data(self):
        self.run_script()
        component = ET.fromstring(self.component.read_text(encoding="utf-8"))
        self.assertEqual(component.findtext("name"), "org.freedesktop.IBus.MeltypeRust")
        self.assertEqual(component.findtext("engines/engine/name"), "meltype-rust")
        command = shlex.split(component.findtext("exec"))
        self.assertEqual(command, [str(self.target / "meltype-ibus"), "--ibus"])
        version = subprocess.check_output([command[0], "--version"], text=True)
        self.assertIn("Meltype Rust IBus", version)
        self.run_script()  # Reinstall from the bundle is idempotent.
        settings = self.data / "MeltypeRust/config.json"
        settings.write_text('{"Enabled":true}', encoding="utf-8")
        other_component = self.component.with_name("meltype.xml")
        other_component.write_text("upstream component", encoding="utf-8")
        unrelated = self.target / "keep.txt"
        unrelated.write_text("keep", encoding="utf-8")
        self.run_script("--uninstall")
        self.assertFalse(self.component.exists())
        self.assertFalse((self.target / "meltype-ibus").exists())
        self.assertEqual(settings.read_text(), '{"Enabled":true}')
        self.assertEqual(other_component.read_text(), "upstream component")
        self.assertEqual(unrelated.read_text(), "keep")

    def test_install_refuses_symlinks_and_remove_refuses_foreign_component(self):
        outside = self.root / "outside"
        outside.mkdir()
        self.target.parent.mkdir(parents=True)
        self.target.symlink_to(outside, target_is_directory=True)
        self.assertNotEqual(self.run_script(check=False).returncode, 0)
        self.assertEqual(list(outside.iterdir()), [])
        self.target.unlink()
        self.run_script()
        self.component.write_text("<component><name>another.application</name></component>", encoding="utf-8")
        self.assertNotEqual(self.run_script("--uninstall", check=False).returncode, 0)
        self.assertTrue(self.component.exists())
        self.assertTrue((self.target / "meltype-ibus").exists())

    def test_system_prefix_registers_and_removes_only_owned_files(self):
        prefix = self.root / "system prefix"
        self.run_script("--system", "--prefix", str(prefix))
        component = prefix / "share/ibus/component/meltype-rust.xml"
        binary = prefix / "libexec/meltype-rust/ibus/meltype-ibus"
        command = shlex.split(ET.parse(component).findtext("exec"))
        self.assertEqual(command, [str(binary), "--ibus"])
        self.assertIn("Meltype Rust IBus", subprocess.check_output([str(binary), "--version"], text=True))
        other = component.with_name("meltype.xml")
        other.write_text("upstream", encoding="utf-8")
        self.run_script("--system", "--prefix", str(prefix), "--uninstall")
        self.assertFalse(component.exists())
        self.assertFalse(binary.exists())
        self.assertEqual(other.read_text(), "upstream")

if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2)
