# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""TSF source/key subscriptions through actual SDK-layout COM calls."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_fields_smoke import CompartmentHost, MANAGER, OPEN
from tsf_com_smoke import Guid, P, U32, HR, UNKNOWN, invoke, release
from tsf_range_smoke import FAIL
SOURCE = Guid.parse("4ea48a35-60ae-446f-8fd6-e6a8d82459f7")
KEYS = Guid.parse("aa80e7f0-2021-11d2-93e0-0060b067b86e")
THREAD_SINK = Guid.parse("aa80e80e-2021-11d2-93e0-0060b067b86e")
MODE_SINK = Guid.parse("743abd5f-f26d-48df-8cc5-238492419b64")
EDIT_SINK = Guid.parse("8127d409-ccd3-4683-967a-b43d5b482bf7")


class Source:
    def __init__(self, host, name):
        self.host, self.name, self.refs = host, name, 1
        self.registrations, self.unadvised = {}, []
        self.fail = self.partial = False
        self.on_unadvise = None
        self.next = 101
        self.table = (P * 5)()
        host.bind(self.table, 1, U32, [], lambda _: self.ref(1))
        host.bind(self.table, 2, U32, [], lambda _: self.ref(-1))
        host.bind(self.table, 3, HR, [c.POINTER(Guid), P, c.POINTER(U32)], self.advise)
        host.bind(self.table, 4, HR, [U32], self.unadvise)
        self.object = host.Object(self.table)

    def ref(self, delta):
        self.refs += delta
        assert self.refs >= 1
        return self.refs

    def advise(self, this, iid, sink, cookie):
        kind = bytes(iid.contents)
        assert kind in [bytes(THREAD_SINK), bytes(MODE_SINK), bytes(EDIT_SINK)]
        self.host.events.append((self.name, "advise", kind))
        if not self.fail or self.partial:
            cookie[0] = self.next
            invoke(sink, 1, U32, [])
            self.registrations[self.next] = sink
            self.next += 1
        return FAIL if self.fail else 0

    def unadvise(self, this, cookie):
        self.unadvised.append(cookie)
        self.host.events.append((self.name, "unadvise", cookie))
        assert cookie in self.registrations
        release(self.registrations.pop(cookie))
        if self.on_unadvise:
            self.on_unadvise()
        return 0


