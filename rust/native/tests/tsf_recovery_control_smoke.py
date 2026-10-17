# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Recovery orchestration over real native snapshots and controlled edit locks."""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_termination_smoke import TerminationTests, P, U32, HR, REQUEST, units
from tsf_com_smoke import invoke, release
EDIT_CONTEXT=c.CFUNCTYPE(P,P)
UPDATE=c.CFUNCTYPE(None,P,U32,HR)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('composition',c.POINTER(P)),('context',c.POINTER(P)),('resume',c.POINTER(P)),('sink',P),
              ('edit_context',EDIT_CONTEXT),('update',UPDATE),('request',REQUEST)]
class RecoveryControlTests(TerminationTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_resume_or_commit.argtypes=[P,U32,U32,c.c_char_p,c.POINTER(Callbacks)]
    def setUp(self):
        super().setUp()
        self.termination_bridge=self.bridge
        self.notify();self.assertTrue(self.resume.value)
        self.host.selection=(3,3);self.host.mode='sync'
        self.events.clear();self.requests.clear();self.own=False;self.visible=True
        self.on_recovery_update=None
        self.recovery_callbacks=[EDIT_CONTEXT(lambda _:c.addressof(self.host.context) if self.visible else None),
            UPDATE(self.recovery_update),REQUEST(self.request)]
        self.recovery_bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.composition),c.pointer(self.context),c.pointer(self.resume),c.addressof(self.host.sink),*self.recovery_callbacks)
    def update(self,owner,op,value=None):
        if value is None:return super().update(owner,op)
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,2)
        self.events.append((op,value))
        if op==0:self.own=bool(value)
        if self.on_recovery_update:self.on_recovery_update(op,value)
    def recovery_update(self,*args):
        try:self.update(*args)
        except BaseException as error:self.errors.append(repr(error))
    def recover(self,host=None):
        return self.api.meltype_tsf_resume_or_commit(c.byref((host or self.host).context),17,11,self.metadata,c.byref(self.recovery_bridge))
    def test_recovery_control_resumes_without_reinserting_and_styles_after_adoption(self):
        self.assertEqual(self.recover(),1)
        self.assertFalse(self.resume.value);self.assertEqual(self.composition.value,c.addressof(self.host.composition))
        self.assertEqual(self.context.value,c.addressof(self.host.context));self.assertEqual(self.host.document,units('old'))
        self.assertEqual(self.host.writes,[]);self.assertEqual(self.requests,[])
        self.assertEqual(self.events,[(0,1),(0,0)]);self.assertEqual(self.host.requests,[(17,7)])
        self.assertEqual(self.host.attributes,[(0,3,11)])
    def test_recovery_control_no_snapshot_has_no_effect(self):
        self.api.meltype_tsf_destroy_resume(self.resume);self.resume.value=None
        self.assertEqual(self.recover(),0);self.assertEqual(self.events,[]);self.assertEqual(self.host.requests,[])
    def test_recovery_control_changed_text_requests_moved_commit_without_writing(self):
        self.host.document=units('new')
        self.assertEqual(self.recover(),2);self.assertFalse(self.resume.value);self.assertFalse(self.composition.value)
        self.assertEqual(self.requests,[{'op':'commit','sid':'通知"😀','moved':True}]);self.assertEqual(self.host.writes,[])
    def test_recovery_control_changed_or_selected_caret_does_not_resume(self):
        self.host.selection=(1,2)
        self.assertEqual(self.recover(),2);self.assertFalse(self.composition.value);self.assertEqual(self.host.writes,[])
    def test_recovery_control_different_context_does_not_query_saved_field(self):
        self.assertEqual(self.recover(self.other),2);self.assertEqual(self.host.requests,[])
        self.assertFalse(self.resume.value);self.assertEqual(len(self.requests),1)
    def test_recovery_control_null_context_commits_without_lock(self):
        result=self.api.meltype_tsf_resume_or_commit(None,17,11,self.metadata,c.byref(self.recovery_bridge))
        self.assertEqual(result,2);self.assertEqual(self.host.requests,[]);self.assertFalse(self.resume.value)
    def test_recovery_control_refused_edit_discards_snapshot_and_clears_own(self):
        self.host.mode='reject';self.own=True
        self.assertEqual(self.recover(),2);self.assertFalse(self.own);self.assertFalse(self.resume.value)
        self.assertEqual(self.events,[(0,0),(1,0)])
    def test_recovery_control_deferred_sync_work_cannot_mutate_after_return(self):
        self.host.mode='async'
        self.assertEqual(self.recover(),2);self.assertFalse(self.resume.value)
        events=list(self.events)
        self.assertEqual(invoke(self.host.pending[0],3,HR,[U32],7),HR(0x80004004).value)
        self.assertEqual(self.events,events);self.assertFalse(self.composition.value)
        self.host.cancel()
    def test_recovery_control_failed_start_requests_fallback_without_text_write(self):
        self.host.fail_start=True
        self.assertEqual(self.recover(),2);self.assertFalse(self.composition.value);self.assertEqual(self.host.writes,[])
    def test_recovery_control_old_context_does_not_set_visible_own_edit(self):
        self.visible=False
        self.assertEqual(self.recover(),1);self.assertEqual(self.events,[(0,0)])
    def test_recovery_control_reentrant_start_preserves_new_input_and_skips_fallback(self):
        original=self.host.start
        def start(*args):
            result=original(*args);self.replacement();return result
        self.host.bind(self.host.manager_table,3,HR,[U32,P,P,c.POINTER(P)],start)
        self.assertEqual(self.recover(),0);self.assertEqual(self.composition.value,c.addressof(self.other.composition))
        self.assertEqual(self.requests,[]);self.assertEqual(self.other.ended,0)
    def test_recovery_control_termination_during_styling_preserves_new_resume(self):
        original=self.host.set_value
        def style(*args):
            self.assertEqual(self.composition.value,c.addressof(self.host.composition))
            result=original(*args)
            self.assertEqual(self.api.meltype_tsf_composition_terminated(7,self.composition,self.metadata,c.byref(self.termination_bridge)),0)
            return result
        self.host.bind(self.host.property_table,9,HR,[U32,P,P],style)
        self.assertEqual(self.recover(),1);self.assertFalse(self.composition.value);self.assertTrue(self.resume.value)
        self.assertEqual(self.requests,[])
    def test_recovery_control_snapshot_survives_owner_original_release_during_read(self):
        self.host.on_read=lambda:self.host.context_ref(-1)
        self.assertEqual(self.recover(),1);self.assertEqual(self.host.context_refs,1)
        self.host.context_ref(1)
    def test_recovery_control_invalid_metadata_does_not_consume_snapshot(self):
        token=self.resume.value;self.metadata=b'{"sid":false}'
        self.assertEqual(self.recover(),-1);self.assertEqual(self.resume.value,token);self.assertEqual(self.events,[])

if __name__=='__main__':
    suite=unittest.TestSuite(RecoveryControlTests(name) for name in dir(RecoveryControlTests) if name.startswith('test_recovery_control_'))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
