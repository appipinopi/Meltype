# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Focus ordering, borrowed document/owner pins and nested focus precedence."""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_termination_smoke import TerminationTests, READ, P, U32, HR
from tsf_key_entry_smoke import Entry, WOULD, MARK, COMPOSING, TOGGLE, HANDLE
COMMIT=c.CFUNCTYPE(c.c_int,P,c.c_int)
ACTION=c.CFUNCTYPE(None,P,U32,P)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('read',READ),('commit',COMMIT),('action',ACTION)]
class FocusTests(TerminationTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_document_focus.argtypes=[P,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_document_focus.restype=HR
        cls.api.meltype_tsf_foreground_focus.argtypes=[c.c_int,P,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_foreground_focus.restype=HR
        cls.api.meltype_tsf_key_state_create.restype=P
        cls.api.meltype_tsf_key_state_destroy.argtypes=[P]
        cls.api.meltype_tsf_key_event.argtypes=[P,U32,P,U32,c.c_uint64,c.POINTER(Entry)]
    def setUp(self):
        super().setUp()
        self.epoch=c.c_uint64()
        self.doc=c.addressof(self.other.context)
        self.commit_ok=1;self.on_commit=self.on_action=None
        def guard(fn,default=None):
            def run(*args):
                try:
                    self.assertEqual(args[0],123)
                    self.assertGreaterEqual(self.host.context_refs,2)
                    return fn(*args)
                except BaseException as e:self.errors.append(repr(e));return default
            return run
        self.focus_callbacks=[READ(guard(lambda _:self.flags,4)),COMMIT(guard(self.commit,0)),ACTION(guard(self.action))]
        self.focus_bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.epoch),*self.focus_callbacks)
    def commit(self,owner,mode):
        self.events.append(('commit',mode))
        if self.on_commit:self.on_commit(mode)
        return self.commit_ok
    def action(self,owner,op,doc):
        self.events.append((op,doc))
        if self.on_action:self.on_action(op)
    def focus(self,doc='default'):
        return self.api.meltype_tsf_document_focus(self.doc if doc=='default' else doc,c.byref(self.focus_bridge))
    def ops(self):return [event[0] for event in self.events]
    def test_focus_without_composition_preserves_operation_order(self):
        self.assertEqual(self.focus(),0);self.assertEqual(self.ops(),list(range(6)))
        self.assertTrue(all(event[1]==self.doc for event in self.events));self.assertEqual(self.other.context_refs,1)
    def test_focus_composing_commits_synchronously_first(self):
        self.flags=1;self.focus()
        self.assertEqual(self.events[0],('commit',0));self.assertEqual(self.ops()[1:],list(range(6)))
    def test_focus_refused_sync_queues_caret_preserving_commit(self):
        self.flags=1;self.commit_ok=0;self.focus()
        self.assertEqual(self.events[:2],[('commit',0),('commit',1)])
        self.assertEqual(self.ops()[2:],list(range(6)))
    def test_focus_disabled_skips_hello_but_switches_edit_sink(self):
        self.flags=2;self.focus();self.assertEqual(self.ops(),list(range(5)))
    def test_focus_disabled_after_advise_skips_hello(self):
        self.on_action=lambda op:setattr(self,'flags',2) if op==4 else None
        self.focus();self.assertEqual(self.ops(),list(range(5)))
    def test_focus_null_document_still_unsubscribes_and_refreshes(self):
        self.focus(None);self.assertEqual(self.ops(),list(range(6)))
        self.assertTrue(all(event[1] is None for event in self.events))
    def test_focus_deactivated_owner_has_no_actions(self):
        self.flags=4;self.focus();self.assertEqual(self.events,[])
    def test_focus_deactivation_during_hide_stops_before_subscription(self):
        self.on_action=lambda op:setattr(self,'flags',4) if op==3 else None
        self.focus();self.assertEqual(self.ops(),[0,1,2,3])
    def nested(self):
        self.on_action=self.on_commit=None;self.flags=0
        self.assertEqual(self.focus(None),0)
    def test_focus_nested_during_failed_commit_skips_old_async_and_actions(self):
        self.flags=1;self.commit_ok=0;self.on_commit=lambda _:self.nested()
        self.focus();self.assertEqual(self.events,[('commit',0)]+[(op,None) for op in range(6)])
    def test_focus_nested_during_hide_does_not_restore_old_document(self):
        self.on_action=lambda op:self.nested() if op==3 else None
        self.focus();self.assertEqual(self.events,[(op,self.doc) for op in range(4)]+[(op,None) for op in range(6)])
    def test_focus_nested_during_advise_skips_old_hello(self):
        self.on_action=lambda op:self.nested() if op==4 else None
        self.focus();self.assertEqual(self.events,[(op,self.doc) for op in range(5)]+[(op,None) for op in range(6)])
    def test_focus_document_retained_when_external_reference_is_released(self):
        self.on_action=lambda op:self.other.context_ref(-1) if op==0 else None
        self.focus();self.assertEqual(self.other.context_refs,0);self.other.context_ref(1)
    def test_focus_owner_retained_when_original_reference_is_released(self):
        self.on_action=lambda op:self.host.context_ref(-1) if op==0 else None
        self.focus();self.assertEqual(self.host.context_refs,1);self.host.context_ref(1)
    def test_focus_foreground_loss_hides_then_forgets_consumed_keys(self):
        state=self.api.meltype_tsf_key_state_create()
        callbacks=[WOULD(lambda o,ctx,k,m,t,ch:1),MARK(lambda *a:None),COMPOSING(lambda _:0),TOGGLE(lambda *a:None),HANDLE(lambda *a:None)]
        entry=Entry(123,c.addressof(self.host.context),*callbacks)
        try:
            def event(kind):return self.api.meltype_tsf_key_event(state,kind,c.byref(self.host.context),65,0,c.byref(entry))
            self.assertEqual(event(1),1);self.assertEqual(event(2),1)
            self.assertEqual(self.api.meltype_tsf_foreground_focus(1,state,c.byref(self.focus_bridge)),0)
            self.assertEqual(self.events,[]);self.assertEqual(event(2),1)
            self.assertEqual(self.api.meltype_tsf_foreground_focus(0,state,c.byref(self.focus_bridge)),0)
            self.assertEqual(self.events,[(3,None)]);self.assertEqual(event(3),0)
        finally:self.api.meltype_tsf_key_state_destroy(state)
    def test_focus_invalid_epoch_has_no_effect(self):
        self.focus_bridge.epoch=None;self.assertEqual(self.focus(),HR(0x80070057).value)
        self.assertEqual(self.events,[])

if __name__=='__main__':
    suite=unittest.TestSuite(FocusTests(name) for name in unittest.defaultTestLoader.getTestCaseNames(FocusTests) if name.startswith('test_focus_'))
    raise SystemExit(not unittest.TextTestRunner(verbosity=2,failfast=True).run(suite).wasSuccessful())
