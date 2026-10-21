# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Old-input cleanup with real Rust edit-session COM objects and queued hosts."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_recovery_smoke import RecoveryHost
from tsf_com_smoke import P, U32, HR, Guid, UNKNOWN, invoke, release, assert_rust_vtable
from tsf_range_smoke import units
from tsf_edit_session_smoke import SYNCHRONOUS, ASYNC


class DeferredHost(RecoveryHost):
    def __init__(self):
        super().__init__()
        self.pending = []
        self.requests = []
        self.mode = "async"
        self.fail_clear = False
        self.fail_end = False
        self.bind(self.context_table, 3, HR, [U32, P, U32, c.POINTER(HR)], self.request)
        self.bind(self.range_table, 24, HR, [c.POINTER(P)], self.get_context)
        self.bind(self.composition_table, 0, HR, [c.POINTER(Guid), c.POINTER(P)], self.query_composition)
        self.alias = self.Object(self.composition_table)

    def query_composition(self, this, iid, out):
        out[0] = None
        if bytes(iid.contents) != bytes(UNKNOWN):
            return HR(0x80004002).value
        self.composition_ref(1)
        out[0] = c.addressof(self.composition)
        return 0

    def get_context(self, this, out):
        self.context_ref(1)
        out[0] = c.addressof(self.context)
        return 0

    def clear(self, this, cookie, range):
        assert cookie == 7 and range in self.ranges
        self.clears += 1
        return HR(0x80004005).value if self.fail_clear else 0

    def end(self, this, cookie):
        result = super().end(this, cookie)
        return HR(0x80004005).value if self.fail_end else result

    def request(self, this, client, session, flags, out):
        assert_rust_vtable(session)
        self.requests.append((client, flags))
        if self.mode == "reject":
            out[0] = 0
            return SYNCHRONOUS
        if self.mode == "sync":
            out[0] = invoke(session, 3, HR, [U32], 7)
            return 0
        invoke(session, 1, U32, [])
        self.pending.append(session)
        out[0] = ASYNC
        return 0

    def flush(self):
        pending, self.pending = self.pending, []
        for session in pending:
            try:
                assert invoke(session, 3, HR, [U32], 7) == 0
            finally:
                release(session)

    def cancel(self):
        pending, self.pending = self.pending, []
        for session in pending:
            release(session)


class AbandonedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        for name, args, result in [
            ("abandoned_create", [], P), ("abandoned_reset", [P], None),
            ("abandoned_destroy", [P], None),
            ("abandoned_add", [P, P, P, P, U32], HR),
            ("abandoned_forget", [P, P], HR),
            ("abandoned_end", [P, P, U32], HR),
            ("clear_composition_attribute", [P, U32], None),
            ("start_composition", [P, P, P, U32], P),
        ]:
            function = getattr(cls.api, "meltype_tsf_" + name)
            function.argtypes, function.restype = args, result

    def setUp(self):
        self.queue = self.api.meltype_tsf_abandoned_create()
        self.assertTrue(self.queue)
        self.hosts = []

    def tearDown(self):
        for host, composition in self.hosts:
            host.cancel()
        self.api.meltype_tsf_abandoned_destroy(self.queue)
        for host, composition in self.hosts:
            release(composition)
            host.verify(self)
            self.assertEqual(host.document, units("old"))
            self.assertEqual(host.writes, [])

    def host(self):
        host = DeferredHost()
        composition = self.api.meltype_tsf_start_composition(
            c.byref(host.context), c.byref(host.sink), host.original, 7)
        self.assertTrue(composition)
        self.hosts.append((host, composition))
        return host

    def add(self, host):
        return self.api.meltype_tsf_abandoned_add(self.queue, c.byref(host.context),
            c.byref(host.composition), c.byref(host.context), 17)

    def forget(self, host, alias=False):
        return self.api.meltype_tsf_abandoned_forget(
            self.queue, c.byref(host.alias if alias else host.composition))

    def end(self, host):
        return self.api.meltype_tsf_abandoned_end(self.queue, c.byref(host.context), 17)

    def test_async_cleanup_retains_owner_context_and_composition_until_release(self):
        host = self.host()
        self.assertEqual(self.add(host), ASYNC)
        self.assertEqual(host.requests, [(17, 6)])
        self.assertEqual((host.context_refs, host.composition_refs, host.ended), (3, 2, 0))
        host.flush()
        self.assertEqual((host.clears, host.ended), (1, 1))
        self.assertEqual((host.context_refs, host.composition_refs), (1, 1))

    def test_immediate_host_execution_is_reentrant_safe(self):
        host = self.host()
        host.mode = "sync"
        host.on_end = lambda: self.assertEqual(self.forget(host), 0)
        self.assertEqual(self.add(host), 0)
        self.assertEqual((host.clears, host.ended), (1, 1))

    def test_termination_forgets_by_com_identity_and_queued_callback_skips(self):
        host = self.host()
        self.add(host)
        self.assertEqual(self.forget(host, alias=True), 1)
        self.assertEqual(self.forget(host), 0)
        host.flush()
        self.assertEqual((host.clears, host.ended), (0, 0))

    def test_unrelated_termination_does_not_remove_old_input(self):
        host, other = self.host(), self.host()
        self.add(host)
        self.assertEqual(self.forget(other), 0)
        host.flush()
        self.assertEqual((host.ended, other.ended), (1, 0))

    def test_external_termination_then_deactivation_skips_pending_end(self):
        host = self.host()
        self.add(host)
        self.assertEqual(self.forget(host), 1)
        host.mode = "sync"
        self.assertEqual(self.end(host), 0)
        host.flush()
        self.assertEqual((host.clears, host.ended), (0, 0))

    def test_cancelled_async_keeps_only_bounded_list_until_reset(self):
        host = self.host()
        self.add(host)
        host.cancel()
        self.assertEqual((host.context_refs, host.composition_refs), (2, 2))
        self.api.meltype_tsf_abandoned_reset(self.queue)
        self.assertEqual((host.context_refs, host.composition_refs, host.ended), (1, 1, 0))

    def test_rejected_edit_keeps_item_for_synchronous_deactivation(self):
        host = self.host()
        host.mode = "reject"
        self.assertEqual(self.add(host), SYNCHRONOUS)
        self.assertEqual((host.context_refs, host.composition_refs), (2, 2))
        host.mode = "sync"
        self.assertEqual(self.end(host), 0)
        self.assertEqual(host.requests[-1], (17, 7))
        self.assertEqual((host.clears, host.ended), (1, 1))

    def test_refused_deactivation_releases_list_and_deferred_session_can_finish(self):
        host = self.host()
        self.add(host)
        host.mode = "reject"
        self.assertEqual(self.end(host), SYNCHRONOUS)
        self.assertEqual(host.ended, 0)
        host.flush()
        self.assertEqual((host.clears, host.ended, host.context_refs), (1, 1, 1))

    def test_reset_skips_previous_activation_and_allows_new_items(self):
        old, current = self.host(), self.host()
        self.add(old)
        self.api.meltype_tsf_abandoned_reset(self.queue)
        self.add(current)
        old.flush()
        current.flush()
        self.assertEqual((old.ended, current.ended), (0, 1))

    def test_successful_deactivation_then_older_async_request_ends_only_once(self):
        host = self.host()
        self.add(host)
        host.mode = "sync"
        self.assertEqual(self.end(host), 0)
        self.assertEqual((host.clears, host.ended), (1, 1))
        host.flush()
        self.assertEqual((host.clears, host.ended), (1, 1))

    def test_failed_end_can_be_retried_by_older_async_request(self):
        host = self.host()
        self.add(host)
        host.mode = "sync"
        host.fail_end = True
        self.assertEqual(self.end(host), 0)
        self.assertEqual(host.ended, 1)
        host.fail_end = False
        host.flush()
        self.assertEqual((host.clears, host.ended), (2, 2))

    def test_evict_oldest_after_eight_without_editing_it(self):
        hosts = [self.host() for _ in range(9)]
        for host in hosts:
            self.add(host)
        for host in hosts:
            host.flush()
        self.assertEqual([host.ended for host in hosts], [0] + [1] * 8)

    def test_destroy_releases_list_but_pending_can_finish_safely(self):
        host = self.host()
        self.add(host)
        self.api.meltype_tsf_abandoned_destroy(self.queue)
        self.api.meltype_tsf_abandoned_destroy(self.queue)
        host.flush()
        self.assertEqual((host.ended, host.context_refs), (1, 1))

    def test_clear_failure_still_ends_and_retains_across_owner_notification(self):
        host = self.host()
        self.add(host)
        host.fail_clear = True
        host.on_end = lambda: self.assertEqual(self.forget(host), 0)
        host.flush()
        self.assertEqual((host.clears, host.ended), (1, 1))

    def test_wrong_thread_cannot_use_state_or_execute_pending_action(self):
        host = self.host()
        self.add(host)
        received = []
        def worker():
            received.append(self.forget(host))
            received.append(self.end(host))
            self.api.meltype_tsf_abandoned_reset(self.queue)
            self.api.meltype_tsf_abandoned_destroy(self.queue)
            received.append(invoke(host.pending[0], 3, HR, [U32], 7))
        thread = threading.Thread(target=worker)
        thread.start()
        thread.join()
        self.assertEqual(received, [0, HR(0x80070057).value, HR(0x8001010e).value])
        host.flush()
        self.assertEqual(host.ended, 1)


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
