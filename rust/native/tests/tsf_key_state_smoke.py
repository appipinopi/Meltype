# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Privacy cache/COM lifetimes and replay protocol using no actual SendInput."""
import ctypes as c
from pathlib import Path
import sys
import threading
import time
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_fields_smoke import FieldHost
from tsf_com_smoke import P, U32, HR
MARKER = 0x4d4c5450
user, kernel = c.WinDLL("user32"), c.WinDLL("kernel32")
user.GetMessageExtraInfo.restype = c.c_ssize_t
user.SetMessageExtraInfo.argtypes = [c.c_ssize_t]
user.SetMessageExtraInfo.restype = c.c_ssize_t
kernel.SetLastError.argtypes = [U32]


class Keyboard(c.Structure):
    _fields_ = [("key", c.c_uint16), ("scan", c.c_uint16), ("flags", U32), ("time", U32), ("extra", c.c_size_t)]
class Mouse(c.Structure):
    _fields_ = [("x", HR), ("y", HR), ("data", U32), ("flags", U32), ("time", U32), ("extra", c.c_size_t)]
class Hardware(c.Structure):
    _fields_ = [("message", U32), ("low", c.c_uint16), ("high", c.c_uint16)]
class Payload(c.Union):
    _fields_ = [("keyboard", Keyboard), ("mouse", Mouse), ("hardware", Hardware)]
class Input(c.Structure):
    _fields_ = [("kind", U32), ("payload", Payload)]
READER = c.CFUNCTYPE(HR, P, P, c.POINTER(HR))
SENDER = c.CFUNCTYPE(U32, P, c.POINTER(Input), U32)


class ReentrantHost(FieldHost):
    def __init__(self):
        self.on_release = None
        super().__init__()

    def context_ref(self, delta):
        super().context_ref(delta)
        if delta < 0 and self.on_release:
            callback, self.on_release = self.on_release, None
            callback()
        return self.context_refs


class KeyStateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api = c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_key_state_create.restype = P
        for name in ["reset", "destroy"]:
            getattr(cls.api, "meltype_tsf_key_state_" + name).argtypes = [P]
        cls.api.meltype_tsf_key_cache_clear.argtypes = [P]
        cls.api.meltype_tsf_key_secret.argtypes = [P, P, U32, HR, P, P, READER, c.POINTER(HR)]
        cls.api.meltype_tsf_reinject.argtypes = [P, U32, P, P, SENDER, c.POINTER(U32)]
        cls.api.meltype_tsf_is_reinjected.argtypes = [P, U32]

    def setUp(self):
        self.state = self.api.meltype_tsf_key_state_create()
        self.assertTrue(self.state)
        self.hosts = [ReentrantHost(), ReentrantHost()]
        self.host = self.hosts[0]
        self.calls, self.sent, self.errors = [], [], []
        self.result, self.private = 0, 1
        self.on_read = self.on_send = None
        self.reader = READER(self.read)
        self.sender = SENDER(self.send)
        self.extra = user.GetMessageExtraInfo()
        user.SetMessageExtraInfo(0)

    def tearDown(self):
        user.SetMessageExtraInfo(self.extra)
        self.api.meltype_tsf_key_state_destroy(self.state)
        for host in self.hosts:
            host.verify(self)
        self.assertEqual(self.errors, [])

    def read(self, owner, context, private):
        try:
            assert owner == 123 and context in [c.addressof(h.context) for h in self.hosts]
            self.assertGreaterEqual(self.host.context_refs, 2)
            self.calls.append(context)
            if self.on_read:
                return self.on_read(private)
            private[0] = self.private
            return self.result
        except BaseException as error:
            self.errors.append(repr(error))
            return 1

    def secret(self, key=65, test=False, host=None):
        private = HR(999)
        result = self.api.meltype_tsf_key_secret(self.state, c.byref((host or self.host).context), key,
            int(test), 123, c.byref(self.host.context), self.reader, c.byref(private))
        return result, private.value

    def send(self, owner, inputs, count):
        try:
            assert owner == 456 and count == 2
            self.assertGreaterEqual(self.host.context_refs, 2)
            copied = []
            for index in range(count):
                value = inputs[index]
                self.assertEqual(value.kind, 1)
                key = value.payload.keyboard
                copied.append((key.key, key.scan, key.flags, key.time, key.extra))
            self.sent.append(copied)
            return self.on_send(inputs) if self.on_send else 2
        except BaseException as error:
            self.errors.append(repr(error))
            return 0

    def replay(self, key=65):
        error = U32(999)
        result = self.api.meltype_tsf_reinject(self.state, key, c.byref(self.host.context),
            456, self.sender, c.byref(error))
        return result, error.value

    def observe(self, key, marked=True):
        user.SetMessageExtraInfo(MARKER if marked else 0)
        return bool(self.api.meltype_tsf_is_reinjected(self.state, key))

    def test_cache_reuses_same_key_context_once_and_preserves_private_result(self):
        self.assertEqual(self.secret(test=True), (0, 1))
        self.assertEqual(self.host.context_refs, 2)
        self.result, self.private = 1, 0
        self.assertEqual(self.secret(), (0, 1))
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.secret(), (1, 0))
        self.assertEqual(len(self.calls), 2)

    def test_other_key_or_context_does_not_reuse_cache(self):
        self.secret(test=True)
        self.secret(key=66)
        self.secret(test=True)
        self.secret(host=self.hosts[1])
        self.assertEqual(len(self.calls), 4)

    def test_expired_result_is_read_again(self):
        self.secret(test=True)
        time.sleep(0.22)
        self.secret()
        self.assertEqual(len(self.calls), 2)

    def test_clear_reset_destroy_release_owned_context(self):
        for action in [self.api.meltype_tsf_key_cache_clear, self.api.meltype_tsf_key_state_reset]:
            self.secret(test=True)
            self.assertEqual(self.host.context_refs, 2)
            action(self.state)
            self.assertEqual(self.host.context_refs, 1)
        self.secret(test=True)
        self.api.meltype_tsf_key_state_destroy(self.state)
        self.assertEqual(self.host.context_refs, 1)

    def test_unknown_and_secret_results_are_cached_without_reclassification(self):
        for result in [-1, 1]:
            self.result = result
            self.assertEqual(self.secret(test=True), (result, 1))
            self.result = 0
            self.assertEqual(self.secret(), (result, 1))

    def test_reentrant_reset_does_not_resurrect_old_cache(self):
        def reset(private):
            self.api.meltype_tsf_key_state_reset(self.state)
            private[0] = 1
            return 0
        self.on_read = reset
        self.secret(test=True)
        self.assertEqual(self.host.context_refs, 1)
        self.on_read = None
        self.secret()
        self.assertEqual(len(self.calls), 2)

    def test_nested_newer_cache_is_not_overwritten_by_outer_read(self):
        def nested(private):
            self.on_read = None
            self.result = self.private = 1
            self.assertEqual(self.secret(key=66, test=True), (1, 1))
            private[0] = 0
            return 0
        self.on_read = nested
        self.assertEqual(self.secret(test=True), (0, 0))
        self.assertEqual(self.secret(key=66), (1, 1))
        self.assertEqual(len(self.calls), 2)

    def test_context_release_can_reenter_without_holding_state_borrow(self):
        self.secret(test=True)
        self.host.on_release = lambda: self.secret(key=66, test=True)
        self.api.meltype_tsf_key_cache_clear(self.state)
        self.assertEqual(len(self.calls), 2)
        self.assertEqual(self.secret(key=66), (0, 1))
        self.assertEqual(len(self.calls), 2)

    def test_transport_input_matches_sdk_keys_scans_flags_and_marker(self):
        self.assertEqual(c.sizeof(Input), 40 if c.sizeof(P) == 8 else 28)
        for key in [65, 0x25, 0x27, 0x26, 0x28, 0x24, 0x23, 0x21, 0x22, 0x2d, 0x2e]:
            self.api.meltype_tsf_key_state_reset(self.state)
            self.assertEqual(self.replay(key), (1, 0))
            extended = 1 if key != 65 else 0
            scan = user.MapVirtualKeyW(key, 0) & 0xffff
            self.assertEqual(self.sent[-1], [(key, scan, extended, 0, MARKER), (key, scan, extended | 2, 0, MARKER)])
            self.assertTrue(self.observe(key))
            self.assertFalse(self.observe(key))

    def test_known_marker_does_not_consume_unmarked_human_key(self):
        self.replay(65)
        self.assertTrue(self.observe(65))
        self.replay(66)
        self.assertFalse(self.observe(66, marked=False))
        self.assertTrue(self.observe(66))

    def test_callback_can_observe_pending_before_sender_returns(self):
        self.on_send = lambda _: 2 if self.observe(65) else 0
        self.assertEqual(self.replay(), (1, 0))
        self.assertFalse(self.observe(65))

    def test_partial_or_failed_send_removes_only_that_pending_entry(self):
        for count in [0, 1]:
            def failure(_):
                kernel.SetLastError(5)
                return count
            self.on_send = failure
            self.assertEqual(self.replay(), (0, 5))
            self.assertFalse(self.observe(65))

    def test_guard_suppresses_loop_and_reset_allows_new_request(self):
        self.assertEqual([self.replay()[0] for _ in range(5)], [1, 1, 1, 1, 2])
        self.assertEqual(len(self.sent), 4)
        self.api.meltype_tsf_key_state_reset(self.state)
        self.assertEqual(self.replay(), (1, 0))

    def test_reentrant_reset_and_new_send_survive_outer_failure(self):
        def nested(_):
            self.on_send = None
            self.api.meltype_tsf_key_state_reset(self.state)
            self.assertEqual(self.replay(66), (1, 0))
            return 0
        self.on_send = nested
        self.assertEqual(self.replay()[0], 0)
        self.assertTrue(self.observe(66))

    def test_sender_can_destroy_state_without_dangling_transport_buffers(self):
        def destroy(inputs):
            self.api.meltype_tsf_key_state_destroy(self.state)
            self.assertEqual(inputs[1].payload.keyboard.key, 65)
            return 2
        self.on_send = destroy
        self.assertEqual(self.replay(), (1, 0))
        self.assertFalse(self.observe(65))

    def test_foreign_thread_cannot_use_or_release_sta_state(self):
        self.secret(test=True)
        results = []
        def worker():
            results.append(self.secret())
            results.append(self.replay()[0])
            self.api.meltype_tsf_key_cache_clear(self.state)
            self.api.meltype_tsf_key_state_destroy(self.state)
        thread = threading.Thread(target=worker)
        thread.start()
        thread.join()
        self.assertEqual(results, [(1, 0), -1])
        self.assertEqual(self.secret(), (0, 1))
        self.assertEqual(len(self.calls), 1)
        self.assertEqual(self.sent, [])


if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
