# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Ordered shutdown, refusal/deferred edits, subscriptions and reactivation."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_termination_smoke import TerminationTests, READ, P, U32, HR
from tsf_subscriptions_smoke import SubscriptionHost
from tsf_com_smoke import release, invoke
from tsf_focus_smoke import Callbacks as FocusCallbacks, COMMIT as FocusCommit, ACTION as FocusAction
from tsf_range_smoke import units
ACTION=c.CFUNCTYPE(None,P,U32,U32,P)
REQUEST=c.CFUNCTYPE(None,P,c.c_char_p,U32,P,P)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('phase',c.POINTER(U32)),
              ('composition',c.POINTER(P)),('context',c.POINTER(P)),('manager',c.POINTER(P)),
              ('mode',c.POINTER(P)),('keys',c.POINTER(P)),('thread',c.POINTER(P)),
              ('read',READ),('action',ACTION),('request',REQUEST)]
class ShutdownTests(TerminationTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_shutdown.argtypes=[U32,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_shutdown.restype=HR
        cls.api.meltype_tsf_end_composition.argtypes=[P,P,U32]
        cls.api.meltype_tsf_subscribe.argtypes=[U32,P,U32,P,c.POINTER(P)]
        cls.api.meltype_tsf_subscribe.restype=P
        cls.api.meltype_tsf_unsubscribe.argtypes=[P]
        cls.api.meltype_tsf_document_focus.argtypes=[P,c.POINTER(FocusCallbacks)]
    def setUp(self):
        super().setUp()
        self.host.mode='sync'
        self.epoch=c.c_uint64();self.phase=U32()
        self.other.context_ref(1);self.manager=P(c.addressof(self.other.context))
        self.mode,self.keys,self.thread=P(),P(),P()
        self.subhost=SubscriptionHost()
        self.on_action=self.on_request=None;self.deactivated=False;self.managers=[]
        def guard(fn,default=None):
            def run(*args):
                try:
                    self.assertEqual(args[0],123);self.assertGreaterEqual(self.host.context_refs,1)
                    return fn(*args)
                except BaseException as e:self.errors.append(repr(e));return default
            return run
        self.shutdown_callbacks=[READ(guard(lambda _:self.flags,1)),ACTION(guard(self.action)),REQUEST(guard(self.request))]
        self.shutdown_bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.epoch),c.pointer(self.phase),
            c.pointer(self.composition),c.pointer(self.context),c.pointer(self.manager),c.pointer(self.mode),c.pointer(self.keys),c.pointer(self.thread),*self.shutdown_callbacks)
    def tearDown(self):
        for slot in [self.mode,self.keys,self.thread]:
            self.api.meltype_tsf_unsubscribe(slot);slot.value=None
        if self.manager.value:release(self.manager);self.manager.value=None
        self.subhost.verify(self)
        super().tearDown()
    def action(self,owner,op,cookie,manager):
        self.events.append(op)
        if op==0:self.deactivated=True
        if op==1 and self.resume.value:self.api.meltype_tsf_destroy_resume(self.resume);self.resume.value=None
        if op==7:
            self.managers.append(manager)
            if manager:self.assertGreaterEqual(self.other.context_refs,3)
        if op==9:
            self.assertEqual(cookie,7)
            self.api.meltype_tsf_end_composition(self.context,self.composition,cookie)
            self.clear()
        if self.on_action:self.on_action(op)
    def request(self,owner,text,cookie,context,target):
        request=json.loads(text);self.requests.append(request)
        if request['op']=='commit':
            self.assertEqual((cookie,context,target),(7,self.context.value,self.composition.value))
            self.assertEqual(self.phase.value,1)
        else:self.assertEqual((cookie,context,target),(0,None,None))
        if self.on_request:self.on_request(request['op'])
    def shutdown(self):return self.api.meltype_tsf_shutdown(17,self.metadata,c.byref(self.shutdown_bridge))
    def reactivate(self):
        self.epoch.value+=1;self.phase.value=0;self.deactivated=False;self.replacement()
    def subscribe(self):
        for kind,slot in [(2,self.mode),(1,self.keys),(0,self.thread)]:
            slot.value=self.api.meltype_tsf_subscribe(kind,c.byref(self.subhost.context),17,c.byref(self.subhost.sink),None)
            self.assertTrue(slot.value)
    def test_shutdown_commit_close_then_cleanup_order(self):
        self.assertEqual(self.shutdown(),0)
        self.assertEqual(self.events,[9]+list(range(9)));self.assertTrue(self.deactivated)
        self.assertEqual([r['op'] for r in self.requests],['commit','close'])
        self.assertEqual(self.requests[0],{'op':'commit','sid':'通知"😀','moved':True})
        self.assertEqual(self.requests[1],{'op':'close','sid':'通知"😀'})
        self.assertEqual(self.phase.value,2);self.assertFalse(self.manager.value)
        self.assertFalse(self.context.value);self.assertFalse(self.composition.value)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.writes,[])
    def test_shutdown_disabled_skips_close_notification(self):
        self.flags=1;self.shutdown();self.assertEqual([r['op'] for r in self.requests],['commit'])
        self.assertEqual(self.phase.value,2)
    def test_shutdown_no_composition_still_closes_and_releases(self):
        self.clear();self.shutdown();self.assertEqual(self.events,list(range(9)))
        self.assertEqual([r['op'] for r in self.requests],['close'])
    def test_shutdown_refused_edit_continues_cleanup_without_writing(self):
        self.host.mode='reject';self.shutdown()
        self.assertEqual(self.host.ended,0);self.assertEqual(self.events,list(range(9)))
        self.assertEqual([r['op'] for r in self.requests],['close']);self.assertFalse(self.composition.value)
    def test_shutdown_deferred_sync_cannot_edit_after_return(self):
        self.host.mode='async';self.shutdown();before=(list(self.events),list(self.requests))
        self.assertEqual(len(self.host.pending),1)
        self.assertEqual(invoke(self.host.pending[0],3,HR,[U32],7),HR(0x80004004).value)
        self.assertEqual((self.events,self.requests),before);self.assertEqual(self.host.writes,[])
        self.host.cancel()
    def test_shutdown_repeated_shutdown_is_inert(self):
        self.shutdown();before=(list(self.events),list(self.requests),self.epoch.value)
        self.assertEqual(self.shutdown(),0);self.assertEqual((self.events,self.requests,self.epoch.value),before)
    def test_shutdown_nested_during_commit_is_inert(self):
        self.on_request=lambda op:self.assertEqual(self.shutdown(),0) if op=='commit' else None
        self.shutdown();self.assertEqual(self.events,[9]+list(range(9)))
        self.assertEqual(len(self.requests),2)
    def test_shutdown_reactivation_during_commit_preserves_new_input(self):
        self.on_request=lambda op:self.reactivate() if op=='commit' else None
        self.shutdown();self.assertEqual(self.events,[]);self.assertEqual(len(self.requests),1)
        self.assertEqual(self.phase.value,0);self.assertEqual(self.other.ended,0)
        self.assertEqual(self.composition.value,c.addressof(self.other.composition))
    def test_shutdown_reactivation_during_close_stops_cleanup(self):
        self.on_request=lambda op:self.reactivate() if op=='close' else None
        self.shutdown();self.assertEqual(self.events,[9]);self.assertEqual(self.phase.value,0)
        self.assertTrue(self.manager.value);self.assertTrue(self.composition.value)
    def test_shutdown_reactivation_during_ui_teardown_preserves_new_input(self):
        self.on_action=lambda op:self.reactivate() if op==5 else None
        self.shutdown();self.assertEqual(self.events,[9]+list(range(6)))
        self.assertEqual(self.phase.value,0);self.assertTrue(self.manager.value)
    def test_shutdown_null_manager_is_supported(self):
        release(self.manager);self.manager.value=None
        self.shutdown();self.assertEqual(self.managers,[None]);self.assertEqual(self.phase.value,2)
    def test_shutdown_context_without_composition_is_released(self):
        release(self.composition);self.composition.value=None
        self.shutdown();self.assertFalse(self.context.value);self.assertEqual(self.host.requests,[])
    def test_shutdown_composition_without_context_is_released(self):
        release(self.context);self.context.value=None
        self.shutdown();self.assertFalse(self.composition.value);self.assertEqual(self.host.requests,[])
    def test_shutdown_invalid_metadata_preserves_everything(self):
        self.metadata=b'{"sid":false}'
        self.assertEqual(self.shutdown(),HR(0x80070057).value)
        self.assertEqual(self.phase.value,0);self.assertTrue(self.manager.value);self.assertEqual(self.events,[])
    def test_shutdown_aliased_slots_are_rejected(self):
        self.shutdown_bridge.manager=self.shutdown_bridge.context
        self.assertEqual(self.shutdown(),HR(0x80070057).value);self.assertEqual(self.phase.value,0)
    def test_shutdown_unsubscribes_mode_keys_thread_after_detaching_each_slot(self):
        self.subscribe();seen=[]
        self.subhost.sources[2].on_unadvise=lambda:seen.append((self.mode.value,bool(self.keys.value),bool(self.thread.value)))
        self.subhost.sources[0].on_unadvise=lambda:seen.append((self.thread.value,self.keys.value))
        self.shutdown();self.assertEqual(seen,[(None,True,True),(None,None)])
        self.assertEqual(self.subhost.key_unadvised,[17]);self.subhost.verify(self)
    def test_shutdown_reactivation_during_unsubscribe_preserves_remaining_tokens(self):
        self.subscribe();self.subhost.sources[2].on_unadvise=self.reactivate
        self.shutdown();self.assertFalse(self.mode.value)
        self.assertTrue(self.keys.value);self.assertTrue(self.thread.value);self.assertTrue(self.manager.value)
        self.assertEqual(self.phase.value,0)
        self.subhost.sources[2].on_unadvise=None
    def test_shutdown_owner_survives_original_release_during_close(self):
        self.on_request=lambda op:self.host.context_ref(-1) if op=='close' else None
        self.shutdown();self.assertEqual(self.host.context_refs,0);self.host.context_ref(1)
    def test_shutdown_focus_notification_while_stopping_does_not_cancel_shutdown(self):
        callbacks=[READ(lambda _:4),FocusCommit(lambda *a:0),FocusAction(lambda *a:self.errors.append('focus acted while stopping'))]
        bridge=FocusCallbacks(123,c.addressof(self.host.context),c.pointer(self.epoch),*callbacks)
        def focus(op):
            before=self.epoch.value
            self.assertEqual(self.api.meltype_tsf_document_focus(None,c.byref(bridge)),0)
            self.assertEqual(self.epoch.value,before)
        self.on_request=focus;self.shutdown();self.assertEqual(self.phase.value,2)
    def test_shutdown_replacement_during_commit_is_not_ended_in_old_edit(self):
        self.on_request=lambda op:self.replacement() if op=='commit' else None
        self.shutdown();self.assertNotIn(9,self.events);self.assertEqual(self.other.ended,0)
        self.assertEqual(self.phase.value,2);self.assertFalse(self.composition.value)

if __name__=='__main__':
    suite=unittest.TestSuite(ShutdownTests(name) for name in unittest.defaultTestLoader.getTestCaseNames(ShutdownTests) if name.startswith('test_shutdown_'))
    raise SystemExit(not unittest.TextTestRunner(verbosity=2,failfast=True).run(suite).wasSuccessful())
