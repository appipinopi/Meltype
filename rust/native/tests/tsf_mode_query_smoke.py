# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Privacy-gated mode queries and response publication/notification ordering."""
import ctypes as c
import json
from pathlib import Path
import sys
import time
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_mode_smoke import ModeTests, READ, SECRET, P, U32, HR, DISABLED
from tsf_key_entry_smoke import Entry, WOULD, MARK, COMPOSING, TOGGLE, HANDLE

QUERY = c.CFUNCTYPE(HR, P, c.c_char_p, U32, HR)
WRITE = c.CFUNCTYPE(None, P, U32)
NOTIFY = c.CFUNCTYPE(None, P, HR)
class QueryCallbacks(c.Structure):
    _fields_ = [("owner", P), ("lifetime", P), ("read", READ), ("secret", SECRET), ("request", QUERY)]
class ApplyCallbacks(c.Structure):
    _fields_ = [("owner", P), ("lifetime", P), ("read", READ), ("write", WRITE), ("notify", NOTIFY)]

class QueryTests(ModeTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_query_mode.argtypes = [P, U32, P, c.c_char_p, c.POINTER(QueryCallbacks), HR]
        cls.api.meltype_tsf_apply_mode.argtypes = [U32, c.POINTER(ApplyCallbacks)]

    def setUp(self):
        super().setUp()
        self.flags = 10
        self.policies, self.writes, self.notifications = [], [], []
        self.on_write = self.on_notify = None
        self.query_callbacks = [READ(self.read), SECRET(self.private), QUERY(self.query_request)]
        self.query_bridge = QueryCallbacks(123, c.addressof(self.host.context), *self.query_callbacks)
        self.apply_callbacks = [READ(self.read), WRITE(self.write), NOTIFY(self.notify)]
        self.apply_bridge = ApplyCallbacks(123, c.addressof(self.host.context), *self.apply_callbacks)

    def private(self, owner, cookie, context):
        try:
            self.assertEqual(cookie, 7)
            self.assertEqual(context, c.addressof(self.host.context))
            self.assertGreaterEqual(self.host.context_refs, 3)
            self.events.append(("secret",))
            return int(self.secret)
        except BaseException as error:
            self.errors.append(repr(error))
            return 1

    def query_request(self, owner, text, timeout, optional):
        self.policies.append((timeout, optional))
        return self.request(owner, text)

    def query(self, changed=False, context=True):
        return self.api.meltype_tsf_query_mode(c.byref(self.host.context) if context else None,
            7, self.token, self.metadata, c.byref(self.query_bridge), int(changed))

    def write(self, owner, flags):
        self.writes.append(flags)
        self.flags = (self.flags & ~6) | flags
        if self.on_write:
            self.on_write()

    def notify(self, owner, english):
        try:
            self.assertGreaterEqual(self.host.context_refs, 1)
            self.assertEqual(bool(self.flags & 4), bool(english))
            self.assertEqual(self.writes[-1], self.flags & 6)
            self.notifications.append(english)
            if self.on_notify:
                self.on_notify()
        except BaseException as error:
            self.errors.append(repr(error))

    def apply(self, flags):
        return self.api.meltype_tsf_apply_mode(flags, c.byref(self.apply_bridge))

    def remember(self, key=65):
        callbacks = [WOULD(lambda *args: 0), MARK(lambda *args: None), COMPOSING(lambda *args: 0),
                     TOGGLE(lambda *args: None), HANDLE(lambda *args: None)]
        entry = Entry(123, c.addressof(self.host.context), *callbacks)
        self.assertEqual(self.api.meltype_tsf_key_event(self.token, 1, c.byref(self.host.context), key, 0, c.byref(entry)), 0)

    def test_query_uses_current_cookie_and_optional_short_timeout(self):
        self.assertEqual(self.query(), 1)
        expected = {"op":"mode", **json.loads(self.metadata), "before":'print("日本語😀")'}
        self.assertEqual({k:v for k,v in self.requests[0].items() if k != "passed"}, expected)
        self.assertEqual(self.policies, [(100, 1)])
        self.assertEqual(self.host.locks, [])
        self.assertNotIn(("ready",), self.events)

    def test_query_non_code_and_inactive_server_do_not_inspect_field(self):
        for flags in [0, 2, 8]:
            self.flags = flags
            self.assertEqual(self.query(), 0)
        self.assertEqual(self.events, [])
        self.assertEqual(self.host.read_count, 0)

    def test_query_disabled_and_secret_fields_do_not_read_or_send(self):
        self.host.integers[bytes(DISABLED)] = 1
        self.assertEqual(self.query(), 0)
        self.assertEqual(self.events, [])
        self.host.integers[bytes(DISABLED)] = 0
        self.secret = True
        self.assertEqual(self.query(), 0)
        self.assertEqual(self.events, [("secret",)])
        self.assertEqual(self.host.read_count, 0)
        self.assertEqual(self.requests, [])

    def test_query_preserves_pending_key_when_privacy_blocks(self):
        self.remember()
        self.secret = True
        self.query()
        self.secret = False
        self.assertEqual(self.query(), 1)
        self.assertEqual(self.requests[-1]["passed"], 65)

    def test_query_recent_key_is_retained_for_following_edit_notifications(self):
        self.remember()
        self.assertEqual(self.query(), 1)
        self.assertEqual(self.query(), 1)
        self.assertEqual([r["passed"] for r in self.requests], [65, 65])
        # Query consumes the pending report even while retaining recent metadata.
        self.host.lock_mode = "reject"
        self.line()
        self.assertNotIn("passed", self.requests[-1])

    def test_query_expired_key_distinguishes_selection_from_text_edit(self):
        self.remember()
        time.sleep(0.52)
        self.query(changed=False)
        self.query(changed=True)
        self.assertEqual([r["passed"] for r in self.requests], [-1, 0])
        self.assertTrue(all("ctrl" not in r for r in self.requests))

    def test_query_missing_selection_sends_empty_context(self):
        self.host.fail_selection = True
        self.assertEqual(self.query(), 1)
        self.assertEqual(self.requests[0]["before"], "")

    def test_query_failed_request_returns_without_notification(self):
        self.done = False
        self.assertEqual(self.query(), 0)
        self.assertEqual(self.notifications, [])
        self.assertEqual(self.policies, [(100, 1)])

    def test_query_null_context_and_invalid_metadata_do_not_send(self):
        self.assertEqual(self.query(context=False), 0)
        self.metadata = b'{"sid":false}'
        self.assertEqual(self.query(), 0)
        self.assertEqual(self.host.read_count, 0)
        self.assertEqual(self.requests, [])

    def test_query_request_can_release_original_owner_reference(self):
        self.on_request = lambda: self.host.context_ref(-1)
        self.assertEqual(self.query(), 1)
        self.assertEqual(self.host.context_refs, 0)
        self.host.context_ref(1)

    def test_apply_inactive_code_english_truth_table(self):
        expected = [0, 0, 0, 2, 0, 0, 0, 6]
        for previous in [0, 2, 4, 6]:
            for reply, projected in enumerate(expected):
                self.flags = previous
                self.writes.clear(); self.notifications.clear()
                self.assertEqual(self.apply(reply), 1)
                self.assertEqual(self.writes, [projected])
                self.assertEqual(self.flags, projected)
                self.assertEqual(self.notifications, [int(bool(projected & 4))] if (previous ^ projected) & 4 else [])

    def test_apply_state_is_visible_before_reentrant_notification(self):
        # Allow just one nested application.
        def nested():
            self.on_notify = None
            self.apply(0)
        self.on_notify = nested
        self.assertEqual(self.apply(7), 1)
        self.assertEqual(self.writes, [6, 0])
        self.assertEqual(self.notifications, [1, 0])
        self.assertEqual(self.flags & 6, 0)

    def test_apply_owner_survives_write_and_notify(self):
        self.on_write = lambda: self.host.context_ref(-1)
        self.assertEqual(self.apply(7), 1)
        self.assertEqual(self.notifications, [1])
        self.assertEqual(self.host.context_refs, 0)
        self.host.context_ref(1)

if __name__ == "__main__":
    suite = unittest.TestSuite(QueryTests(name) for name in dir(QueryTests) if name.startswith(("test_query_", "test_apply_")))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
