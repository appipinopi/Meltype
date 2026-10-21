# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Mode notification ordering and replacement-safe publication/commit/UI."""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_fields_smoke import CompartmentHost, OPEN
from tsf_com_smoke import P, U32, HR, Guid, release
CURRENT=c.CFUNCTYPE(U32,P,c.POINTER(P))
UPDATE=c.CFUNCTYPE(None,P,U32,c.c_int)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('phase',c.POINTER(U32)),('current',CURRENT),('update',UPDATE)]
class Host(CompartmentHost):
    def __init__(self):
        super().__init__();self.on_read=None
    def read_mode(self,*args):
        result=super().read_mode(*args)
        if self.on_read:
            callback,self.on_read=self.on_read,None;callback()
        return result
class OpenChangeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_open_changed.argtypes=[c.POINTER(Guid),c.POINTER(Callbacks)]
        cls.api.meltype_tsf_open_changed.restype=HR
    def setUp(self):
        self.host,self.other=Host(),Host();self.host.context_ref(1)
        self.manager=P(c.addressof(self.host.context));self.epoch=c.c_uint64();self.phase=U32()
        self.open=True;self.composing=self.ended=False;self.direct=True
        self.events=[];self.errors=[];self.on_update=None;self.metadata_reads=0
        def guard(fn,default=None):
            def run(*args):
                try:
                    self.assertEqual(args[0],123);self.assertGreaterEqual(self.host.context_refs,1)
                    return fn(*args)
                except BaseException as e:self.errors.append(repr(e));return default
            return run
        self.callbacks=[CURRENT(guard(self.current,4)),UPDATE(guard(self.update))]
        self.bridge=Callbacks(123,c.addressof(self.host.context),c.pointer(self.epoch),c.pointer(self.phase),*self.callbacks)
    def tearDown(self):
        if self.manager.value:release(self.manager)
        self.host.verify(self);self.other.verify(self);self.assertEqual(self.errors,[])
    def current(self,owner,out):
        self.metadata_reads+=1;out[0]=self.manager.value
        return int(self.open)|(int(self.composing)<<1)|(int(self.ended)<<2)
    def update(self,owner,op,value):
        self.events.append((op,value))
        if op==0:self.open=bool(value);self.direct=False
        if self.on_update:self.on_update(op)
    def notify(self,guid=OPEN):return self.api.meltype_tsf_open_changed(c.byref(guid),c.byref(self.bridge))
    def set_mode(self,value):self.host.integers[bytes(OPEN)]=value
    def nested_open(self):
        self.on_update=None;self.set_mode(1);self.assertEqual(self.notify(),0)
    def test_open_change_open_publishes_then_logs_and_refreshes(self):
        self.assertEqual(self.notify(),0);self.assertEqual(self.events,[(0,1),(3,0),(2,0)])
        self.assertTrue(self.open);self.assertFalse(self.direct)
    def test_open_change_close_commits_before_refresh(self):
        self.set_mode(0);self.composing=True;self.notify()
        self.assertEqual(self.events,[(0,0),(3,0),(1,0),(2,0)]);self.assertFalse(self.open)
    def test_open_change_closed_without_composition_does_not_commit(self):
        self.set_mode(0);self.notify();self.assertEqual(self.events,[(0,0),(3,0),(2,0)])
    def test_open_change_nonzero_signed_value_opens(self):
        self.set_mode(-1);self.open=False;self.composing=True;self.notify()
        self.assertEqual(self.events,[(0,1),(3,0),(2,0)])
    def test_open_change_unrelated_guid_does_not_read_any_state(self):
        self.notify(Guid.parse('00000000-0000-0000-0000-000000000000'))
        self.assertEqual(self.metadata_reads,0);self.assertEqual(self.host.reads,[]);self.assertEqual(self.events,[])
    def test_open_change_null_manager_has_no_effect(self):
        release(self.manager);self.manager.value=None;self.notify()
        self.assertEqual(self.host.reads,[]);self.assertEqual(self.events,[])
    def test_open_change_failed_partial_manager_has_no_effect_or_leak(self):
        self.host.fail_manager=self.host.partial_manager=True;self.notify()
        self.assertEqual(self.events,[]);self.assertEqual(self.host.manager_refs,1)
    def test_open_change_failed_partial_compartment_preserves_mode_and_direct_word(self):
        self.host.fail_get=self.host.partial_get=True;self.notify()
        self.assertTrue(self.open);self.assertTrue(self.direct);self.assertEqual(self.events,[(3,0),(2,0)])
    def test_open_change_failed_value_still_commits_existing_closed_input(self):
        self.host.fail_read=True;self.open=False;self.composing=True;self.notify()
        self.assertEqual(self.events,[(3,0),(1,0),(2,0)]);self.assertTrue(self.direct)
    def test_open_change_noninteger_variant_closes_and_releases_interface(self):
        self.host.kind=13;self.composing=True;self.notify()
        self.assertEqual(self.events,[(0,0),(3,0),(1,0),(2,0)]);self.assertEqual(self.host.values[0].refs,1)
    def test_open_change_nested_during_read_keeps_new_mode(self):
        self.set_mode(0);self.composing=True;self.host.on_read=self.nested_open;self.notify()
        self.assertTrue(self.open);self.assertEqual(self.events,[(0,1),(3,0),(2,0)])
    def test_open_change_nested_during_publication_skips_old_commit(self):
        self.set_mode(0);self.composing=True
        self.on_update=lambda op:self.nested_open() if op==0 else None
        self.notify();self.assertEqual(self.events,[(0,0),(0,1),(3,0),(2,0)])
    def test_open_change_nested_during_commit_skips_old_refresh(self):
        self.set_mode(0);self.composing=True
        self.on_update=lambda op:self.nested_open() if op==1 else None
        self.notify();self.assertEqual(self.events,[(0,0),(3,0),(1,0),(0,1),(3,0),(2,0)])
    def test_open_change_manager_replacement_during_read_is_not_overwritten(self):
        def replace():
            old=self.manager.value;self.other.context_ref(1);self.manager.value=c.addressof(self.other.context);release(old)
        self.host.on_read=replace;self.set_mode(0);self.notify()
        self.assertTrue(self.open);self.assertTrue(self.direct);self.assertEqual(self.events,[])
    def test_open_change_shutdown_during_read_stops_updates(self):
        self.host.on_read=lambda:setattr(self.phase,'value',1);self.notify();self.assertEqual(self.events,[])
    def test_open_change_stopped_service_is_inert(self):
        self.phase.value=2;self.notify();self.assertEqual(self.metadata_reads,0);self.assertEqual(self.epoch.value,0)
    def test_open_change_deactivated_service_is_inert(self):
        self.ended=True;self.notify();self.assertEqual(self.host.reads,[]);self.assertEqual(self.events,[])
    def test_open_change_owner_retained_during_original_release(self):
        self.host.on_read=lambda:self.host.context_ref(-1)
        self.notify();self.assertEqual(self.host.context_refs,1);self.host.context_ref(1)
    def test_open_change_deactivation_after_log_stops_commit_and_refresh(self):
        self.set_mode(0);self.composing=True
        self.on_update=lambda op:setattr(self.phase,'value',1) if op==3 else None
        self.notify();self.assertEqual(self.events,[(0,0),(3,0)])

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
