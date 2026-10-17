# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Mode switching against controlled focus/lock/range COM interfaces."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_fields_smoke import CompartmentHost, DISABLED
from tsf_range_smoke import Host, units, FAIL
from tsf_com_smoke import P, U32, HR, invoke, release
from tsf_key_entry_smoke import Entry, WOULD, MARK, COMPOSING, TOGGLE, HANDLE

READ = c.CFUNCTYPE(U32, P)
READY = c.CFUNCTYPE(HR, P)
SECRET = c.CFUNCTYPE(HR, P, U32, P)
SET = c.CFUNCTYPE(None, P, HR)
REQUEST = c.CFUNCTYPE(HR, P, c.c_char_p)
NOTICE = c.CFUNCTYPE(None, P, HR)
class Callbacks(c.Structure):
    _fields_ = [("owner", P), ("lifetime", P), ("read", READ), ("ready", READY),
                ("secret", SECRET), ("set_open", SET), ("request", REQUEST), ("notice", NOTICE)]

class ModeHost(CompartmentHost):
    def __init__(self):
        super().__init__()
        self.document = units('print("日本語😀")')
        self.ranges[self.original][:2] = [len(self.document), len(self.document)]
        self.read_count = 0
        self.lock_mode, self.pending, self.locks = "sync", None, []
        self.bind(self.context_table, 3, HR, [U32, P, U32, c.POINTER(HR)], self.edit)
        self.focus_refs = {"thread": 1, "document": 1}
        self.focus_fail = self.top_fail = False
        self.thread_table, self.document_table = (P * 8)(), (P * 7)()
        for name, table in [("thread", self.thread_table), ("document", self.document_table)]:
            self.bind(table, 1, U32, [], lambda _, n=name: self.focus_ref(n, 1))
            self.bind(table, 2, U32, [], lambda _, n=name: self.focus_ref(n, -1))
        self.bind(self.thread_table, 7, HR, [c.POINTER(P)], self.focus)
        self.bind(self.document_table, 6, HR, [c.POINTER(P)], self.top)
        self.thread, self.doc = self.Object(self.thread_table), self.Object(self.document_table)

    def focus_ref(self, name, delta):
        self.focus_refs[name] += delta
        return self.focus_refs[name]

    def focus(self, _, out):
        self.focus_ref("document", 1)
        out[0] = c.addressof(self.doc)
        return FAIL if self.focus_fail else 0

    def top(self, _, out):
        self.context_ref(1)
        out[0] = c.addressof(self.context)
        return FAIL if self.top_fail else 0

    def edit(self, _, client, session, flags, result):
        assert client == 17 and flags == 3  # TF_ES_SYNC | TF_ES_READ
        self.locks.append(flags)
        if self.lock_mode == "reject":
            result[0] = 0
            return FAIL
        if self.lock_mode == "async":
            invoke(session, 1, U32, [])
            self.pending = session
            result[0] = 0x40300
            return 0
        result[0] = invoke(session, 3, HR, [U32], 7)
        return 0

    def read(self, *args):
        self.read_count += 1
        return Host.read(self, *args)

    def verify(self, test):
        test.assertIsNone(self.pending)
        test.assertEqual(self.focus_refs, {"thread": 1, "document": 1})
        super().verify(test)

class ModeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_toggle_mode.argtypes = [P, P, P, U32, c.c_char_p, c.POINTER(Callbacks)]
        cls.api.meltype_tsf_code_line.argtypes = [P, P, U32, c.c_char_p, c.POINTER(Callbacks), HR]
        cls.api.meltype_tsf_key_state_create.restype = P
        cls.api.meltype_tsf_key_state_destroy.argtypes = [P]
        cls.api.meltype_tsf_key_event.argtypes = [P, U32, P, U32, c.c_uint64, c.POINTER(Entry)]

    def setUp(self):
        self.host = ModeHost()
        self.token = self.api.meltype_tsf_key_state_create()
        self.flags, self.ready, self.secret, self.done = 1, True, False, True
        self.events, self.requests, self.errors = [], [], []
        self.on_open = self.on_request = None
        self.metadata = json.dumps({"sid":'テスト"😀', "process":'C:\\app\\a".exe'}, ensure_ascii=False).encode()
        self.callbacks = [READ(self.read), READY(self.server), SECRET(self.private), SET(self.set_open), REQUEST(self.request), NOTICE(self.notice)]
        self.bridge = Callbacks(123, c.addressof(self.host.context), *self.callbacks)

    def tearDown(self):
        if self.host.pending:
            release(self.host.pending)
            self.host.pending = None
        self.api.meltype_tsf_key_state_destroy(self.token)
        self.host.verify(self)
        self.assertEqual(self.errors, [])

    def read(self, owner):
        return self.flags

    def server(self, owner):
        self.events.append(("ready",))
        return int(self.ready)

    def private(self, owner, cookie, context):
        try:
            self.assertEqual(cookie, 7)
            self.assertEqual(context, c.addressof(self.host.context))
            self.assertGreaterEqual(self.host.context_refs, 4)
            self.events.append(("secret",))
            return int(self.secret)
        except BaseException as error:
            self.errors.append(repr(error))
            return 1

    def set_open(self, owner, opened):
        self.events.append(("open", opened))
        self.flags = (self.flags & ~1) | int(opened != 0)
        if self.on_open:
            self.on_open()

    def request(self, owner, text):
        try:
            self.assertGreaterEqual(self.host.context_refs, 2)
            self.requests.append(json.loads(text))
            if self.on_request:
                self.on_request()
            return int(self.done)
        except BaseException as error:
            self.errors.append(repr(error))
            return 0

    def notice(self, owner, japanese):
        try:
            self.assertGreaterEqual(self.host.context_refs, 2)
            self.events.append(("notice", japanese))
        except BaseException as error:
            self.errors.append(repr(error))

    def toggle(self, context=True, focus=False):
        return self.api.meltype_tsf_toggle_mode(c.byref(self.host.context) if context else None,
            c.byref(self.host.thread) if focus else None, self.token, 17, self.metadata, c.byref(self.bridge))

    def line(self, japanese=True, context=True):
        return self.api.meltype_tsf_code_line(c.byref(self.host.context) if context else None,
            self.token, 17, self.metadata, c.byref(self.bridge), int(japanese))

    def test_ordinary_toggle_does_not_read_document(self):
        self.assertEqual(self.toggle(), 1)
        self.assertEqual(self.events, [("open", 0)])
        self.assertEqual(self.toggle(), 1)
        self.assertEqual(self.events, [("open", 0), ("open", 1)])
        self.assertEqual(self.host.read_count, 0)

    def test_code_japanese_switches_line_instead_of_closing(self):
        self.flags = 3
        self.assertEqual(self.toggle(), 1)
        self.assertEqual(self.flags & 1, 1)
        self.assertEqual(self.requests[0]["japanese"], False)
        self.assertEqual(self.events[-1], ("notice", 0))

    def test_code_english_opens_then_switches_line(self):
        self.flags = 6
        self.assertEqual(self.toggle(), 1)
        self.assertEqual(self.events[0], ("open", 1))
        self.assertEqual(self.requests[0]["japanese"], True)

    def test_open_notification_refreshes_code_state(self):
        self.flags = 0
        self.on_open = lambda: setattr(self, "flags", 7)
        self.toggle()
        self.assertEqual(len(self.requests), 1)

    def test_failed_code_toggle_closes_global_mode(self):
        self.flags, self.done = 3, False
        self.toggle()
        self.assertEqual(self.events[-1], ("open", 0))
        self.assertNotIn(("notice", 0), self.events)

    def test_secret_never_reads_text_or_sends_request(self):
        self.flags, self.secret = 3, True
        self.assertEqual(self.line(), 0)
        self.assertEqual(self.host.read_count, 0)
        self.assertEqual(self.requests, [])
        self.toggle()
        self.assertEqual(self.events[-1], ("open", 0))

    def test_disabled_context_does_not_query_server(self):
        self.host.integers[bytes(DISABLED)] = 1
        self.assertEqual(self.line(), 0)
        self.assertEqual(self.events, [])

    def test_server_unavailable_does_not_read_document(self):
        self.ready = False
        self.assertEqual(self.line(), 0)
        self.assertEqual(self.events, [("ready",)])
        self.assertEqual(self.host.read_count, 0)

    def test_unicode_request_and_before_context(self):
        self.assertEqual(self.line(), 1)
        self.assertEqual(self.requests, [{"op":"toggle", **json.loads(self.metadata), "japanese":True, "before":'print("日本語😀")'}])
        self.assertEqual(self.host.locks, [3])

    def test_refused_read_uses_tracked_line_without_before(self):
        self.host.lock_mode = "reject"
        self.assertEqual(self.line(), 1)
        self.assertNotIn("before", self.requests[0])
        self.assertEqual(self.host.read_count, 0)

    def test_delayed_sync_session_cannot_read_after_return(self):
        self.host.lock_mode = "async"
        self.assertEqual(self.line(), 1)
        self.assertNotIn("before", self.requests[0])
        self.assertEqual(invoke(self.host.pending, 3, HR, [U32], 7), HR(0x80004004).value)
        release(self.host.pending)
        self.host.pending = None
        self.assertEqual(self.host.read_count, 0)
        self.assertNotIn(("secret",), self.events)

    def test_null_line_context_does_not_resolve_focus(self):
        self.assertEqual(self.line(context=False), 0)
        self.assertEqual(self.events, [])
        self.assertEqual(self.toggle(context=False), 1)
        self.assertEqual(self.events, [("open", 0)])

    def test_focus_context_is_used_and_owned(self):
        self.flags = 3
        self.assertEqual(self.toggle(context=False, focus=True), 1)
        self.assertEqual(self.requests[0]["before"], 'print("日本語😀")')

    def test_failed_focus_releases_partial_output_and_closes(self):
        self.flags, self.host.focus_fail = 3, True
        self.toggle(context=False, focus=True)
        self.assertEqual(self.requests, [])
        self.assertEqual(self.events, [("open", 0)])

    def test_failed_top_releases_partial_context_and_closes(self):
        self.flags, self.host.top_fail = 3, True
        self.toggle(context=False, focus=True)
        self.assertEqual(self.requests, [])
        self.assertEqual(self.events, [("open", 0)])

    def test_bad_metadata_does_not_toggle_or_read(self):
        self.metadata = b'{"sid":17}'
        self.assertEqual(self.toggle(), 0)
        self.assertEqual(self.line(), 0)
        self.assertEqual(self.events, [])

    def test_passed_key_is_included_once_and_consumed_before_private_check(self):
        callbacks = [WOULD(lambda *args: 0), MARK(lambda *args: None), COMPOSING(lambda *args: 0),
                     TOGGLE(lambda *args: None), HANDLE(lambda *args: None)]
        entry = Entry(123, c.addressof(self.host.context), *callbacks)
        def remember():
            self.assertEqual(self.api.meltype_tsf_key_event(self.token, 1, c.byref(self.host.context), 65, 0, c.byref(entry)), 0)
        remember()
        self.line()
        self.assertEqual(self.requests[-1]["passed"], 65)
        self.line()
        self.assertNotIn("passed", self.requests[-1])
        remember()
        self.secret = True
        self.assertEqual(self.line(), 0)
        self.secret = False
        self.line()
        self.assertNotIn("passed", self.requests[-1])

    def test_owner_survives_original_reference_release_in_request(self):
        self.on_request = lambda: self.host.context_ref(-1)
        self.assertEqual(self.line(), 1)
        self.assertEqual(self.host.context_refs, 0)
        # Restore the controlled host's baseline reference for fixture checks.
        self.host.context_ref(1)
        self.assertEqual(self.events[-1], ("notice", 1))

if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