class SubscriptionHost(CompartmentHost):
    def __init__(self):
        super().__init__()
        self.events = []
        self.sources = [Source(self, name) for name in ["original", "replacement", "mode"]]
        self.active_source = self.sources[0]
        self.fail_source = self.partial_source = self.fail_keys = False
        self.fail_key_advise = False
        self.fail_top = self.partial_top = False
        self.top_calls = 0
        self.keys_refs = self.document_refs = self.sink_refs = 1
        self.key_calls, self.key_sinks, self.key_unadvised = [], {}, []
        self.keys_table, self.document_table, self.sink_table = (P * 5)(), (P * 9)(), (P * 3)()
        self.bind(self.keys_table, 1, U32, [], lambda _: self.count("keys_refs", 1))
        self.bind(self.keys_table, 2, U32, [], lambda _: self.count("keys_refs", -1))
        self.bind(self.keys_table, 3, HR, [U32, P, HR], self.advise_keys)
        self.bind(self.keys_table, 4, HR, [U32], self.unadvise_keys)
        self.bind(self.document_table, 1, U32, [], lambda _: self.count("document_refs", 1))
        self.bind(self.document_table, 2, U32, [], lambda _: self.count("document_refs", -1))
        self.bind(self.document_table, 6, HR, [c.POINTER(P)], self.get_top)
        self.bind(self.sink_table, 1, U32, [], lambda _: self.count("sink_refs", 1))
        self.bind(self.sink_table, 2, U32, [], lambda _: self.count("sink_refs", -1))
        self.keys, self.document_manager, self.sink = self.Object(self.keys_table), self.Object(self.document_table), self.Object(self.sink_table)
        table, _ = self.compartments[bytes(OPEN)]
        self.bind(table, 0, HR, [c.POINTER(Guid), c.POINTER(P)], self.query_mode)
        self.on_write = lambda: self.events.append(("mode", "set", self.integers[bytes(OPEN)]))

    def count(self, name, delta):
        value = getattr(self, name) + delta
        assert value >= 1
        setattr(self, name, value)
        return value

    def query(self, this, iid, out):
        kind = bytes(iid.contents)
        out[0] = None
        if kind == bytes(UNKNOWN):
            self.context_ref(1)
            out[0] = c.addressof(self.context)
            return 0
        if kind == bytes(SOURCE):
            if not self.fail_source or self.partial_source:
                self.active_source.ref(1)
                out[0] = c.addressof(self.active_source.object)
            return FAIL if self.fail_source else 0
        if kind == bytes(KEYS):
            if self.fail_keys:
                return FAIL
            self.count("keys_refs", 1)
            out[0] = c.addressof(self.keys)
            return 0
        return super().query(this, iid, out)

    def query_mode(self, this, iid, out):
        out[0] = None
        if bytes(iid.contents) == bytes(UNKNOWN):
            self.compartment_ref(bytes(OPEN), 1)
            out[0] = c.addressof(self.compartments[bytes(OPEN)][1])
        elif bytes(iid.contents) == bytes(SOURCE):
            self.sources[2].ref(1)
            out[0] = c.addressof(self.sources[2].object)
        else:
            return FAIL
        return 0

    def get_top(self, this, out):
        self.top_calls += 1
        out[0] = None
        if not self.fail_top or self.partial_top:
            self.context_ref(1)
            out[0] = c.addressof(self.context)
        return FAIL if self.fail_top else 0

    def advise_keys(self, this, client, sink, foreground):
        assert client == 17 and foreground == 1
        self.key_calls.append((client, foreground))
        if self.fail_key_advise:
            return FAIL
        invoke(sink, 1, U32, [])
        self.key_sinks[client] = sink
        return 0

    def unadvise_keys(self, this, client):
        self.key_unadvised.append(client)
        release(self.key_sinks.pop(client))
        return 0

    def verify(self, test):
        super().verify(test)
        test.assertEqual((self.keys_refs, self.document_refs, self.sink_refs), (1, 1, 1))
        test.assertEqual(self.key_sinks, {})
        for source in self.sources:
            test.assertEqual(source.refs, 1)
            test.assertEqual(source.registrations, {})


class SubscriptionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_subscribe.argtypes = [U32, P, U32, P, c.POINTER(P)]
        cls.api.meltype_tsf_subscribe.restype = P
        cls.api.meltype_tsf_unsubscribe.argtypes = [P]

    def setUp(self):
        self.host = SubscriptionHost()
        self.tokens, self.contexts = [], []

    def tearDown(self):
        for token in self.tokens:
            self.api.meltype_tsf_unsubscribe(token)
        for context in self.contexts:
            release(context)
        self.host.verify(self)

    def subscribe(self, kind):
        out = P(123)
        owner = self.host.document_manager if kind == 3 else self.host.context
        token = self.api.meltype_tsf_subscribe(kind, c.byref(owner), 17, c.byref(self.host.sink), c.byref(out))
        if token:
            self.tokens.append(token)
        if out.value:
            self.contexts.append(out.value)
        return token, out.value

    def test_thread_subscription_retains_source_and_balances_sink(self):
        token, context = self.subscribe(0)
        self.assertTrue(token)
        self.assertIsNone(context)
        self.assertEqual((self.host.sources[0].refs, self.host.sink_refs), (2, 2))
        self.api.meltype_tsf_unsubscribe(token)
        self.assertEqual(self.host.sources[0].unadvised, [101])
        self.assertEqual(self.host.sink_refs, 1)

    def test_keys_use_foreground_client_and_retained_manager(self):
        token, _ = self.subscribe(1)
        self.assertTrue(token)
        self.assertEqual(self.host.key_calls, [(17, 1)])
        self.assertEqual((self.host.keys_refs, self.host.sink_refs), (2, 2))
        self.api.meltype_tsf_unsubscribe(token)
        self.assertEqual(self.host.key_unadvised, [17])

    def test_mode_sets_japanese_before_advising_and_uses_mode_source(self):
        token, _ = self.subscribe(2)
        self.assertTrue(token)
        self.assertEqual(self.host.events, [("mode", "set", 1), ("mode", "advise", bytes(MODE_SINK))])
        self.api.meltype_tsf_unsubscribe(token)
        self.assertEqual(self.host.sources[2].unadvised, [101])

    def test_refused_initial_mode_write_still_subscribes(self):
        self.host.fail_write = True
        token, _ = self.subscribe(2)
        self.assertTrue(token)
        self.assertEqual(self.host.events, [("mode", "advise", bytes(MODE_SINK))])

    def test_edit_subscription_returns_owned_context(self):
        token, context = self.subscribe(3)
        self.assertTrue(token)
        self.assertEqual(context, c.addressof(self.host.context))
        self.assertEqual(self.host.context_refs, 2)
        self.assertEqual(self.host.events[0], ("original", "advise", bytes(EDIT_SINK)))

    def test_unsubscribe_uses_original_source_after_replacement(self):
        token, _ = self.subscribe(0)
        self.host.active_source = self.host.sources[1]
        self.api.meltype_tsf_unsubscribe(token)
        self.assertEqual(self.host.sources[0].unadvised, [101])
        self.assertEqual(self.host.sources[1].unadvised, [])

    def test_reentrant_and_repeated_unsubscribe_is_idempotent(self):
        token, _ = self.subscribe(0)
        self.host.sources[0].on_unadvise = lambda: self.api.meltype_tsf_unsubscribe(token)
        self.api.meltype_tsf_unsubscribe(token)
        self.api.meltype_tsf_unsubscribe(token)
        self.assertEqual(self.host.sources[0].unadvised, [101])

    def test_failed_advise_with_cookie_rolls_back(self):
        self.host.sources[0].fail = self.host.sources[0].partial = True
        self.assertEqual(self.subscribe(0), (None, None))
        self.assertEqual(self.host.sources[0].unadvised, [101])

    def test_failed_advise_without_cookie_does_not_unsubscribe_another_sink(self):
        self.host.sources[0].fail = True
        self.assertEqual(self.subscribe(0), (None, None))
        self.assertEqual(self.host.sources[0].unadvised, [])

    def test_failed_key_advise_does_not_claim_client_subscription(self):
        self.host.fail_key_advise = True
        self.assertEqual(self.subscribe(1), (None, None))
        self.assertEqual(self.host.key_calls, [(17, 1)])
        self.assertEqual((self.host.key_unadvised, self.host.sink_refs), ([], 1))

    def test_failed_source_query_releases_partial_output(self):
        self.host.fail_source = self.host.partial_source = True
        self.assertEqual(self.subscribe(0), (None, None))
        self.assertEqual(self.host.sources[0].refs, 1)

    def test_failed_top_context_releases_partial_output(self):
        self.host.fail_top = self.host.partial_top = True
        self.assertEqual(self.subscribe(3), (None, None))
        self.assertEqual(self.host.context_refs, 1)

    def test_foreign_thread_cannot_unsubscribe_sta_source(self):
        token, _ = self.subscribe(0)
        worker = threading.Thread(target=lambda: self.api.meltype_tsf_unsubscribe(token))
        worker.start()
        worker.join()
        self.assertEqual(self.host.sources[0].unadvised, [])
        self.assertEqual(self.host.sink_refs, 2)

    def test_independent_tokens_remove_only_their_own_sink(self):
        first, _ = self.subscribe(0)
        second, _ = self.subscribe(3)
        self.assertNotEqual(first, second)
        self.api.meltype_tsf_unsubscribe(first)
        self.assertEqual(list(self.host.sources[0].registrations), [102])
        self.assertEqual(self.host.sink_refs, 2)

    def test_invalid_kind_and_null_inputs_do_not_touch_sources(self):
        self.assertEqual(self.subscribe(9), (None, None))
        out = P(123)
        self.assertFalse(self.api.meltype_tsf_subscribe(0, None, 17, c.byref(self.host.sink), c.byref(out)))
        self.assertIsNone(out.value)
        self.assertFalse(self.api.meltype_tsf_subscribe(3, c.byref(self.host.document_manager), 17, c.byref(self.host.sink), None))
        self.assertEqual((self.host.events, self.host.top_calls), ([], 0))


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
