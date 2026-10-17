# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Composition lifecycle/style tests over controlled, SDK-layout COM hosts."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_range_smoke import Host,units,FAIL
from tsf_com_smoke import Guid,P,U32,HR,release

MANAGER=Guid.parse("d40c8aae-ac92-4fc7-9a11-0ee0e23aa39b")
class CompositionHost(Host):
    def __init__(self):
        super().__init__("old",0,3)
        self.manager_refs=1;self.context_refs=1;self.composition_refs=0;self.sink_refs=1
        self.attributes=[];self.ended=0;self.started=[];self.fail_start=False;self.on_end=None;self.after_write=None
        self.manager_table=(P*7)();self.composition_table=(P*7)();self.sink_table=(P*4)()
        self.bind(self.context_table,0,HR,[c.POINTER(Guid),c.POINTER(P)],self.query_context)
        self.bind(self.context_table,1,U32,[],lambda _:self.context_ref(1))
        self.bind(self.context_table,2,U32,[],lambda _:self.context_ref(-1))
        self.bind(self.manager_table,1,U32,[],lambda _:self.manager_ref(1))
        self.bind(self.manager_table,2,U32,[],lambda _:self.manager_ref(-1))
        self.bind(self.manager_table,3,HR,[U32,P,P,c.POINTER(P)],self.start)
        self.bind(self.composition_table,1,U32,[],lambda _:self.composition_ref(1))
        self.bind(self.composition_table,2,U32,[],lambda _:self.composition_ref(-1))
        self.bind(self.composition_table,3,HR,[c.POINTER(P)],self.get_range)
        self.bind(self.composition_table,6,HR,[U32],self.end)
        self.bind(self.property_table,9,HR,[U32,P,P],self.set_value)
        self.manager=self.Object(self.manager_table);self.composition=self.Object(self.composition_table);self.sink=self.Object(self.sink_table)
    def query_context(self,_,iid,out):
        out[0]=None
        if bytes(iid.contents)!=bytes(MANAGER):return HR(0x80004002).value
        self.manager_ref(1);out[0]=c.addressof(self.manager);return 0
    def manager_ref(self,delta):
        self.manager_refs+=delta;return self.manager_refs
    def context_ref(self,delta):
        self.context_refs+=delta;return self.context_refs
    def composition_ref(self,delta):
        self.composition_refs+=delta
        assert self.composition_refs>=0
        if self.composition_refs==0:
            self.ref(self.original,-1);self.sink_refs-=1
        return self.composition_refs
    def start(self,_,cookie,range,sink,out):
        assert cookie==7 and range==self.original and sink==c.addressof(self.sink)
        out[0]=None
        if self.fail_start:return FAIL
        self.started.append(range);self.composition_refs=1
        self.ref(self.original,1);self.sink_refs+=1
        out[0]=c.addressof(self.composition);return 0
    def get_range(self,_,out):
        assert self.composition_refs>0
        self.ref(self.original,1);out[0]=self.original;return 0
    def end(self,_,cookie):
        assert cookie==7
        self.ended+=1
        if self.on_end:self.on_end()
        # The Rust-owned reference must survive the host's reentrant Release.
        assert self.composition_refs>0
        return 0
    def set_value(self,_,cookie,range,value):
        assert cookie==7 and c.cast(value,c.POINTER(c.c_uint16))[0]==3 # VT_I4
        atom=c.cast(value+8,c.POINTER(HR))[0]
        start,end,_=self.ranges[range];self.attributes.append((start,end,atom));return 0
    def write(self,*args):
        result=super().write(*args)
        if self.after_write:self.after_write()
        return result
    def verify(self,test):
        super().verify(test)
        test.assertEqual((self.manager_refs,self.context_refs,self.composition_refs,self.sink_refs),(1,1,0,1))

class CompositionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32]
        cls.api.meltype_tsf_start_composition.restype=P
        cls.api.meltype_tsf_update_composition.argtypes=[P,P,U32,c.c_char_p,U32,U32,U32]
        cls.api.meltype_tsf_end_composition.argtypes=[P,P,U32]
        cls.api.meltype_tsf_set_attribute.argtypes=[P,P,U32,U32]
    def setUp(self):
        self.host=CompositionHost();self.pointer=None
        self.addCleanup(self.host.verify,self)
        self.addCleanup(self.close)
    def close(self):
        if self.pointer:release(self.pointer);self.pointer=None
    def start(self,explicit=False):
        self.pointer=self.api.meltype_tsf_start_composition(c.byref(self.host.context),c.byref(self.host.sink),self.host.original if explicit else None,7)
        return self.pointer
    def update(self,**view):
        return self.api.meltype_tsf_update_composition(c.byref(self.host.context),self.pointer,7,json.dumps(view,ensure_ascii=False).encode(),11,22,33)
    def test_start_from_selection_and_utf16_clause_styles(self):
        self.assertTrue(self.start())
        self.assertEqual(self.update(text="日本😀",converting=True,clauses=["日","本😀"],selectedClause=1),1)
        self.assertEqual(self.host.document,units("日本😀"))
        self.assertEqual(self.host.attributes,[(0,1,22),(1,4,33)])
        self.assertEqual(self.host.selection,(4,4))
        self.assertEqual(self.api.meltype_tsf_end_composition(c.byref(self.host.context),self.pointer,7),1)
        self.assertEqual(self.host.clears,1);self.assertEqual(self.host.ended,1)
    def test_explicit_range_and_unconverted_input_attribute(self):
        self.assertTrue(self.start(explicit=True))
        self.assertEqual(self.update(text="にほn"),1)
        self.assertEqual(self.host.document,units("にほn"))
        self.assertEqual(self.host.attributes,[(0,3,11)])
        self.assertEqual(self.host.selection,(3,3))
    def test_start_failure_releases_selection_and_manager_references(self):
        self.host.fail_start=True
        self.assertFalse(self.start())
        self.assertEqual(self.host.writes,[])
        self.assertEqual(self.host.document,units("old"))
    def test_invalid_clause_lengths_and_failed_write_do_not_move_caret_or_style(self):
        self.start()
        self.assertEqual(self.update(text="😀",converting=True,clauses=["日"]),0)
        self.host.fail_write=True
        self.assertEqual(self.update(text="変更"),0)
        self.assertEqual(self.host.document,units("old"))
        self.assertEqual(self.host.selection,(0,3));self.assertEqual(self.host.attributes,[])
    def test_end_retains_composition_across_reentrant_owner_release(self):
        self.start()
        pointer=self.pointer;self.pointer=None
        self.host.on_end=lambda:release(pointer)
        self.assertEqual(self.api.meltype_tsf_end_composition(c.byref(self.host.context),pointer,7),1)
        self.assertEqual(self.host.composition_refs,0)
        self.assertEqual(self.host.ended,1)
    def test_resume_attribute_and_invalid_atom(self):
        self.api.meltype_tsf_set_attribute(c.byref(self.host.context),self.host.original,7,0)
        self.assertEqual(self.host.attributes,[])
        self.api.meltype_tsf_set_attribute(c.byref(self.host.context),self.host.original,7,44)
        self.assertEqual(self.host.attributes,[(0,3,44)])
    def test_update_retains_composition_when_host_releases_owner_during_write(self):
        self.start()
        pointer=self.pointer;self.pointer=None
        self.host.after_write=lambda:release(pointer)
        result=self.api.meltype_tsf_update_composition(c.byref(self.host.context),pointer,7,b'{"text":"new"}',11,22,33)
        self.assertEqual(result,1)
        self.assertEqual(self.host.attributes,[(0,3,11)])
        self.assertEqual(self.host.composition_refs,0)
    def test_start_selection_error_releases_partial_host_output(self):
        self.host.fail_selection=True
        self.assertFalse(self.start())
        self.assertEqual(self.host.started,[])

if __name__=="__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
