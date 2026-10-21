# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Native service SDK vtables, identity, call ABI and reentrant destruction."""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_com_smoke import P,U32,HR,Guid,UNKNOWN,invoke,release,assert_rust_vtable,SPECS
N=c.c_size_t
I=c.c_ssize_t
class Call(c.Structure):
    _fields_=[('op',U32),('a',P),('b',P),('c',P),('n1',N),('n2',I)]
DISPATCH=c.CFUNCTYPE(HR,P,c.POINTER(Call))
DESTROY=c.CFUNCTYPE(None,P)
IDS=[UNKNOWN]+[Guid.parse(value) for value in [
    'aa80e7f7-2021-11d2-93e0-0060b067b86e','6e4e2102-f9cd-433d-b496-303ce03a6507',
    'aa80e80e-2021-11d2-93e0-0060b067b86e','aa80e7f5-2021-11d2-93e0-0060b067b86e',
    'a781718c-579a-4b15-a280-32b8577acc5e','fee47777-163c-4769-996a-6e9c50ad8f54',
    '743abd5f-f26d-48df-8cc5-238492419b64','8127d409-ccd3-4683-967a-b43d5b482bf7']]
class ServiceComTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_service_com_create.argtypes=[P,DISPATCH,DESTROY];cls.api.meltype_tsf_service_com_create.restype=P
        cls.api.meltype_tsf_service_com_interface.argtypes=[P,c.POINTER(Guid)];cls.api.meltype_tsf_service_com_interface.restype=P
    def setUp(self):
        self.calls=[];self.destroyed=[];self.on_call=None
        @DISPATCH
        def dispatch(owner,pointer):
            value=pointer.contents
            self.calls.append((owner,value.op,value.a,value.b,value.c,value.n1,value.n2))
            if value.op in [9,10,11,12]:c.cast(value.b,c.POINTER(HR))[0]=1
            if self.on_call:self.on_call()
            return 1
        @DESTROY
        def destroy(owner):self.destroyed.append(owner)
        self.dispatch=dispatch;self.destroy=destroy
        self.root=self.api.meltype_tsf_service_com_create(123,dispatch,destroy)
        self.assertTrue(self.root)
    def tearDown(self):
        if self.root:self.assertEqual(release(self.root),0)
        self.assertEqual(self.destroyed,[123])
    def interface(self,index):
        value=self.api.meltype_tsf_service_com_interface(self.root,c.byref(IDS[index]))
        self.assertTrue(value);return value
    def test_all_seven_sdk_vtables_and_shared_unknown_identity(self):
        for index in range(len(IDS)):
            with self.subTest(interface=index):
                value=P();self.assertEqual(invoke(self.root,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(IDS[index]),c.byref(value)),0)
                assert_rust_vtable(value.value)
                identity=P();self.assertEqual(invoke(value,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(UNKNOWN),c.byref(identity)),0)
                self.assertEqual(identity.value,self.root);release(identity);release(value)
        self.assertEqual(invoke(self.root,1,U32,[]),2);self.assertEqual(release(self.root),1)
    def test_borrowed_lookup_does_not_add_reference(self):
        for index in range(len(IDS)):self.interface(index)
        self.assertEqual(invoke(self.root,1,U32,[]),2);self.assertEqual(release(self.root),1)
        self.assertFalse(self.api.meltype_tsf_service_com_interface(None,c.byref(UNKNOWN)))
        self.assertFalse(self.api.meltype_tsf_service_com_interface(self.root,None))
    def test_invalid_query_clears_output_and_preserves_owner(self):
        unsupported=Guid.parse('00000000-0000-0000-0000-000000000001');out=P(456)
        self.assertEqual(invoke(self.root,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(unsupported),c.byref(out)),HR(0x80004002).value)
        self.assertFalse(out.value)
        out=P(456);self.assertEqual(invoke(self.root,0,HR,[P,c.POINTER(P)],None,c.byref(out)),HR(0x80070057).value);self.assertFalse(out.value)
        self.assertEqual(invoke(self.root,0,HR,[c.POINTER(Guid),P],c.byref(UNKNOWN),None),HR(0x80070057).value)
    def test_creation_failure_does_not_consume_adapter(self):
        self.assertFalse(self.api.meltype_tsf_service_com_create(456,DISPATCH(),self.destroy))
        self.assertFalse(self.api.meltype_tsf_service_com_create(456,self.dispatch,DESTROY()))
        self.assertEqual(self.destroyed,[])
    def test_activation_focus_composition_and_edit_call_layout(self):
        self.assertEqual(invoke(self.root,3,HR,[P,U32],0x1111,0xfedcba98),1)
        self.assertEqual(self.calls[-1][1:],(0,0x1111,None,None,0xfedcba98,0))
        self.assertEqual(invoke(self.root,5,HR,[P,U32,U32],0x2222,17,0x80000001),1)
        self.assertEqual(self.calls[-1][1],1);self.assertEqual(self.calls[-1][-1]&0xffffffff,0x80000001)
        self.assertEqual(invoke(self.interface(3),5,HR,[P,P],0x3333,0x4444),1)
        self.assertEqual(self.calls[-1][1:4],(5,0x3333,0x4444))
        self.assertEqual(invoke(self.interface(4),3,HR,[HR],1),1);self.assertEqual(self.calls[-1][1],8)
        self.assertEqual(invoke(self.interface(5),3,HR,[U32,P],0xabcdef01,0x5555),1)
        self.assertEqual(self.calls[-1][1:],(14,0x5555,None,None,0xabcdef01,0))
        guid=Guid.parse(SPECS[0][0]);self.assertEqual(invoke(self.interface(7),3,HR,[c.POINTER(Guid)],c.byref(guid)),1)
        self.assertEqual(self.calls[-1][1],17)
        self.assertEqual(invoke(self.interface(8),3,HR,[P,U32,P],0x6666,0x87654321,0x7777),1)
        self.assertEqual(self.calls[-1][1:],(18,0x6666,0x7777,None,0x87654321,0))
        self.assertEqual(invoke(self.root,4,HR,[]),1);self.assertEqual(self.calls[-1][1],2)
    def test_key_wparam_lparam_and_eaten_system_abi(self):
        keys=self.interface(4);word=0xfedcba9876543210 if c.sizeof(P)==8 else 0xfedcba98
        for slot,op in [(4,9),(5,10),(6,11),(7,12)]:
            eaten=HR(99)
            self.assertEqual(invoke(keys,slot,HR,[P,N,I,c.POINTER(HR)],0x1234,word,-123456789,c.byref(eaten)),1)
            self.assertEqual(self.calls[-1][1],op);self.assertEqual(self.calls[-1][-2:],(word,-123456789));self.assertEqual(eaten.value,1)
        self.assertEqual(invoke(keys,4,HR,[P,N,I,P],None,0,0,None),HR(0x80070057).value)
    def test_native_noop_and_display_paths_need_no_adapter(self):
        thread=self.interface(3)
        for slot in [3,4,6,7]:self.assertEqual(invoke(thread,slot,HR,[P],None),0)
        eaten=HR(99);self.assertEqual(invoke(self.interface(4),8,HR,[P,P,c.POINTER(HR)],None,None,c.byref(eaten)),0);self.assertEqual(eaten.value,0)
        provider=self.interface(6);out=P()
        self.assertEqual(invoke(provider,3,HR,[c.POINTER(P)],c.byref(out)),0);assert_rust_vtable(out.value);release(out)
        guid=Guid.parse(SPECS[0][0]);self.assertEqual(invoke(provider,4,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(guid),c.byref(out)),0);release(out)
        self.assertEqual(self.calls,[])
    def test_dispatch_pin_survives_caller_releasing_last_reference(self):
        keys=self.interface(4);root=self.root
        def drop():
            self.root=None;self.assertEqual(release(root),1);self.assertEqual(self.destroyed,[])
        self.on_call=drop;eaten=HR()
        self.assertEqual(invoke(keys,6,HR,[P,N,I,c.POINTER(HR)],None,65,0,c.byref(eaten)),1)
        self.assertEqual(self.destroyed,[123])
    def test_foreign_thread_never_enters_sta_adapter(self):
        keys=self.interface(4);results=[]
        def run():
            eaten=HR(99);result=invoke(keys,6,HR,[P,N,I,c.POINTER(HR)],None,65,0,c.byref(eaten));results.append((result,eaten.value))
        worker=threading.Thread(target=run);worker.start();worker.join()
        self.assertEqual(results,[(HR(0x8001010e).value,0)]);self.assertEqual(self.calls,[])
if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
