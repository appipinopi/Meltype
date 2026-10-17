# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Edit notifications: own edits, mode refresh, boundaries and stale input."""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_recovery_smoke import RecoveryHost
from tsf_com_smoke import P, U32, HR, release
from tsf_range_smoke import FAIL, units

TAKE = c.CFUNCTYPE(HR, P)
CURRENT = c.CFUNCTYPE(P, P, c.POINTER(P))
QUERY = c.CFUNCTYPE(None, P, U32, P, HR)
MOVED = c.CFUNCTYPE(None, P, HR)
COMMIT = c.CFUNCTYPE(None, P)


class Callbacks(c.Structure):
    _fields_ = [("owner", P), ("lifetime", P), ("take", TAKE), ("current", CURRENT),
                ("query", QUERY), ("moved", MOVED), ("commit", COMMIT)]


class EventHost(RecoveryHost):
    def __init__(self):
        super().__init__("xxoldyy")
        self.ranges[self.original][:2] = [2, 5]
        self.selection = (3, 3)
        self.record_refs, self.record_reads = 1, 0
        self.changed, self.fail_status, self.fail_compare = True, False, False
        self.on_range = None
        self.fail_range = False
        self.record_table = (P * 5)()
        self.bind(self.record_table, 1, U32, [], lambda _: self.record_ref(1))
        self.bind(self.record_table, 2, U32, [], lambda _: self.record_ref(-1))
        self.bind(self.record_table, 3, HR, [c.POINTER(HR)], self.status)
        self.record = self.Object(self.record_table)

    def record_ref(self, delta):
        self.record_refs += delta
        return self.record_refs

    def status(self, this, out):
        self.record_reads += 1
        out[0] = int(self.changed)
        return FAIL if self.fail_status else 0

    def compare(self, endpoint, this, cookie, other, anchor, out):
        assert cookie == 7 and anchor in [0, 1]
        if self.fail_compare:
            return FAIL
        left, right = self.ranges[this][endpoint], self.ranges[other][anchor]
        out[0] = (left > right) - (left < right)
        return 0

    def get_range(self, *args):
        result = super().get_range(*args)
        if self.on_range:
            self.on_range()
        return FAIL if self.fail_range else result

    def verify(self, test):
        super().verify(test)
        test.assertEqual(self.record_refs, 1)
        test.assertEqual(self.document, units("xxoldyy"))
        test.assertEqual(self.writes, [])


class EventTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_start_composition.argtypes = [P, P, P, U32]
        cls.api.meltype_tsf_start_composition.restype = P
        cls.api.meltype_tsf_edit_notification.argtypes = [P, P, U32, c.POINTER(Callbacks)]

    def setUp(self):
        self.host = EventHost()
        self.own, self.events, self.errors = False, [], []
        self.pointer = self.api.meltype_tsf_start_composition(c.byref(self.host.context),
            c.byref(self.host.sink), self.host.original, 7)
        self.assertTrue(self.pointer)
        self.current = self.pointer
        self.current_context = c.addressof(self.host.context)
        self.on_query = None
        def guarded(function, default=None):
            def callback(*args):
                try:
                    assert args[0] == 123
                    return function(*args)
                except BaseException as error:
                    self.errors.append(repr(error))
                    return default
            return callback
        self.take = TAKE(guarded(self.take_own, 0))
        self.read = CURRENT(guarded(self.read_current))
        self.query = QUERY(guarded(self.query_mode))
        self.moved = MOVED(guarded(lambda _, reset: self.events.append(("moved", reset))))
        self.commit = COMMIT(guarded(lambda _: self.events.append(("commit", True))))
        self.callbacks = Callbacks(123, c.addressof(self.host.context), self.take, self.read, self.query, self.moved, self.commit)

    def tearDown(self):
        if self.pointer:
            release(self.pointer)
        self.host.verify(self)
        self.assertEqual(self.errors, [])

    def take_own(self, _):
        # Owner, context and record are held across every callback.
        self.assertGreaterEqual(self.host.context_refs, 3)
        self.assertEqual(self.host.record_refs, 2)
        value, self.own = self.own, False
        return int(value)

    def read_current(self, _, context):
        context[0] = self.current_context
        return self.current

    def query_mode(self, _, cookie, context, changed):
        assert cookie == 7 and context == c.addressof(self.host.context)
        self.events.append(("query", changed))
        if self.on_query:
            self.on_query()

    def dispatch(self, record=True):
        result = self.api.meltype_tsf_edit_notification(c.byref(self.host.context),
            c.byref(self.host.record) if record else None, 7, c.byref(self.callbacks))
        self.assertEqual(result, 1)

    def test_own_edit_is_consumed_without_querying_record(self):
        self.own = True
        self.dispatch()
        self.assertFalse(self.own)
        self.assertEqual((self.host.record_reads, self.events), (0, []))

    def test_null_record_does_not_consume_own_edit(self):
        self.own = True
        self.dispatch(record=False)
        self.assertTrue(self.own)
        self.assertEqual(self.events, [])

    def test_failed_status_does_not_refresh_or_commit(self):
        self.host.fail_status = True
        self.current = None
        self.dispatch()
        self.assertEqual(self.events, [])

    def test_unchanged_selection_refreshes_mode_only_when_not_composing(self):
        self.host.changed = False
        self.dispatch()
        self.assertEqual(self.events, [])
        self.current = None
        self.dispatch()
        self.assertEqual(self.events, [("query", 0)])

    def test_noncomposing_selection_change_marks_moved_after_mode_query(self):
        self.current = None
        self.dispatch()
        self.assertEqual(self.events[0], ("query", 1))
        self.assertEqual(self.events[1][0], "moved")
        self.assertEqual(len(self.events), 2)

    def test_composition_in_another_context_does_not_commit_or_query_mode(self):
        self.current_context = 1234
        self.dispatch()
        self.assertEqual([event[0] for event in self.events], ["moved"])

    def test_inside_and_boundary_selections_keep_composition(self):
        for selection in [(2, 2), (3, 3), (5, 5), (2, 5)]:
            self.host.selection = selection
            self.dispatch()
            self.assertEqual(self.events, [])

    def test_outside_selection_commits_preserving_application_caret(self):
        for selection in [(1, 1), (6, 6), (1, 4), (4, 6)]:
            self.events.clear()
            self.host.selection = selection
            self.dispatch()
            self.assertEqual(self.events, [("moved", 1), ("commit", True)])
            self.assertEqual(self.host.selection, selection)

    def test_failed_selection_or_comparison_does_not_commit(self):
        self.host.selection = (0, 0)
        self.host.fail_selection = True
        self.dispatch()
        self.host.fail_selection = False
        self.host.fail_range = True
        self.dispatch()
        self.host.fail_range = False
        self.host.fail_compare = True
        self.dispatch()
        self.assertEqual(self.events, [])

    def test_replacement_during_range_query_does_not_commit_current_input(self):
        self.host.selection = (0, 0)
        replacement = self.host.Object(self.host.composition_table)
        self.host.on_range = lambda: setattr(self, "current", c.addressof(replacement))
        self.dispatch()
        self.assertEqual(self.events, [])

    def test_mode_query_can_start_composition_before_selection_handling(self):
        self.current = None
        self.on_query = lambda: setattr(self, "current", self.pointer)
        self.dispatch()
        self.assertEqual(self.events, [("query", 1)])

    def test_termination_during_range_query_does_not_commit_new_input(self):
        self.host.selection = (0, 0)
        def terminate():
            pointer, self.pointer = self.pointer, None
            self.current = None
            release(pointer)
            self.assertGreater(self.host.composition_refs, 0)
        self.host.on_range = terminate
        self.dispatch()
        self.assertEqual(self.events, [])
        self.assertEqual(self.host.composition_refs, 0)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
