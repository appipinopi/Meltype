# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Host-committed preedit recovery against controlled SDK-layout COM objects."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_composition_smoke import CompositionHost
from tsf_range_smoke import units, FAIL
from tsf_com_smoke import P, U32, HR, release


class RecoveryHost(CompositionHost):
    def __init__(self, text="old"):
        super().__init__()
        self.document = units(text)
        self.ranges[self.original][:2] = [0, len(self.document)]
        self.selection = (len(self.document), len(self.document))
        self.current_range = self.original
        self.fail_compare = False
        self.bind(self.range_table, 18, HR, [U32, P, HR, c.POINTER(HR)],
                  lambda *args: self.compare(0, *args))
        self.bind(self.range_table, 19, HR, [U32, P, HR, c.POINTER(HR)],
                  lambda *args: self.compare(1, *args))

    def compare(self, endpoint, this, cookie, other, anchor, out):
        assert cookie == 7 and anchor == 1
        if self.fail_compare:
            return FAIL
        left = self.ranges[this][endpoint]
        right = self.ranges[other][anchor]
        out[0] = (left > right) - (left < right)
        return 0

    def get_selection(self, this, cookie, index, count, out, fetched):
        assert cookie == 7 and count == 1
        out[0].range = self.new_range(*self.selection)
        fetched[0] = 1
        return FAIL if self.fail_selection else 0

    def start(self, _, cookie, range, sink, out):
        assert cookie == 7 and range in self.ranges and sink == c.addressof(self.sink)
        assert self.composition_refs == 0
        out[0] = None
        if self.fail_start:
            return FAIL
        self.current_range = range
        self.started.append(range)
        self.composition_refs = 1
        self.ref(range, 1)
        self.sink_refs += 1
        out[0] = c.addressof(self.composition)
        return 0

    def composition_ref(self, delta):
        self.composition_refs += delta
        assert self.composition_refs >= 0
        if self.composition_refs == 0:
            self.ref(self.current_range, -1)
            self.sink_refs -= 1
        return self.composition_refs

    def get_range(self, _, out):
        assert self.composition_refs > 0
        self.ref(self.current_range, 1)
        out[0] = self.current_range
        return 0


class RecoveryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        for name, args, result in [
            ("start_composition", [P, P, P, U32], P),
            ("capture_resume", [P, P, U32], P),
            ("resume_matches", [P, P], HR),
            ("try_resume", [P, P, P, U32], P),
            ("resume_attribute", [P, P, U32, U32], None),
            ("destroy_resume", [P], None),
        ]:
            function = getattr(cls.api, "meltype_tsf_" + name)
            function.argtypes = args
            function.restype = result

    def host(self, text="old"):
        host = RecoveryHost(text)
        self.addCleanup(host.verify, self)
        return host

    def capture(self, host):
        composition = self.api.meltype_tsf_start_composition(
            c.byref(host.context), c.byref(host.sink), host.original, 7)
        self.assertTrue(composition)
        try:
            token = self.api.meltype_tsf_capture_resume(c.byref(host.context), composition, 7)
        finally:
            release(composition)
        if token:
            self.addCleanup(self.api.meltype_tsf_destroy_resume, token)
        return token

    def resume(self, token, host, context=None):
        result = self.api.meltype_tsf_try_resume(
            token, context if context is not None else c.byref(host.context),
            c.byref(host.sink), 7)
        if result:
            self.addCleanup(release, result)
            self.api.meltype_tsf_resume_attribute(c.byref(host.context), result, 7, 11)
        return result

    def test_resume_preserves_utf16_document_and_restores_input_attribute(self):
        host = self.host("あ😀n")
        token = self.capture(host)
        self.assertTrue(token)
        self.assertEqual(host.context_refs, 2)
        self.assertEqual(self.api.meltype_tsf_resume_matches(token, c.byref(host.context)), 1)
        self.assertTrue(self.resume(token, host))
        self.assertEqual(host.attributes, [(0, 4, 11)])
        self.assertEqual(host.document, units("あ😀n"))
        self.assertEqual(host.writes, [])
        self.assertEqual(host.selection, (4, 4))
        self.assertEqual(len(host.started), 2)
        self.assertEqual(self.api.meltype_tsf_resume_matches(token, c.byref(host.context)), 0)
        self.assertFalse(self.resume(token, host))

    def test_changed_text_never_reinserts_or_starts_composition(self):
        host = self.host()
        token = self.capture(host)
        host.document = units("new")
        self.assertFalse(self.resume(token, host))
        self.assertEqual(host.document, units("new"))
        self.assertEqual((host.writes, host.attributes, len(host.started)), ([], [], 1))
        self.assertEqual(host.context_refs, 1)

    def test_moved_caret_and_nonempty_selection_consume_snapshot(self):
        for selection in [(0, 0), (2, 2), (4, 4), (0, 3), (3, 4)]:
            with self.subTest(selection=selection):
                host = self.host()
                token = self.capture(host)
                host.selection = selection
                self.assertFalse(self.resume(token, host))
                self.assertEqual((host.writes, host.attributes, len(host.started)), ([], [], 1))
                self.assertEqual(host.context_refs, 1)

    def test_another_input_field_consumes_without_querying_it(self):
        host = self.host()
        other = self.host("elsewhere")
        token = self.capture(host)
        self.assertEqual(self.api.meltype_tsf_resume_matches(token, c.byref(other.context)), 0)
        self.assertFalse(self.resume(token, host, c.byref(other.context)))
        self.assertEqual((host.context_refs, other.context_refs), (1, 1))
        self.assertEqual(other.started, [])
        self.assertFalse(self.resume(token, host))

    def test_capture_uses_original_nonempty_less_than_64_utf16_limit(self):
        for text, accepted in [("", False), ("あ" * 63, True), ("あ" * 64, False),
                               ("😀" * 31 + "n", True), ("😀" * 32, False)]:
            with self.subTest(length=len(units(text))):
                host = self.host(text)
                token = self.capture(host)
                self.assertEqual(bool(token), accepted)
                if token:
                    self.assertTrue(self.resume(token, host))
                self.assertEqual(host.writes, [])

    def test_capture_read_failure_drops_all_acquired_references(self):
        host = self.host()
        host.fail_read = True
        self.assertFalse(self.capture(host))
        self.assertEqual(host.context_refs, 1)

    def test_resume_host_errors_consume_without_writing(self):
        for failure in ["fail_read", "fail_selection", "fail_compare", "fail_start"]:
            with self.subTest(failure=failure):
                host = self.host()
                token = self.capture(host)
                setattr(host, failure, True)
                self.assertFalse(self.resume(token, host))
                self.assertEqual((host.writes, host.attributes, host.context_refs), ([], [], 1))

    def test_destroy_unused_and_stale_snapshot_is_idempotent(self):
        host = self.host()
        token = self.capture(host)
        self.api.meltype_tsf_destroy_resume(token)
        self.api.meltype_tsf_destroy_resume(token)
        self.assertEqual(host.context_refs, 1)
        newer = self.capture(host)
        self.assertNotEqual(token, newer)
        self.assertFalse(self.resume(token, host))
        self.assertTrue(self.resume(newer, host))

    def test_foreign_thread_cannot_use_or_release_sta_snapshot(self):
        host = self.host()
        token = self.capture(host)
        received = []
        def worker():
            received.append(self.api.meltype_tsf_resume_matches(token, c.byref(host.context)))
            received.append(self.api.meltype_tsf_try_resume(
                token, c.byref(host.context), c.byref(host.sink), 7))
            self.api.meltype_tsf_destroy_resume(token)
        thread = threading.Thread(target=worker)
        thread.start()
        thread.join()
        self.assertEqual(received, [0, None])
        self.assertEqual(host.context_refs, 2)
        self.assertTrue(self.resume(token, host))

    def test_styling_retains_composition_when_owner_releases_during_notification(self):
        host = self.host()
        token = self.capture(host)
        result = self.api.meltype_tsf_try_resume(token, c.byref(host.context), c.byref(host.sink), 7)
        self.assertTrue(result)
        original = host.set_value
        def terminated(*args):
            release(result)
            self.assertGreater(host.composition_refs, 0)
            return original(*args)
        host.bind(host.property_table, 9, HR, [U32, P, P], terminated)
        self.api.meltype_tsf_resume_attribute(c.byref(host.context), result, 7, 11)
        self.assertEqual(host.composition_refs, 0)
        self.assertEqual(host.attributes, [(0, 3, 11)])


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
