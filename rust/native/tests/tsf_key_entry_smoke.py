# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Native TSF key dispatch, with private COM hosts and substituted input transport."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parent))
from tsf_key_state_smoke import KeyStateTests, P, U32, HR, user

WOULD = c.CFUNCTYPE(HR, P, P, U32, c.c_uint64, HR, c.POINTER(c.c_uint16))
MARK = c.CFUNCTYPE(None, P, U32, HR)
COMPOSING = c.CFUNCTYPE(HR, P)
TOGGLE = c.CFUNCTYPE(None, P, P)
HANDLE = c.CFUNCTYPE(None, P, P, U32, c.c_uint16)
class Entry(c.Structure):
    _fields_ = [("owner", P), ("lifetime", P), ("would", WOULD), ("mark", MARK),
                ("composing", COMPOSING), ("toggle", TOGGLE), ("handle", HANDLE)]
class Passed(c.Structure):
    _fields_ = [("key", HR), ("control", HR)]
class Stale(c.Structure):
    _fields_ = [("tries", U32), ("key", U32), ("time", c.c_uint64)]

class EntryTests(KeyStateTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_key_event.argtypes = [P, U32, P, U32, c.c_uint64, c.POINTER(Entry)]
        cls.api.meltype_tsf_key_foreground.argtypes = [P, HR]
        cls.api.meltype_tsf_key_passed.argtypes = [P, HR, HR, c.POINTER(Passed)]
        cls.api.meltype_tsf_key_stale_read.argtypes = [P, c.POINTER(Stale)]
        cls.api.meltype_tsf_key_stale_write.argtypes = [P, U32, U32, c.c_uint64]
        cls.api.meltype_tsf_key_stale_clear.argtypes = [P]
        user.GetKeyboardState.argtypes = [c.POINTER(c.c_ubyte)]
        user.SetKeyboardState.argtypes = [c.POINTER(c.c_ubyte)]
        cls.api.meltype_tsf_key_char.argtypes = [U32, c.c_uint64]

    def setUp(self):
        super().setUp()
        self.keyboard = (c.c_ubyte * 256)()
        self.assertTrue(user.GetKeyboardState(self.keyboard))
        self.assertTrue(user.SetKeyboardState((c.c_ubyte * 256)()))
        self.events, self.accept, self.composing, self.on_would = [], True, False, None
        self.callbacks = [WOULD(self.would), MARK(lambda o,k,m: self.events.append(("mark", k, m))),
            COMPOSING(lambda o: int(self.composing)), TOGGLE(lambda o,ctx: self.events.append(("toggle",))),
            HANDLE(lambda o,ctx,k,ch: self.events.append(("handle", k, ch)))]
        self.entry = Entry(123, c.addressof(self.host.context), *self.callbacks)

    def tearDown(self):
        self.assertTrue(user.SetKeyboardState(self.keyboard))
        super().tearDown()

    def would(self, owner, ctx, key, message, test, ch):
        try:
            self.assertEqual(owner, 123)
            self.assertEqual(ctx, c.addressof(self.host.context))
            self.assertGreaterEqual(self.host.context_refs, 3)
            self.events.append(("would", key, test))
            ch[0] = 0xffff  # Dispatch must recompute the actual character.
            if self.on_would:
                self.on_would()
            return int(self.accept)
        except BaseException as error:
            self.errors.append(repr(error))
            return 0

    def event(self, kind, key=65):
        return self.api.meltype_tsf_key_event(self.state, kind, c.byref(self.host.context), key, 0, c.byref(self.entry))

    def passed(self, mode=0, changed=0):
        value = Passed()
        result = self.api.meltype_tsf_key_passed(self.state, mode, changed, c.byref(value))
        return result, value.key, value.control

    def test_entry_down_up_and_actual_character(self):
        self.assertEqual(self.event(0), 1)
        self.assertEqual(self.event(2), 0)
        self.assertEqual(self.event(1), 1)
        self.assertEqual(self.events[-1], ("handle", 65, self.api.meltype_tsf_key_char(65, 0)))
        self.assertEqual([self.event(k) for k in [2, 2, 3, 3]], [1, 1, 1, 0])

    def test_entry_rejected_key_metadata_and_modifier_exclusion(self):
        self.accept = False
        self.assertEqual(self.event(0), 0)
        self.assertEqual(self.events[-1], ("mark", 65, 2))
        self.assertEqual(self.passed(), (1, 65, 0))
        self.assertEqual(self.passed(), (0, 0, 0))
        self.events.clear()
        self.assertEqual(self.event(1, 0x10), 0)
        self.assertEqual(self.events, [("would", 0x10, 0)])
        self.assertEqual(self.passed()[0], 0)

    def test_entry_japanese_toggle_and_composition_routing(self):
        self.assertEqual(self.event(1, 0x19), 1)
        self.assertEqual(self.event(1, 0x19), 1)
        self.assertEqual(self.events.count(("toggle",)), 2)
        self.composing = True
        self.assertEqual(self.event(1, 0x19), 1)
        self.assertEqual(self.events[-1][0], "handle")

    def test_entry_replay_pair_preserves_second_packet(self):
        self.assertEqual(self.replay(), (1, 0))
        self.assertEqual(self.replay(), (1, 0))
        for kind in [0, 1, 0, 1]:
            self.assertEqual(self.event(kind), 0)
        self.assertEqual(self.events, [("mark", 65, 1)] * 4)
        self.assertEqual(self.event(1), 1)

    def test_entry_alt_grave_repeat_and_key_release(self):
        pressed = (c.c_ubyte * 256)()
        pressed[0x12] = pressed[0xa4] = 0x80
        self.assertTrue(user.SetKeyboardState(pressed))
        for _ in range(2):
            self.assertEqual(self.event(1, 0xc0), 1)
        self.assertEqual(self.events.count(("toggle",)), 1)
        self.assertEqual(self.event(3, 0xc0), 1)
        self.assertEqual(self.event(1, 0xc0), 1)
        self.assertEqual(self.events.count(("toggle",)), 2)

    def test_entry_control_metadata_is_consumed_but_retained_for_mode(self):
        pressed = (c.c_ubyte * 256)()
        pressed[0x11] = pressed[0xa2] = 0x80
        self.assertTrue(user.SetKeyboardState(pressed))
        self.accept = False
        self.event(1, 0x25)
        self.assertEqual(self.passed(), (1, 0x25, 1))
        self.assertEqual(self.passed(), (0, 0, 0))
        self.assertEqual(self.passed(1), (1, 0x25, 1))

    def test_entry_replay_releases_consumed_key(self):
        self.event(1)
        self.assertEqual(self.event(2), 1)
        self.assertEqual(self.replay(), (1, 0))
        self.assertEqual(self.event(2), 0)

    def test_entry_foreground_and_reentrant_reset(self):
        self.event(1)
        self.api.meltype_tsf_key_foreground(self.state, 0)
        self.assertEqual(self.event(2), 0)
        self.events.clear()
        self.on_would = lambda: self.api.meltype_tsf_key_state_reset(self.state)
        self.assertEqual(self.event(1), 0)
        self.assertEqual(self.events, [("would", 65, 0)])
        self.assertEqual(self.event(2), 0)

    def test_entry_stale_state_preserves_time_on_clear(self):
        stamp = (1 << 48) + 17
        self.api.meltype_tsf_key_stale_write(self.state, 7, 65, stamp)
        value = Stale()
        self.api.meltype_tsf_key_stale_read(self.state, c.byref(value))
        self.assertEqual((value.tries, value.key, value.time), (7, 65, stamp))
        self.api.meltype_tsf_key_stale_clear(self.state)
        self.api.meltype_tsf_key_stale_read(self.state, c.byref(value))
        self.assertEqual((value.tries, value.key, value.time), (0, 0, stamp))
        self.api.meltype_tsf_key_state_reset(self.state)
        self.api.meltype_tsf_key_stale_read(self.state, c.byref(value))
        self.assertEqual(value.time, 0)

    def test_entry_invalid_and_foreign_thread(self):
        self.assertEqual(self.event(4), 0)
        self.assertEqual(self.event(1, 256), 0)
        result = []
        worker = threading.Thread(target=lambda: result.append(self.event(1)))
        worker.start(); worker.join()
        self.assertEqual(result, [0])
        self.assertEqual(self.events, [])

if __name__ == "__main__":
    # Cache/replay cases run in their own suite; this suite covers only entry dispatch.
    suite = unittest.TestSuite(EntryTests(name) for name in dir(EntryTests) if name.startswith("test_entry_"))
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
