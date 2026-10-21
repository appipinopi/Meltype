#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""A real isolated IBus daemon/client exercises the Rust engine executable.

Run with: xvfb-run -a dbus-run-session -- python3 rust/ibus/tests/integration.py PATH_TO_ENGINE
No desktop input-source registration or user IBus daemon is modified.
"""
import json
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import time
import unittest
import gi

gi.require_version("IBus", "1.0")
from gi.repository import GLib, IBus

def pump(seconds=0.05):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        while GLib.MainContext.default().pending():
            GLib.MainContext.default().iteration(False)
        time.sleep(0.002)

def wait(predicate, description, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        pump()
    raise AssertionError("timed out: " + description)

class EngineTests(unittest.TestCase):
    executable = Path(sys.argv[1]).resolve()

    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory(prefix="meltype-flash-ibus-")
        cls.addClassCleanup(cls.directory.cleanup)
        cls.root = Path(cls.directory.name)
        cls.installed = os.environ.get("MELTYPE_IBUS_TEST_INSTALLED") == "1"
        for variable, leaf in [("XDG_DATA_HOME", "xdg-data"), ("XDG_CACHE_HOME", "cache"), ("XDG_CONFIG_HOME", "config"), ("XDG_RUNTIME_DIR", "runtime")]:
            directory = cls.root / leaf
            directory.mkdir(mode=0o700)
            os.environ[variable] = str(directory)
        os.environ["IBUS_ADDRESS"] = "unix:path=" + str(cls.root / "ibus-bus")
        os.environ["MELTYPE_RUST_DATA_DIR"] = str(cls.root / "data")
        components = cls.root / "xdg-data/ibus/component"
        if cls.installed:
            bundle = cls.root / "bundle"
            bundle.mkdir()
            shutil.copy2(cls.executable, bundle / "meltype-ibus")
            shutil.copy2(Path(__file__).resolve().parents[3] / "linux/icon.png", bundle / "icon.png")
            for name in ["meltype-flash", "meltype-rs"]:
                (bundle / name).write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            script = Path(__file__).resolve().parents[1] / "install.sh"
            shutil.copy2(script, bundle / "install.sh")
            prefix = cls.root / "日本語 & installed' path"
            subprocess.run(["bash", str(bundle / "install.sh"), "--system", "--prefix", str(prefix)], check=True, capture_output=True)
            components = prefix / "share/ibus/component"
        components.mkdir(parents=True, exist_ok=True)
        os.environ["IBUS_COMPONENT_PATH"] = str(components)
        (cls.root / "data").mkdir()
        (cls.root / "data/config.json").write_text(json.dumps({"LiveConversion":False, "PredictiveCandidates":False, "SigilWordsDirect":True}), encoding="utf-8")
        (cls.root / "data/state.json").write_text(json.dumps({"preferences":{"user_dictionary":{"めるたいぷ":[f"候補{i}" for i in range(18)]}}}), encoding="utf-8")
        cls.daemon_log = (cls.root / "daemon.log").open("w+")
        cls.daemon = subprocess.Popen(["ibus-daemon", "--single", "--panel", "disable", "--config", "disable", "--emoji-extension", "disable", "--address", os.environ["IBUS_ADDRESS"], "--cache=none", "--timeout=3000"], stdout=cls.daemon_log, stderr=subprocess.STDOUT)
        cls.addClassCleanup(cls.stop, cls.daemon)
        IBus.init()
        wait(lambda: (cls.root / "ibus-bus").exists(), "isolated IBus socket")
        cls.bus = IBus.Bus.new()
        wait(cls.bus.is_connected, "IBus connection")
        cls.panel = IBus.PanelService.new(cls.bus.get_connection())
        cls.bus.request_name("org.freedesktop.IBus.Panel", 4)
        cls.addClassCleanup(cls.panel.destroy)
        cls.engine_log = (cls.root / "engine.log").open("w+")
        if cls.installed:
            wait(lambda: any(e.get_name() == "meltype-flash" for e in cls.bus.list_engines()), "Rust component discovery")
        else:
            cls.engine = subprocess.Popen([str(cls.executable)], stdout=cls.engine_log, stderr=subprocess.STDOUT, env={**os.environ, "G_DEBUG":"fatal-warnings"})
            cls.addClassCleanup(cls.stop, cls.engine)
            wait(lambda: any(e.get_name() == "meltype-flash" for e in cls.bus.list_active_engines()), "Rust factory registration")

    @staticmethod
    def stop(process):
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)

    def setUp(self):
        self.document = ""
        self.marked = ""
        self.candidates = []
        self.selected_candidate = 0
        self.deletions = []
        self.auxiliary = ""
        self.context_requests = 0
        self.selection = None
        self.context = self.bus.create_input_context("Meltype-flash integration")
        self.addCleanup(self.context.destroy)
        self.context.connect("commit-text", self.commit)
        self.context.connect("update-preedit-text", self.preedit)
        self.context.connect("update-lookup-table", self.lookup)
        self.context.connect("hide-lookup-table", lambda *_: self.candidates.clear())
        self.context.connect("update-auxiliary-text", lambda _ic, text, visible: setattr(self, "auxiliary", text.get_text() if visible else ""))
        self.context.connect("delete-surrounding-text", self.delete)
        self.context.connect("require-surrounding-text", self.require_surrounding)
        self.context.set_capabilities(int(IBus.Capabilite.PREEDIT_TEXT | IBus.Capabilite.AUXILIARY_TEXT | IBus.Capabilite.LOOKUP_TABLE | IBus.Capabilite.FOCUS | IBus.Capabilite.PROPERTY | IBus.Capabilite.SURROUNDING_TEXT))
        self.context.focus_in()
        pump()
        def select():
            self.bus.set_global_engine("meltype-flash")
            self.context.set_engine("meltype-flash")
            pump()
            description = self.context.get_engine()
            return description is not None and description.get_name() == "meltype-flash"
        wait(select, "selected test engine")
        self.surrounding()
        pump()

    def surrounding(self):
        cursor, anchor = self.selection or (len(self.document), len(self.document))
        self.context.set_surrounding_text(IBus.Text.new_from_string(self.document), cursor, anchor)

    def require_surrounding(self, *_):
        self.context_requests += 1
        self.surrounding()

    def commit(self, _ic, text):
        self.document += text.get_text()
        self.surrounding()

    def preedit(self, _ic, text, cursor, visible):
        self.marked = text.get_text() if visible else ""

    def lookup(self, _ic, table, visible):
        self.candidates = [table.get_candidate(i).get_text() for i in range(table.get_number_of_candidates())] if visible else []
        self.selected_candidate = table.get_cursor_pos()

    def delete(self, _ic, offset, count):
        self.deletions.append((offset, count))
        at = len(self.document) + offset
        self.document = self.document[:at] + self.document[at + count:]
        self.surrounding()

    def key(self, keyval, scalar=None, state=0):
        self.surrounding()
        consumed = self.context.process_key_event(keyval, 0, state)
        pump(0.015)
        if not consumed and scalar is not None:
            self.document += scalar
            self.surrounding()
        return consumed

    def type(self, raw):
        return [self.key(IBus.unicode_to_keyval(c), c) for c in raw]

    def test_japanese_conversion_candidates_and_commit(self):
        self.assertTrue(all(self.type("hashi")))
        self.assertEqual(self.marked, "はし")
        self.assertTrue(self.key(IBus.KEY_space, " "))
        self.assertIn("橋", self.candidates)
        self.assertIn("箸", self.candidates)
        self.assertTrue(self.key(IBus.KEY_Return))
        self.assertIn(self.document, ["橋", "箸", "端", "はし"])
        self.assertEqual(self.marked, "")

    def test_completion_keys_pass_and_space_returns_to_japanese(self):
        self.assertFalse(any(self.type("@src/app.ts")))
        self.assertEqual(self.document, "@src/app.ts")
        self.assertEqual(self.marked, "")
        # Space reaches the application without ProcessKeyEvent.
        self.document += " "
        self.surrounding()
        self.assertTrue(all(self.type("kyou")))
        self.assertEqual(self.marked, "きょう")

    def test_candidate_panel_click_selects_the_requested_candidate(self):
        self.type("hashi")
        self.key(IBus.KEY_space, " ")
        index = next(i for i, value in enumerate(self.candidates) if value.startswith("箸"))
        self.panel.candidate_clicked(index, 1, 0)
        wait(lambda: self.document == "箸", "panel candidate commit")
        self.assertEqual(self.document, "箸")
        self.assertEqual(self.marked, "")

    def test_candidate_panel_paging_click_uses_the_current_page(self):
        self.type("merutaipu")
        self.key(IBus.KEY_space, " ")
        self.assertGreaterEqual(len(self.candidates), 18)
        self.assertEqual(self.selected_candidate, 0)
        self.panel.page_down()
        wait(lambda: self.selected_candidate == 9, "panel page down")
        self.assertEqual(self.selected_candidate, 9)
        self.panel.page_up()
        wait(lambda: self.selected_candidate == 0, "panel page up")
        self.assertEqual(self.selected_candidate, 0)
        self.panel.cursor_down()
        wait(lambda: self.selected_candidate == 1, "panel cursor down")
        self.assertEqual(self.selected_candidate, 1)
        self.panel.cursor_up()
        wait(lambda: self.selected_candidate == 0, "panel cursor up")
        self.assertEqual(self.selected_candidate, 0)
        self.panel.page_down()
        wait(lambda: self.selected_candidate == 9, "panel second page down")
        expected = self.candidates[10]
        self.panel.candidate_clicked(1, 1, 0)
        wait(lambda: self.document == expected, "second-page candidate commit")
        self.assertEqual(self.document, expected)
        self.assertEqual(self.marked, "")

    def test_direct_mode_jis_keys_and_unicode(self):
        self.assertTrue(self.key(IBus.KEY_Muhenkan))
        self.assertFalse(any(self.type("hello")))
        self.assertTrue(self.key(IBus.KEY_Henkan))
        self.type("ka")
        self.assertEqual(self.marked, "か")
        self.key(IBus.KEY_Escape)
        self.type("İ😀")
        self.key(IBus.KEY_Return)
        self.assertTrue(self.document.endswith("İ😀"))

    def test_focus_out_commits_once_and_release_does_not_type(self):
        self.assertFalse(self.key(IBus.KEY_a, state=int(IBus.ModifierType.RELEASE_MASK)))
        self.assertEqual(self.marked, "")
        self.type("kyou")
        self.context.focus_out()
        pump(0.1)
        self.assertEqual(self.document, "きょう")
        self.assertEqual(self.marked, "")

    def test_focus_out_preserves_pending_preedit_and_enter_finalizes_it(self):
        self.type("nihon")
        self.assertEqual(self.marked, "にほn")
        self.context.focus_out()
        pump()
        # IBus commits its existing preedit before notifying the engine.
        self.assertEqual(self.document, "にほn")
        self.context.focus_in()
        pump()
        self.type("nihon")
        self.key(IBus.KEY_Return)
        self.assertEqual(self.document, "にほnにほん")

    def test_password_never_requests_context_or_creates_preedit(self):
        self.context.set_content_type(int(IBus.InputPurpose.PASSWORD), 0)
        pump()
        requests = self.context_requests
        self.assertFalse(any(self.type("secret")))
        self.assertEqual(self.marked, "")
        self.assertEqual(self.context_requests, requests)
        self.assertEqual(self.deletions, [])
        self.context.set_content_type(int(IBus.InputPurpose.FREE_FORM), 0)
        pump()
        self.type("ka")
        self.assertEqual(self.marked, "か")

    def test_direct_detection_replaces_only_verified_prefix(self):
        self.key(IBus.KEY_Muhenkan)
        self.assertFalse(any(self.type("kon")))
        self.assertEqual(self.document, "kon")
        self.assertTrue(self.key(IBus.KEY_n, "n"))
        self.assertEqual(self.document, "")
        self.assertEqual(self.marked, "こん")
        self.assertEqual(self.deletions, [(-3, 3)])
        self.key(IBus.KEY_Return)
        self.assertEqual(self.document, "こん")

    def test_nonempty_selection_disables_backward_replacement(self):
        self.key(IBus.KEY_Muhenkan)
        self.assertFalse(any(self.type("kon")))
        self.selection = (3, 0)
        self.key(IBus.KEY_n)
        self.assertEqual(self.deletions, [])
        self.assertEqual(self.document, "kon")
        self.assertEqual(self.marked, "")

    @classmethod
    def tearDownClass(cls):
        for name, file in [("engine", cls.engine_log), ("daemon", cls.daemon_log)]:
            file.flush()
            file.seek(0)
            logs = file.read()
            if logs.strip():
                print(name + " log:\n" + logs, file=sys.stderr)
            file.close()

if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
