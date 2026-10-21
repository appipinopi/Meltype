# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Host termination: identity, resume capture and replacement-safe detachment."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_abandoned_smoke import DeferredHost, P, U32, HR, release
from tsf_range_smoke import units
READ=c.CFUNCTYPE(U32,P)
UPDATE=c.CFUNCTYPE(None,P,U32)
REQUEST=c.CFUNCTYPE(None,P,c.c_char_p)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('composition',c.POINTER(P)),('context',c.POINTER(P)),
              ('resume',c.POINTER(P)),('abandoned',P),('read',READ),('update',UPDATE),('request',REQUEST)]
class Host(DeferredHost):
    def __init__(self):
        self.on_clear=self.on_read=None
        super().__init__()
    def clear(self,*args):
        result=super().clear(*args)
        if self.on_clear:
            callback,self.on_clear=self.on_clear,None
            callback()
        return result
    def read(self,*args):
        if self.on_read:
            callback,self.on_read=self.on_read,None
            callback()
        return super().read(*args)
class TerminationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_composition_terminated.argtypes=[U32,P,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_composition_terminated.restype=HR
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32];cls.api.meltype_tsf_start_composition.restype=P
        cls.api.meltype_tsf_capture_resume.argtypes=[P,P,U32];cls.api.meltype_tsf_capture_resume.restype=P
        cls.api.meltype_tsf_destroy_resume.argtypes=[P]
        cls.api.meltype_tsf_resume_matches.argtypes=[P,P]
        cls.api.meltype_tsf_abandoned_create.restype=P
        cls.api.meltype_tsf_abandoned_destroy.argtypes=[P]
        cls.api.meltype_tsf_abandoned_add.argtypes=[P,P,P,P,U32]
        cls.api.meltype_tsf_abandoned_forget.argtypes=[P,P]
    def setUp(self):
        self.host,self.other=Host(),Host()
        self.host.selection=(0,3)
        self.composition=P(self.start(self.host));self.target=self.composition.value
        self.host.context_ref(1);self.context=P(c.addressof(self.host.context));self.resume=P()
        self.other_composition=None
        self.queue=self.api.meltype_tsf_abandoned_create()
        self.flags=0;self.events,self.requests,self.errors=[],[],[]
        self.on_update=None
        def protect(method,fallback):
            def guarded(*args):
                try:return method(*args)
                except BaseException as error:self.errors.append(repr(error));return fallback
            return guarded
        self.callbacks=[READ(protect(lambda _:self.flags,1)),UPDATE(protect(self.update,None)),REQUEST(protect(self.request,None))]
        self.bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.composition),c.pointer(self.context),c.pointer(self.resume),self.queue,*self.callbacks)
        self.metadata=json.dumps({'sid':'通知"😀'},ensure_ascii=False).encode()
    def tearDown(self):
        self.host.cancel();self.other.cancel();self.api.meltype_tsf_abandoned_destroy(self.queue)
        self.clear()
        if self.other_composition:release(self.other_composition)
        self.host.verify(self);self.other.verify(self);self.assertEqual(self.errors,[])
    def start(self,host):
        result=self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7)
        self.assertTrue(result);return result
    def clear(self):
        comp,ctx,resume=self.composition.value,self.context.value,self.resume.value
        self.composition.value=self.context.value=self.resume.value=None
        if resume:self.api.meltype_tsf_destroy_resume(resume)
        if comp:release(comp)
        if ctx:release(ctx)
    def replacement(self):
        self.clear();self.composition.value=self.start(self.other)
        self.other.context_ref(1);self.context.value=c.addressof(self.other.context)
    def update(self,owner,op):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,2)
        self.events.append(op)
        if op==1:self.flags&=~4
        if self.on_update:self.on_update(op)
    def request(self,owner,text):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,2)
        self.requests.append(json.loads(text))
    def notify(self,target=None):
        return self.api.meltype_tsf_composition_terminated(7,target or self.target,self.metadata,c.byref(self.bridge))
    def test_termination_captures_preedit_without_committing_or_reinserting(self):
        self.assertEqual(self.notify(),0)
        self.assertFalse(self.composition.value);self.assertFalse(self.context.value);self.assertTrue(self.resume.value)
        self.assertEqual(self.events,[0,1]);self.assertEqual(self.requests,[])
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.writes,[])
        self.assertTrue(self.api.meltype_tsf_resume_matches(self.resume,c.byref(self.host.context)))
    def test_termination_converting_input_commits_moved_without_applying_reply(self):
        self.flags=4;self.assertEqual(self.notify(),0)
        self.assertFalse(self.resume.value);self.assertEqual(self.events,[0,1,2])
        self.assertEqual(self.requests,[{'op':'commit','sid':'通知"😀','moved':True}])
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.ended,0)
    def test_termination_disabled_input_neither_captures_nor_contacts_core(self):
        self.flags=1;self.notify()
        self.assertFalse(self.resume.value);self.assertEqual(self.requests,[]);self.assertEqual(self.events,[0,1,2])
    def test_termination_deactivated_input_does_not_capture(self):
        self.flags=2;self.notify()
        self.assertFalse(self.resume.value);self.assertEqual(self.events,[0,1,2])
    def test_termination_empty_or_64_unit_text_is_not_captured(self):
        for text in ['', 'あ'*64]:
            if not self.composition.value:
                self.composition.value=self.start(self.host);self.host.context_ref(1);self.context.value=c.addressof(self.host.context)
                self.target=self.composition.value
            self.host.document=units(text);self.host.ranges[self.host.current_range][:2]=[0,len(self.host.document)]
            self.notify();self.assertFalse(self.resume.value)
        self.assertEqual(len(self.requests),2)
    def test_termination_failed_text_read_does_not_capture_or_edit(self):
        self.host.fail_read=True;self.notify()
        self.assertFalse(self.resume.value);self.assertEqual(self.host.writes,[]);self.assertEqual(len(self.requests),1)
    def test_termination_alias_identity_matches_current_composition(self):
        self.assertEqual(self.notify(c.addressof(self.host.alias)),0)
        self.assertTrue(self.resume.value);self.assertFalse(self.composition.value)
    def test_termination_old_target_only_clears_attribute_and_forgets_queue(self):
        self.other_composition=self.start(self.other)
        self.api.meltype_tsf_abandoned_add(self.queue,c.byref(self.other.context),self.other_composition,c.byref(self.host.context),17)
        self.assertEqual(self.notify(self.other_composition),0)
        self.assertEqual(self.events,[]);self.assertEqual(self.requests,[]);self.assertEqual(self.composition.value,self.target)
        self.assertEqual(self.api.meltype_tsf_abandoned_forget(self.queue,self.other_composition),0)
        self.assertGreaterEqual(self.other.clears,1)
    def test_termination_replacement_during_attribute_clear_is_preserved(self):
        self.host.on_clear=self.replacement;self.notify()
        self.assertEqual(self.composition.value,c.addressof(self.other.composition));self.assertEqual(self.events,[])
        self.assertFalse(self.resume.value);self.assertEqual(self.requests,[])
    def test_termination_replacement_during_capture_is_not_detached(self):
        self.host.on_read=self.replacement;self.notify()
        self.assertEqual(self.composition.value,c.addressof(self.other.composition));self.assertFalse(self.resume.value)
        self.assertEqual(self.events,[]);self.assertEqual(self.requests,[])
    def test_termination_replacement_during_hide_stops_old_state_updates(self):
        self.flags=4
        self.on_update=lambda op:self.replacement() if op==0 else None
        self.notify();self.assertEqual(self.events,[0]);self.assertEqual(self.requests,[])
        self.assertEqual(self.composition.value,c.addressof(self.other.composition))
    def test_termination_old_resume_is_destroyed_before_replacement_capture(self):
        old=self.api.meltype_tsf_capture_resume(c.byref(self.host.context),self.target,7)
        self.assertTrue(old);self.resume.value=old;self.notify()
        self.assertTrue(self.resume.value);self.assertNotEqual(self.resume.value,old)
        self.assertFalse(self.api.meltype_tsf_resume_matches(old,c.byref(self.host.context)))
    def test_termination_owner_survives_original_reference_release_during_capture(self):
        self.host.on_read=lambda:self.host.context_ref(-1)
        self.notify();self.assertTrue(self.resume.value)
        self.assertEqual(self.host.context_refs,1);self.host.context_ref(1)
    def test_termination_invalid_metadata_leaves_current_state_untouched(self):
        self.metadata=b'{"sid":false}'
        self.assertEqual(self.notify(),HR(0x80070057).value)
        self.assertEqual(self.composition.value,self.target);self.assertEqual(self.events,[])

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
