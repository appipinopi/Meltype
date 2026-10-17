# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Native state layout/defaults, independent storage and balanced lifetimes."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
P=c.c_void_p
U32=c.c_uint32
class State(c.Structure):
    _fields_=[
        ('refs',c.c_int32),
        ('manager',P),
        ('client',U32),
        ('thread_subscription',P),
        ('key_subscription',P),
        ('mode_subscription',P),
        ('edit_subscription',P),
        ('edit_context',P),
        ('composition',P),
        ('composition_context',P),
        ('atom_input',U32),
        ('atom_converted',U32),
        ('atom_target',U32),
        ('keys',P),
        ('resume',P),
        ('abandoned',P),
        ('last_hello',c.c_uint64),
        ('focus_epoch',c.c_uint64),
        ('mode_epoch',c.c_uint64),
        ('shutdown_phase',U32),
        ('server_active',c.c_bool),
        ('direct_detection',c.c_bool),
        ('direct_word',c.c_bool),
        ('open',c.c_bool),
        ('code_app',c.c_bool),
        ('code_english',c.c_bool),
        ('disabled',c.c_bool),
        ('own_process',c.c_bool),
        ('last_converting',c.c_bool),
        ('caret_moved',c.c_bool),
        ('own_edit',c.c_bool),
        ('secret_checked',c.c_bool),
        ('secret_composing',c.c_bool),
        ('private_field',c.c_bool),
        ('keep_caret',c.c_bool),
        ('deactivated',c.c_bool),
        ('key_failed',c.c_bool),
        ('consumed',c.c_bool),
        ('scope_source',P),
        ('pass_reason',P),
    ]
class StateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_service_state_create.restype=P
        cls.api.meltype_tsf_service_state_destroy.argtypes=[P]
        cls.api.meltype_tsf_service_state_destroy.restype=None
        cls.api.meltype_tsf_service_state_layout.argtypes=[c.POINTER(U32),U32]
        cls.api.meltype_tsf_service_state_layout.restype=U32
        cls.api.meltype_tsf_service_state_live_count.restype=U32
        size=cls.api.meltype_tsf_service_state_layout(None,0)
        metadata=(U32*size)();cls.api.meltype_tsf_service_state_layout(metadata,size)
        expected=[1,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
        if list(metadata)!=expected:raise AssertionError('State ABI mismatch; do not borrow this allocation')
    def setUp(self):self.owned=[];self.before=self.live()
    def tearDown(self):
        for data in self.owned:self.api.meltype_tsf_service_state_destroy(data)
        self.assertEqual(self.live(),self.before)
    def live(self):return self.api.meltype_tsf_service_state_live_count()
    def create(self):
        data=self.api.meltype_tsf_service_state_create()
        self.assertTrue(data);self.owned.append(data);return State.from_address(data)
    def test_layout_matches_all_fields_before_borrowing(self):
        size=self.api.meltype_tsf_service_state_layout(None,0)
        result=(U32*size)();self.assertEqual(self.api.meltype_tsf_service_state_layout(result,size),size)
        expected=[1,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
        self.assertEqual(list(result),expected)
    def test_short_metadata_buffer_is_untouched(self):
        size=self.api.meltype_tsf_service_state_layout(None,0);result=(U32*(size+2))(*([0xaabbccdd]*(size+2)))
        self.assertEqual(self.api.meltype_tsf_service_state_layout(result,size-1),size)
        self.assertEqual(list(result),[0xaabbccdd]*(size+2))
    def test_defaults_match_upstream_service(self):
        state=self.create()
        for name,kind in State._fields_:
            with self.subTest(field=name):
                expected=1 if name=='refs' else name in ['open','consumed'] if kind==c.c_bool else None if kind==P else 0
                self.assertEqual(getattr(state,name),expected)
    def test_independent_instances_and_64bit_times(self):
        first=self.create();second=self.create();self.assertEqual(self.live(),self.before+2)
        first.server_active=True;first.open=False;first.last_hello=0xfedcba9876543210;first.focus_epoch=0x100000001
        self.assertFalse(second.server_active);self.assertTrue(second.open);self.assertEqual(second.last_hello,0)
        self.assertEqual(first.last_hello,0xfedcba9876543210);self.assertEqual(first.focus_epoch,0x100000001)
    def test_null_destroy_is_inert(self):
        self.api.meltype_tsf_service_state_destroy(None);self.assertEqual(self.live(),self.before)
    def test_independent_ui_threads_balance_storage(self):
        errors=[]
        def work():
            try:
                for _ in range(100):
                    data=self.api.meltype_tsf_service_state_create()
                    if not data:raise AssertionError('allocation failed')
                    state=State.from_address(data);state.open=False;state.mode_epoch=123456789
                    self.api.meltype_tsf_service_state_destroy(data)
            except BaseException as e:errors.append(repr(e))
        threads=[threading.Thread(target=work) for _ in range(4)]
        for thread in threads:thread.start()
        for thread in threads:thread.join()
        self.assertEqual(errors,[]);self.assertEqual(self.live(),self.before)
if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)

