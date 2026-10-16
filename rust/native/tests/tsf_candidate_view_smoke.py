# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Candidate reply projection plus selected-clause geometry through real COM ABI."""
import ctypes as c
import json
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_recovery_smoke import RecoveryHost
from tsf_range_smoke import units, FAIL
from tsf_com_smoke import P, U32, HR, release
from tsf_candidate_smoke import Backend, Rect, Point, I, user, pump, bounds, capture


class GeometryHost(RecoveryHost):
    def __init__(self):
        super().__init__("前😀文節後")
        self.extents = []
        self.view_refs = 1
        self.fail_view = self.fail_extent = False
        self.zero_rect = self.negative_rect = False
        self.on_extent = None
        self.view_table = (P * 7)()
        self.bind(self.context_table, 9, HR, [c.POINTER(P)], self.get_view)
        self.bind(self.view_table, 1, U32, [], lambda _: self.view_ref(1))
        self.bind(self.view_table, 2, U32, [], lambda _: self.view_ref(-1))
        self.bind(self.view_table, 4, HR, [U32, P, c.POINTER(Rect), c.POINTER(I)], self.extent)
        self.view = self.Object(self.view_table)

    def view_ref(self, delta):
        self.view_refs += delta
        return self.view_refs

    def get_view(self, this, out):
        out[0] = None
        if self.fail_view:
            return FAIL
        self.view_ref(1)
        out[0] = c.addressof(self.view)
        return 0

    def extent(self, this, cookie, range, out, clipped):
        assert cookie == 7
        start, end, _ = self.ranges[range]
        self.extents.append((start, end))
        if self.on_extent:
            self.on_extent()
        clipped[0] = 1  # Clipped but nonzero geometry is valid in upstream.
        out[0] = Rect(100 + start * 10, 120, 100 + end * 10, 140)
        if self.zero_rect:
            out[0] = Rect()
        if self.negative_rect:
            out[0] = Rect(-300, -200, -280, -180)
        return FAIL if self.fail_extent else 0

    def verify(self, test):
        super().verify(test)
        test.assertEqual(self.view_refs, 1)
        test.assertEqual(self.document, units("前😀文節後"))
        test.assertEqual(self.writes, [])
        test.assertEqual(self.ranges[self.original][:2], [0, 6])


class GeometryTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_start_composition.argtypes = [P, P, P, U32]
        cls.api.meltype_tsf_start_composition.restype = P
        cls.api.meltype_tsf_candidate_anchor.argtypes = [P, P, U32, c.c_char_p, c.POINTER(Rect)]
        cls.api.meltype_tsf_candidate_anchor.restype = I

    def setUp(self):
        self.host = GeometryHost()
        self.pointer = self.api.meltype_tsf_start_composition(c.byref(self.host.context),
            c.byref(self.host.sink), self.host.original, 7)
        self.assertTrue(self.pointer)

    def tearDown(self):
        if self.pointer:
            release(self.pointer)
        self.host.verify(self)

    def anchor(self, **view):
        rect = Rect(9, 9, 9, 9)
        result = self.api.meltype_tsf_candidate_anchor(c.byref(self.host.context), self.pointer, 7,
            json.dumps(view, ensure_ascii=False).encode(), c.byref(rect))
        return result, (rect.left, rect.top, rect.right, rect.bottom)

    def test_selected_clause_uses_utf16_offset_without_mutating_original_range(self):
        result, rect = self.anchor(converting=True, clauses=["前", "😀", "文節", "後"], selectedClause=2)
        self.assertEqual(result, 1)
        self.assertEqual(self.host.extents, [(3, 5)])
        self.assertEqual(rect, (130, 120, 150, 140))

    def test_nonconverting_or_invalid_selected_clause_anchors_whole_preedit(self):
        for converting, selected in [(False, 1), (True, -1), (True, 9)]:
            with self.subTest(converting=converting, selected=selected):
                result, _ = self.anchor(converting=converting, clauses=["前", "😀文節後"], selectedClause=selected)
                self.assertEqual(result, 1)
                self.assertEqual(self.host.extents[-1], (0, 6))

    def test_partial_clause_shift_falls_back_instead_of_using_wrong_geometry(self):
        result, rect = self.anchor(converting=True, clauses=["前", "a" * 20], selectedClause=1)
        self.assertEqual(result, 0)
        self.assertEqual(self.host.extents, [])
        self.assertEqual(rect[2] - rect[0], 1)

    def test_failed_view_extent_or_zero_rect_uses_fallback(self):
        for flag in ["fail_view", "fail_extent", "zero_rect"]:
            with self.subTest(flag=flag):
                setattr(self.host, flag, True)
                result, rect = self.anchor()
                self.assertEqual(result, 0)
                self.assertEqual((rect[2] - rect[0], rect[3] - rect[1]), (1, 16))
                setattr(self.host, flag, False)

    def test_negative_monitor_coordinates_and_clipped_rect_are_preserved(self):
        self.host.negative_rect = True
        self.assertEqual(self.anchor(), (1, (-300, -200, -280, -180)))

    def test_host_release_during_geometry_query_keeps_composition_alive(self):
        def terminate():
            pointer, self.pointer = self.pointer, None
            release(pointer)
            self.assertGreater(self.host.composition_refs, 0)
        self.host.on_extent = terminate
        self.assertEqual(self.anchor()[0], 1)
        self.assertEqual(self.host.composition_refs, 0)

    def test_invalid_json_does_not_change_output(self):
        rect = Rect(9, 8, 7, 6)
        result = self.api.meltype_tsf_candidate_anchor(c.byref(self.host.context), self.pointer, 7, b'{', c.byref(rect))
        self.assertEqual(result, -1)
        self.assertEqual((rect.left, rect.top, rect.right, rect.bottom), (9, 8, 7, 6))

    def test_fallback_prefers_test_thread_caret_and_converts_client_coordinates(self):
        user.CreateWindowExW.argtypes = [U32, c.c_wchar_p, c.c_wchar_p, U32, I, I, I, I, P, P, P, P]
        user.CreateWindowExW.restype = P
        user.DestroyWindow.argtypes = [P]
        user.CreateCaret.argtypes = [P, P, I, I]
        user.ClientToScreen.argtypes = [P, c.POINTER(Point)]
        window = user.CreateWindowExW(0, "STATIC", "Meltype private caret test", 0x80000000,
            100, 200, 200, 100, None, None, None, None)
        self.assertTrue(window)
        try:
            self.assertTrue(user.CreateCaret(window, None, 1, 19))
            try:
                self.assertTrue(user.SetCaretPos(8, 9))
                point = Point(8, 9)
                self.assertTrue(user.ClientToScreen(window, c.byref(point)))
                self.host.fail_view = True
                self.assertEqual(self.anchor(), (0, (point.x, point.y, point.x + 1, point.y + 19)))
            finally:
                user.DestroyCaret()
        finally:
            user.DestroyWindow(window)


class ProjectionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        ole = c.WinDLL("ole32")
        assert ole.CoInitializeEx(None, 2) in [0, 1]
        cls.addClassCleanup(ole.CoUninitialize)

    def setUp(self):
        self.actual = Backend(sys.argv[1])
        self.expected = Backend(sys.argv[2], reference=True) if len(sys.argv) > 2 else Backend(sys.argv[1])
        self.host = GeometryHost()
        api = self.actual.api
        api.meltype_tsf_start_composition.argtypes = [P, P, P, U32]
        api.meltype_tsf_start_composition.restype = P
        api.meltype_tsf_candidate_show_tsf.argtypes = [P, P, P, U32, c.c_char_p]
        api.meltype_tsf_candidate_show_tsf.restype = I
        self.composition = api.meltype_tsf_start_composition(c.byref(self.host.context), c.byref(self.host.sink), self.host.original, 7)
        self.assertTrue(self.composition)

    def tearDown(self):
        self.actual.close()
        self.expected.close()
        release(self.composition)
        self.host.verify(self)

    def show(self, view, handle=None):
        result = self.actual.api.meltype_tsf_candidate_show_tsf(self.actual.handle if handle is None else handle,
            c.byref(self.host.context), self.composition, 7, json.dumps(view, ensure_ascii=False).encode())
        pump()
        return result

    def compare(self, reply, expected):
        self.assertEqual(self.show(reply), 1)
        # Capture each renderer separately, with the other hidden, so overlap
        # cannot let a broken renderer capture the reference's pixels.
        from tsf_candidate_smoke import windows
        actual_window = next(w for w in windows(self.actual.name) if user.IsWindowVisible(w))
        user.SendMessageW(actual_window, 0x2a3, 0, 0)
        actual_geometry, actual_image = bounds(actual_window), capture(actual_window)
        self.actual.api.meltype_tsf_candidate_hide(self.actual.handle)
        expected_window = self.expected.show(expected, Rect(100, 120, 160, 140))
        user.SendMessageW(expected_window, 0x2a3, 0, 0)
        self.assertEqual(actual_geometry, bounds(expected_window))
        self.assertEqual(actual_image, capture(expected_window))
        self.expected.api.meltype_tsf_candidate_hide(self.expected.handle)
        self.assertEqual(self.show(reply), 1)
        return actual_window

    def test_converted_candidates_notes_and_typed_suppression(self):
        self.compare({"converting": True, "candidates": ["候補A", "候補B"], "notes": ["a", "b"],
            "selectedIndex": 1, "text": "候補B", "typed": "候補B"},
            {"candidates": ["候補A", "候補B"], "notes": ["a", "b"], "selected": 1})

    def test_prediction_replaces_candidates_and_clears_conversion_metadata(self):
        window = self.compare({"candidates": ["不要"], "notes": ["unused"], "meaning": "unused",
            "predictions": ["予測A", "予測B"], "selectedPrediction": 1},
            {"candidates": ["予測A", "予測B"], "selected": 1, "prediction": True})
        scale = user.GetDpiForWindow(window) / 96
        x, y = int(30 * scale), int(20 * scale)
        user.SendMessageW(window, 0x0202, 0, x | y << 16)
        self.assertEqual(self.actual.events, [(0, 1)])

    def test_single_candidate_typed_and_suggestion_only_views(self):
        for reply, expected in [
            ({"converting": True, "candidates": ["候補"], "meaning": "unused", "typed": "kouho", "text": "候補"},
             {"candidates": [], "selected": 0, "typed": "kouho"}),
            ({"suggestion": "補助", "candidates": ["不要"], "notes": ["unused"], "meaning": "unused"},
             {"candidates": [], "selected": -1, "suggestion": "補助"}),
        ]:
            with self.subTest(reply=reply):
                self.compare(reply, expected)

    def test_irrelevant_view_hides_without_querying_host(self):
        self.assertEqual(self.show({"predictions": ["候補"]}), 1)
        self.assertTrue(self.actual.api.meltype_tsf_candidate_visible(self.actual.handle))
        self.host.extents.clear()
        self.assertEqual(self.show({"converting": True, "candidates": ["候補"], "typed": "候補", "text": "候補"}), 1)
        self.assertFalse(self.actual.api.meltype_tsf_candidate_visible(self.actual.handle))
        self.assertEqual(self.host.extents, [])

    def test_stale_token_does_not_query_host(self):
        self.actual.close()
        self.assertEqual(self.show({"predictions": ["候補"]}, 123456789), 0)
        self.assertEqual(self.host.extents, [])

    def test_reentrant_candidate_destroy_during_geometry_is_safe(self):
        self.host.on_extent = self.actual.close
        self.assertEqual(self.show({"predictions": ["候補"]}), 0)
        self.assertEqual(self.host.extents, [(0, 6)])

    def test_foreign_thread_does_not_query_sta_context(self):
        received = []
        def worker():
            received.append(self.actual.api.meltype_tsf_candidate_show_tsf(self.actual.handle,
                c.byref(self.host.context), self.composition, 7, b'{"predictions":["test"]}'))
        thread = threading.Thread(target=worker)
        thread.start()
        thread.join()
        self.assertEqual(received, [0])
        self.assertEqual(self.host.extents, [])


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
