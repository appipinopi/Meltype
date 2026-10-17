# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Commit scheduler, stale targets and caret policy over private COM hosts."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_abandoned_smoke import DeferredHost, P, U32, HR, invoke, release, SYNCHRONOUS, ASYNC
from tsf_range_smoke import units, W
CURRENT=c.CFUNCTYPE(P,P,c.POINTER(P),c.POINTER(P))
UPDATE=c.CFUNCTYPE(None,P,U32,U32,HR)
REQUEST=c.CFUNCTYPE(None,P,c.c_char_p,U32,P,P)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('current',CURRENT),('update',UPDATE),('request_apply',REQUEST)]

class CommitTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_commit.argtypes=[U32,P,U32,U32,c.c_char_p,c.POINTER(Callbacks),HR]
        cls.api.meltype_tsf_commit.restype=HR
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32];cls.api.meltype_tsf_start_composition.restype=P
        cls.api.meltype_tsf_end_composition.argtypes=[P,P,U32]
        cls.api.meltype_tsf_write_range.argtypes=[P,P,U32,c.POINTER(W),U32,HR]

    def setUp(self):
        self.hosts=[DeferredHost(),DeferredHost()]
        self.compositions=[]
        for host in self.hosts:
            self.compositions.append(self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7))
            self.assertTrue(self.compositions[-1])
        self.hosts[0].mode='sync'
        self.selected=0;self.has_composition=True;self.sink=0;self.own=False;self.keep=False
        self.events,self.requests,self.errors=[],[],[]
        self.on_request=None;self.write_text=False
        def protect(method,fallback):
            def guarded(*args):
                try:return method(*args)
                except BaseException as error:self.errors.append(repr(error));return fallback
            return guarded
        self.callbacks=[CURRENT(protect(self.current,None)),UPDATE(protect(self.update,None)),REQUEST(protect(self.request,None))]
        self.bridge=Callbacks(123,c.addressof(self.hosts[0].context),*self.callbacks)
        self.metadata=json.dumps({'sid':'確定"😀'},ensure_ascii=False).encode()

    def tearDown(self):
        for host in self.hosts:host.cancel()
        for host,composition in zip(self.hosts,self.compositions):release(composition);host.verify(self)
        self.assertEqual(self.errors,[])

    def current(self,owner,context,sink):
        self.assertEqual(owner,123)
        context[0]=c.addressof(self.hosts[self.selected].context)
        sink[0]=c.addressof(self.hosts[self.sink].context) if self.sink is not None else None
        return self.compositions[self.selected] if self.has_composition else None

    def update(self,owner,operation,cookie,value):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.hosts[0].context_refs,2)
        self.events.append((operation,cookie,value))
        if operation==0:self.own=bool(value)
        elif operation==1:self.keep=bool(value)
        elif operation==2:
            self.assertFalse(self.keep)
            self.api.meltype_tsf_end_composition(c.byref(self.hosts[self.selected].context),self.compositions[self.selected],cookie)
            self.has_composition=False
        elif operation==4:self.own=False

    def request(self,owner,text,cookie,context,target):
        self.assertEqual(owner,123);self.assertEqual(cookie,7)
        host=self.hosts[self.selected]
        self.assertEqual(context,c.addressof(host.context));self.assertEqual(target,self.compositions[self.selected])
        self.assertGreaterEqual(host.composition_refs,2)
        self.assertGreaterEqual(self.hosts[0].context_refs,2)
        self.requests.append(json.loads(text))
        if self.on_request:self.on_request()
        if self.write_text:
            text=units('日本😀');buffer=(W*len(text))(*text)
            self.assertTrue(self.api.meltype_tsf_write_range(context,host.current_range,cookie,buffer,len(text),int(self.keep)))

    def call(self,mode=0,keep=True,context=0):
        return self.api.meltype_tsf_commit(mode,c.byref(self.hosts[context].context),17,7,self.metadata,c.byref(self.bridge),int(keep))

    def test_sync_request_moved_and_cleanup_order(self):
        self.assertEqual(self.call(),0)
        self.assertEqual(self.requests,[{'op':'commit','sid':'確定"😀','moved':True}])
        self.assertEqual(self.events,[(0,7,1),(1,7,1),(1,7,0),(2,7,0),(3,7,0),(4,0,0)])
        self.assertEqual(self.hosts[0].requests,[(17,7)])
        self.assertFalse(self.keep);self.assertFalse(self.own);self.assertEqual(self.hosts[0].ended,1)

    def test_existing_session_keeps_own_mark_until_edit_notification(self):
        self.assertEqual(self.call(2,False),0)
        self.assertTrue(self.own);self.assertFalse(self.keep)
        self.assertNotIn('moved',self.requests[0]);self.assertEqual(self.hosts[0].requests,[])

    def test_caret_is_preserved_for_click_and_moves_for_normal_commit(self):
        self.write_text=True;self.hosts[0].selection=(1,1)
        self.assertEqual(self.call(2,True),0)
        self.assertEqual(self.hosts[0].selection,(1,1))
        self.has_composition=True;self.hosts[0].selection=(1,1)
        self.assertEqual(self.call(2,False),0)
        self.assertEqual(self.hosts[0].selection,(4,4))

    def test_old_context_does_not_mark_visible_context_own_edit(self):
        self.sink=1
        self.assertEqual(self.call(),0)
        self.assertNotIn((0,7,1),self.events);self.assertFalse(self.own)

    def test_existing_wrong_context_only_hides_candidates(self):
        self.assertEqual(self.call(2,context=1),0)
        self.assertEqual(self.events,[(3,7,0)]);self.assertEqual(self.requests,[])
        self.assertEqual(self.hosts[0].ended,0)

    def test_no_composition_is_success_without_scheduling(self):
        self.has_composition=False
        self.assertEqual(self.call(),0);self.assertEqual(self.call(1),0)
        self.assertEqual(self.events,[]);self.assertEqual(self.hosts[0].requests,[])
        self.call(2);self.assertEqual(self.events,[(3,7,0)])

    def test_refused_sync_request_clears_own_without_committing(self):
        self.own=True;self.hosts[0].mode='reject'
        self.assertEqual(self.call(),SYNCHRONOUS)
        self.assertEqual(self.events,[(4,0,0)]);self.assertEqual(self.requests,[]);self.assertFalse(self.own)

    def test_async_retains_owner_context_target_until_execution(self):
        host=self.hosts[0];host.mode='async'
        self.assertEqual(self.call(1),ASYNC)
        self.assertEqual(self.events,[]);self.assertEqual(host.context_refs,3);self.assertEqual(host.composition_refs,2)
        host.flush()
        self.assertEqual(len(self.requests),1);self.assertEqual(host.context_refs,1);self.assertEqual(host.composition_refs,1)
        self.assertEqual(host.requests,[(17,6)])

    def test_async_cancel_releases_all_refs_without_editing(self):
        host=self.hosts[0];host.mode='async';self.call(1);host.cancel()
        self.assertEqual(self.events,[]);self.assertEqual(self.requests,[])
        self.assertEqual(host.context_refs,1);self.assertEqual(host.composition_refs,1)

    def test_async_replacement_is_not_ended_or_hidden_by_old_request(self):
        host=self.hosts[0];host.mode='async';self.call(1);self.selected=1;host.flush()
        self.assertEqual(self.events,[]);self.assertEqual(self.requests,[])
        self.assertEqual(self.hosts[1].ended,0)

    def test_replacement_during_request_is_not_ended(self):
        self.on_request=lambda:setattr(self,'selected',1)
        self.assertEqual(self.call(),0)
        self.assertNotIn((2,7,0),self.events);self.assertEqual(self.hosts[1].ended,0)
        self.assertFalse(self.keep)

    def test_sync_incorrectly_deferred_cannot_execute_after_return(self):
        host=self.hosts[0];host.mode='async'
        self.assertEqual(self.call(),HR(0x80004004).value)
        events=list(self.events)
        self.assertEqual(invoke(host.pending[0],3,HR,[U32],7),HR(0x80004004).value)
        self.assertEqual(self.events,events);self.assertEqual(self.requests,[])
        host.cancel()

    def test_owner_survives_original_reference_release_during_request(self):
        self.on_request=lambda:self.hosts[0].context_ref(-1)
        self.assertEqual(self.call(),0);self.assertEqual(self.hosts[0].context_refs,0)
        self.hosts[0].context_ref(1)

    def test_invalid_mode_and_metadata_do_nothing(self):
        self.assertEqual(self.call(3),HR(0x80070057).value)
        self.metadata=b'{"sid":false}'
        self.assertEqual(self.call(),HR(0x80070057).value)
        self.assertEqual(self.events,[]);self.assertEqual(self.requests,[])

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
