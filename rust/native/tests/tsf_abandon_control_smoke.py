# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Abandon old input without writing its document or resetting replacement input."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_termination_smoke import TerminationTests, P, U32, UPDATE, REQUEST
from tsf_range_smoke import units
from tsf_com_smoke import release

class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('composition',c.POINTER(P)),('context',c.POINTER(P)),
              ('abandoned',c.POINTER(P)),('update',UPDATE),('request',REQUEST)]

class AbandonControlTests(TerminationTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_abandon_composition.argtypes=[U32,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_abandon_composition.restype=c.c_int
    def setUp(self):
        super().setUp()
        self.queue_slot=P(self.queue)
        self.bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.composition),c.pointer(self.context),c.pointer(self.queue_slot),*self.callbacks[1:])
    def tearDown(self):
        self.queue=self.queue_slot.value
        super().tearDown()
    def abandon(self):
        return self.api.meltype_tsf_abandon_composition(17,self.metadata,c.byref(self.bridge))
    def update(self,owner,op):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,1)
        self.events.append(op)
        if self.on_update:self.on_update(op)
    def request(self,owner,text):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,1)
        self.requests.append(json.loads(text))
    def test_abandon_control_detaches_and_schedules_old_field_without_inserting(self):
        self.assertEqual(self.abandon(),1)
        self.assertFalse(self.composition.value);self.assertFalse(self.context.value)
        self.assertEqual(self.events,[0,1,2]);self.assertEqual(len(self.host.pending),1)
        self.assertEqual(self.requests,[{'op':'commit','sid':'通知"😀','moved':True}])
        self.assertEqual(self.host.ended,0)
        self.host.flush();self.assertEqual(self.host.ended,1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.writes,[])
    def test_abandon_control_sync_cleanup_sees_slots_detached(self):
        self.host.mode='sync'
        def detached():
            self.assertFalse(self.composition.value);self.assertFalse(self.context.value)
        self.host.on_clear=detached
        self.assertEqual(self.abandon(),1);self.assertEqual(self.host.ended,1)
    def test_abandon_control_refused_cleanup_retains_item_for_later(self):
        self.host.mode='reject';self.assertEqual(self.abandon(),1)
        self.assertEqual(self.host.ended,0)
        self.assertEqual(self.api.meltype_tsf_abandoned_forget(self.queue_slot,self.target),1)
    def test_abandon_control_creates_missing_queue(self):
        self.api.meltype_tsf_abandoned_destroy(self.queue_slot);self.queue_slot.value=None
        self.assertEqual(self.abandon(),1);self.assertTrue(self.queue_slot.value)
        self.host.flush();self.assertEqual(self.host.ended,1)
    def test_abandon_control_no_target_still_notifies_core(self):
        self.clear();self.assertEqual(self.abandon(),1)
        self.assertEqual(self.host.pending,[]);self.assertEqual(self.events,[0,1,2])
    def test_abandon_control_composition_without_context_is_released(self):
        release(self.context);self.context.value=None
        self.assertEqual(self.abandon(),1);self.assertFalse(self.composition.value)
        self.assertEqual(self.host.pending,[])
    def test_abandon_control_context_without_composition_is_released(self):
        release(self.composition);self.composition.value=None
        self.assertEqual(self.abandon(),1);self.assertFalse(self.context.value)
        self.assertEqual(self.host.pending,[])
    def test_abandon_control_replacement_during_sync_cleanup_is_preserved(self):
        self.host.mode='sync';self.host.on_clear=self.replacement
        self.assertEqual(self.abandon(),0);self.assertEqual(self.events,[]);self.assertEqual(self.requests,[])
        self.assertEqual(self.composition.value,c.addressof(self.other.composition))
    def test_abandon_control_replacement_during_reset_skips_hide_and_commit(self):
        self.on_update=lambda op:self.replacement() if op==0 else None
        self.assertEqual(self.abandon(),0);self.assertEqual(self.events,[0]);self.assertEqual(self.requests,[])
    def test_abandon_control_replacement_during_hide_skips_moved_and_commit(self):
        self.on_update=lambda op:self.replacement() if op==1 else None
        self.assertEqual(self.abandon(),0);self.assertEqual(self.events,[0,1]);self.assertEqual(self.requests,[])
    def test_abandon_control_replacement_during_moved_update_skips_commit(self):
        self.on_update=lambda op:self.replacement() if op==2 else None
        self.assertEqual(self.abandon(),0);self.assertEqual(self.events,[0,1,2]);self.assertEqual(self.requests,[])
    def test_abandon_control_owner_survives_original_release(self):
        self.host.mode='sync'
        self.host.on_clear=lambda:self.host.context_ref(-1)
        self.assertEqual(self.abandon(),1);self.assertEqual(self.host.context_refs,0)
        self.host.context_ref(1)
    def test_abandon_control_invalid_metadata_preserves_owned_pair(self):
        self.metadata=b'{"sid":false}'
        self.assertEqual(self.abandon(),-1);self.assertEqual(self.composition.value,self.target)
        self.assertEqual(self.events,[]);self.assertEqual(self.host.pending,[])
    def test_abandon_control_aliased_slots_rejected_without_changes(self):
        self.bridge.context=self.bridge.composition
        self.assertEqual(self.abandon(),-1);self.assertEqual(self.composition.value,self.target)

if __name__=='__main__':
    suite=unittest.TestSuite(AbandonControlTests(name) for name in unittest.defaultTestLoader.getTestCaseNames(AbandonControlTests) if name.startswith('test_abandon_control_'))
    result=unittest.TextTestRunner(verbosity=2,failfast=True).run(suite)
    raise SystemExit(not result.wasSuccessful())
