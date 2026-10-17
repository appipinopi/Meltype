# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Activation through private SDK COM hosts; no system/profile registration."""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_subscriptions_smoke import SubscriptionHost
from tsf_com_smoke import P, U32, HR, Guid, invoke, release
from tsf_range_smoke import FAIL
BAR_MANAGER=Guid.parse('ba468c55-9956-4fb1-a59d-52a7dd7cc6aa')
INITIALIZE=c.CFUNCTYPE(None,P,U32,c.c_int,c.c_int,c.c_char_p)
ACTION=c.CFUNCTYPE(P,P,U32,c.c_int,P)
CATEGORY=c.CFUNCTYPE(P,P)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('phase',c.POINTER(U32)),
              ('manager',c.POINTER(P)),('keys',c.POINTER(P)),('abandoned',c.POINTER(P)),('resume',c.POINTER(P)),
              ('subscriptions',c.POINTER(P)*3),('atoms',c.POINTER(U32)*3),('sinks',P*3),
              ('initialize',INITIALIZE),('action',ACTION),('category',CATEGORY)]
class StartupHost(SubscriptionHost):
    def __init__(self):
        super().__init__()
        self.log=[];self.on_focus=None;self.fail_focus=self.partial_focus=self.null_focus=False
        self.fail_bar_query=self.partial_bar_query=False
        self.bar_refs=self.bar_manager_refs=self.category_refs=1
        self.held=[];self.registered=[];self.fail_register=None
        self.bind(self.context_table,7,HR,[c.POINTER(P)],self.focus)
        self.category_table=(P*17)();self.bar_table=(P*3)();self.bar_manager_table=(P*15)()
        for table,name in [(self.category_table,'category_refs'),(self.bar_table,'bar_refs'),(self.bar_manager_table,'bar_manager_refs')]:
            self.bind(table,1,U32,[],lambda _,name=name:self.count(name,1))
            self.bind(table,2,U32,[],lambda _,name=name:self.count(name,-1))
        self.bind(self.category_table,14,HR,[c.POINTER(Guid),c.POINTER(U32)],self.register)
        self.bind(self.bar_manager_table,5,HR,[P],self.add_bar)
        self.category=self.Object(self.category_table);self.bar=self.Object(self.bar_table);self.bar_manager=self.Object(self.bar_manager_table)
    def query(self,this,iid,out):
        if bytes(iid.contents)==bytes(BAR_MANAGER):
            out[0]=None
            if not self.fail_bar_query or self.partial_bar_query:
                self.count('bar_manager_refs',1);out[0]=c.addressof(self.bar_manager)
            return FAIL if self.fail_bar_query else 0
        return super().query(this,iid,out)
    def focus(self,this,out):
        out[0]=None;self.log.append('focus')
        if not self.null_focus and (not self.fail_focus or self.partial_focus):
            self.count('document_refs',1);out[0]=c.addressof(self.document_manager)
        if self.on_focus:self.on_focus()
        return FAIL if self.fail_focus else 0
    def register(self,this,guid,out):
        index=len(self.registered);self.registered.append(bytes(guid.contents));self.log.append(('atom',index));out[0]=700+index
        return FAIL if index==self.fail_register else 0
    def add_bar(self,this,bar):
        self.log.append('addbar');invoke(bar,1,U32,[]);self.held.append(bar);return 0
    def clear_bars(self):
        held,self.held=self.held,[]
        for bar in held:release(bar)
    def verify(self,test):
        super().verify(test);test.assertEqual((self.category_refs,self.bar_refs,self.bar_manager_refs),(1,1,1))
class StartupTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_startup.argtypes=[P,U32,U32,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_startup.restype=HR
        for name in ['unsubscribe','key_state_destroy','abandoned_destroy','destroy_resume']:
            getattr(cls.api,'meltype_tsf_'+name).argtypes=[P]
        cls.api.meltype_tsf_key_state_create.restype=P
        cls.api.meltype_tsf_key_stale_write.argtypes=[P,U32,U32,c.c_uint64]
        cls.api.meltype_tsf_key_stale_read.argtypes=[P,P]
    def setUp(self):
        self.host=StartupHost();self.errors=[];self.initialized=[];self.actions=[]
        self.epoch=c.c_uint64();self.phase=U32(2)
        self.manager,self.keys,self.abandoned,self.resume=P(),P(),P(),P()
        self.tokens=[P() for _ in range(3)];self.atoms=[U32(10+i) for i in range(3)]
        self.on_initialize=self.on_action=None;self.with_category=True
        def guard(fn,default=None):
            def run(*args):
                try:
                    self.assertEqual(args[0],123);self.assertGreaterEqual(self.host.context_refs,2)
                    return fn(*args)
                except BaseException as e:self.errors.append(repr(e));return default
            return run
        self.callbacks=[INITIALIZE(guard(self.initialize)),ACTION(guard(self.action)),CATEGORY(guard(self.category))]
        self.bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.epoch),c.pointer(self.phase),
            c.pointer(self.manager),c.pointer(self.keys),c.pointer(self.abandoned),c.pointer(self.resume),
            (c.POINTER(P)*3)(*[c.pointer(token) for token in self.tokens]),
            (c.POINTER(U32)*3)(*[c.pointer(atom) for atom in self.atoms]),
            (P*3)(*[c.addressof(self.host.sink)]*3),*self.callbacks)
    def tearDown(self):
        for token in self.tokens:self.api.meltype_tsf_unsubscribe(token)
        self.api.meltype_tsf_key_state_destroy(self.keys);self.api.meltype_tsf_abandoned_destroy(self.abandoned)
        self.api.meltype_tsf_destroy_resume(self.resume)
        if self.manager.value:release(self.manager)
        self.host.clear_bars();self.host.verify(self);self.assertEqual(self.errors,[])
    def initialize(self,owner,client,disabled,own,sid):
        self.initialized.append((client,disabled,own,sid.decode()));self.host.log.append('initialize')
        if self.on_initialize:self.on_initialize()
    def action(self,owner,op,value,document):
        self.actions.append((op,value,document));self.host.log.append(('action',op))
        if self.on_action:self.on_action(op)
        return c.addressof(self.host.bar) if op==2 else None
    def category(self,owner):
        if not self.with_category:return None
        self.host.count('category_refs',1);return c.addressof(self.host.category)
    def startup(self,flags=0,manager='default'):
        return self.api.meltype_tsf_startup(c.addressof(self.host.context) if manager=='default' else manager,17,flags,c.byref(self.bridge))
    def stop(self):self.epoch.value+=1;self.phase.value=2
    def ops(self):return [a[0] for a in self.actions]
    def test_startup_orders_atoms_mode_ui_focus_hello(self):
        self.assertEqual(self.startup(),0);self.assertEqual(self.phase.value,0)
        self.assertEqual(self.ops(),[0,1,2,3,4]);self.assertEqual(self.actions[0],(0,1,None))
        self.assertEqual(self.host.log,['initialize',('atom',0),('atom',1),('atom',2),('action',0),('action',1),('action',2),'addbar','focus',('action',3),('action',4)])
        self.assertTrue(all(token.value for token in self.tokens));self.assertEqual([a.value for a in self.atoms],[700,701,702])
        self.assertEqual(self.initialized[0][:3],(17,0,0));self.assertRegex(self.initialized[0][3],r'^\d+-\d+-\d+$')
        expected=['c8107d36-2806-4d8f-a183-fd0aeb8ed862','e17aa399-c5eb-42e6-be45-c44f0651f2e2','cef94813-c47f-48c1-b4ef-d4582d5d4054']
        self.assertEqual(self.host.registered,[bytes(Guid.parse(guid)) for guid in expected])
    def test_startup_secure_mode_skips_mode_bar_and_hello(self):
        self.startup(2);self.assertEqual(self.initialized[0][1],1)
        self.assertEqual(self.ops(),[0,1,3]);self.assertEqual(self.actions[0],(0,0,None))
        self.assertFalse(self.tokens[2].value);self.assertEqual(self.host.held,[])
    def test_startup_other_flags_do_not_disable_input(self):
        self.startup(1);self.assertEqual(self.initialized[0][1],0);self.assertIn(4,self.ops())
    def test_startup_thread_subscription_failure_does_not_stop_other_steps(self):
        self.host.fail_source=True;self.startup()
        self.assertFalse(self.tokens[0].value);self.assertTrue(self.tokens[1].value);self.assertTrue(self.tokens[2].value)
    def test_startup_key_subscription_failure_does_not_stop_mode(self):
        self.host.fail_key_advise=True;self.startup()
        self.assertFalse(self.tokens[1].value);self.assertTrue(self.tokens[2].value);self.assertIn(4,self.ops())
    def test_startup_category_unavailable_keeps_existing_atoms(self):
        self.with_category=False;self.startup();self.assertEqual([a.value for a in self.atoms],[10,11,12]);self.assertIn(4,self.ops())
    def test_startup_failed_guid_preserves_only_its_old_atom(self):
        self.host.fail_register=1;self.startup();self.assertEqual([a.value for a in self.atoms],[700,11,702])
    def test_startup_null_focus_skips_edit_advice(self):
        self.host.null_focus=True;self.startup();self.assertEqual(self.ops(),[0,1,2,4])
    def test_startup_failed_populated_focus_output_is_released(self):
        self.host.fail_focus=self.host.partial_focus=True;self.startup()
        self.assertNotIn(3,self.ops());self.assertEqual(self.host.document_refs,1)
    def test_startup_failed_populated_bar_manager_output_is_released(self):
        self.host.fail_bar_query=self.host.partial_bar_query=True;self.startup()
        self.assertEqual(self.host.bar_manager_refs,1);self.assertEqual(self.host.held,[])
    def test_startup_shutdown_during_initialize_stops_subscriptions(self):
        self.on_initialize=self.stop;self.startup()
        self.assertEqual(self.ops(),[]);self.assertTrue(all(not token.value for token in self.tokens))
    def test_startup_shutdown_during_mode_change_unsubscribes_late_token(self):
        self.host.on_write=self.stop;self.startup()
        self.assertFalse(self.tokens[2].value);self.assertEqual(self.host.sources[2].registrations,{})
        self.assertEqual(self.ops(),[]);self.assertTrue(self.tokens[0].value)
    def test_startup_shutdown_during_focus_releases_document_and_skips_hello(self):
        self.host.on_focus=self.stop;self.startup()
        self.assertEqual(self.ops(),[0,1,2]);self.assertEqual(self.host.document_refs,1)
    def test_startup_nested_activation_during_initialize_wins(self):
        def again():self.on_initialize=None;self.startup()
        self.on_initialize=again;self.startup()
        self.assertEqual(len(self.initialized),2);self.assertEqual(self.ops(),[0,1,2,3,4]);self.assertEqual(len(self.host.held),1)
    def test_startup_nested_activation_during_bar_creation_skips_old_add(self):
        def again(op):
            if op==2:self.on_action=None;self.startup()
        self.on_action=again;self.startup();self.assertEqual(len(self.host.held),1)
        self.assertEqual(self.host.log.count('addbar'),1)
    def test_startup_reactivation_releases_old_subscription_tokens(self):
        self.startup();self.startup();self.assertEqual(self.host.key_unadvised,[17])
        self.assertEqual(len(self.host.sources[0].unadvised),1);self.assertEqual(len(self.host.sources[2].unadvised),1)
    def test_startup_resets_existing_stale_key_state(self):
        class Stale(c.Structure):_fields_=[('tries',U32),('key',U32),('time',c.c_uint64)]
        self.keys.value=self.api.meltype_tsf_key_state_create();self.api.meltype_tsf_key_stale_write(self.keys,4,65,999)
        self.startup();value=Stale();self.api.meltype_tsf_key_stale_read(self.keys,c.byref(value))
        self.assertEqual((value.tries,value.key,value.time),(0,0,0))
    def test_startup_null_manager_is_rejected_without_mutation(self):
        self.assertEqual(self.startup(manager=None),HR(0x80070057).value)
        self.assertEqual(self.phase.value,2);self.assertEqual(self.initialized,[])
    def test_startup_aliased_slots_are_rejected(self):
        self.bridge.keys=self.bridge.manager
        self.assertEqual(self.startup(),HR(0x80070057).value);self.assertEqual(self.phase.value,2)
    def test_startup_null_sink_is_rejected(self):
        self.bridge.sinks[1]=None
        self.assertEqual(self.startup(),HR(0x80070057).value);self.assertEqual(self.phase.value,2)
    def test_startup_reentry_during_manager_addref_does_not_publish_stale_owner(self):
        original=self.host.context_ref
        additions=0
        def reference(delta):
            nonlocal additions
            result=original(delta)
            if delta==1:
                additions+=1
                if additions==3:self.stop()
            return result
        self.host.context_ref=reference
        self.startup();self.assertFalse(self.manager.value);self.assertEqual(self.initialized,[])
        self.assertEqual(self.phase.value,2);self.assertEqual(self.host.context_refs,1)

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
