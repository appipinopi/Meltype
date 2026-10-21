# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Composition slots, ownership transfer and reentrant replacement isolation."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_recovery_smoke import RecoveryHost, P, U32, HR, release, units
from tsf_range_smoke import FAIL
W=c.c_uint16
class Slots(c.Structure):
    _fields_=[('lifetime',P),('composition',c.POINTER(P)),('context',c.POINTER(P)),('sink',P)]
class OwnerHost(RecoveryHost):
    def __init__(self):
        self.partial_range=self.partial_property=False
        super().__init__()
    def get_range(self,*args):
        result=super().get_range(*args)
        return FAIL if self.partial_range else result
    def get_property(self,*args):
        result=super().get_property(*args)
        return FAIL if self.partial_property else result
    def clear(self,_,cookie,range):
        assert cookie==7 and range in self.ranges
        self.clears+=1
        return 0
class OwnerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_apply_owned_composition.argtypes=[P,U32,c.c_char_p,U32,U32,U32,c.POINTER(Slots)]
        cls.api.meltype_tsf_write_owned_commit.argtypes=[P,P,U32,c.POINTER(W),U32,HR,c.POINTER(Slots)]
        cls.api.meltype_tsf_end_owned_composition.argtypes=[U32,c.POINTER(Slots)]
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32];cls.api.meltype_tsf_start_composition.restype=P

    def setUp(self):
        self.host=OwnerHost();self.other=OwnerHost()
        self.host.selection=(0,3)
        self.composition=P();self.context=P()
        self.slots=Slots(c.addressof(self.host.context),c.pointer(self.composition),c.pointer(self.context),c.addressof(self.host.sink))

    def tearDown(self):
        self.clear()
        self.host.verify(self);self.other.verify(self)

    def clear(self):
        composition,context=self.composition.value,self.context.value
        self.composition.value=self.context.value=None
        if composition:release(composition)
        if context:release(context)

    def apply(self,host=None,**view):
        host=host or self.host
        return self.api.meltype_tsf_apply_owned_composition(c.byref(host.context),7,json.dumps(view or {'text':'かな'},ensure_ascii=False).encode(),11,22,33,c.byref(self.slots))

    def end(self):return self.api.meltype_tsf_end_owned_composition(7,c.byref(self.slots))

    def commit(self,text='確定😀',keep=False):
        value=units(text);buffer=(W*len(value))(*value)
        return self.api.meltype_tsf_write_owned_commit(c.byref(self.host.context),self.host.original,7,buffer,len(value),int(keep),c.byref(self.slots))

    def replacement(self,same_context=False):
        target=self.api.meltype_tsf_start_composition(c.byref(self.other.context),c.byref(self.other.sink),self.other.original,7)
        self.assertTrue(target)
        host=self.host if same_context else self.other
        host.context_ref(1)
        self.clear()
        self.composition.value=target;self.context.value=c.addressof(host.context)

    def test_owner_start_publishes_owned_pair_and_updates_utf16_view(self):
        self.assertEqual(self.apply(text='日本😀',converting=True,clauses=['日','本😀'],selectedClause=1),1)
        self.assertEqual(self.composition.value,c.addressof(self.host.composition));self.assertEqual(self.context.value,c.addressof(self.host.context))
        self.assertEqual(self.host.context_refs,2);self.assertEqual(self.host.composition_refs,1)
        self.assertEqual(self.host.document,units('日本😀'));self.assertEqual(self.host.selection,(4,4))
        self.assertEqual(self.host.attributes,[(0,1,22),(1,4,33)])

    def test_owner_existing_view_reuses_composition_without_extra_slot_refs(self):
        self.apply();self.apply(text='日本')
        self.assertEqual(len(self.host.started),1);self.assertEqual(self.host.context_refs,2);self.assertEqual(self.host.composition_refs,1)

    def test_owner_start_failure_leaves_slots_empty(self):
        self.host.fail_start=True
        self.assertEqual(self.apply(),0);self.assertFalse(self.composition.value);self.assertFalse(self.context.value)
        self.assertEqual(self.host.context_refs,1);self.assertEqual(self.host.document,units('old'))

    def test_owner_commit_starts_temporary_composition_then_ends_and_moves_caret(self):
        self.assertEqual(self.commit(),3)
        self.assertFalse(self.composition.value);self.assertFalse(self.context.value)
        self.assertEqual(self.host.document,units('確定😀'));self.assertEqual(self.host.selection,(4,4))
        self.assertEqual(self.host.ended,1);self.assertEqual(self.host.context_refs,1)

    def test_owner_commit_preserves_moved_caret(self):
        self.host.selection=(1,1)
        self.assertEqual(self.commit(keep=True),3);self.assertEqual(self.host.selection,(1,1))

    def test_owner_commit_falls_back_to_range_write_if_start_refused(self):
        self.host.fail_start=True
        self.assertEqual(self.commit(),1);self.assertEqual(self.host.document,units('確定😀'))
        self.assertEqual(self.host.ended,0);self.assertFalse(self.composition.value)

    def test_owner_failed_write_still_ends_temporary_composition(self):
        self.host.fail_write=True
        self.assertEqual(self.commit(),2);self.assertEqual(self.host.document,units('old'))
        self.assertFalse(self.composition.value);self.assertEqual(self.host.ended,1)

    def test_owner_end_clears_pair_and_repeated_end_does_nothing(self):
        self.apply();self.assertEqual(self.end(),1);self.assertEqual(self.end(),0)
        self.assertEqual(self.host.context_refs,1);self.assertEqual(self.host.composition_refs,0)
        self.assertFalse(self.context.value);self.assertEqual(self.host.ended,1)

    def test_owner_end_notification_can_clear_slots_without_double_release(self):
        self.apply();self.host.on_end=self.clear
        self.assertEqual(self.end(),1);self.assertEqual(self.host.context_refs,1);self.assertEqual(self.host.composition_refs,0)

    def test_owner_end_preserves_replacement_in_same_context(self):
        self.apply();self.host.on_end=lambda:self.replacement(True)
        self.assertEqual(self.end(),1)
        self.assertEqual(self.composition.value,c.addressof(self.other.composition));self.assertEqual(self.context.value,c.addressof(self.host.context))
        self.assertEqual(self.host.context_refs,2);self.assertEqual(self.other.ended,0)

    def test_owner_end_preserves_replacement_in_different_context(self):
        self.apply();self.host.on_end=self.replacement
        self.assertEqual(self.end(),1)
        self.assertEqual(self.context.value,c.addressof(self.other.context));self.assertEqual(self.other.ended,0)
        self.assertEqual(self.host.context_refs,1);self.assertEqual(self.other.context_refs,2)

    def test_owner_commit_does_not_end_replacement_created_during_write(self):
        self.apply();self.host.after_write=lambda:self.replacement(True)
        self.assertEqual(self.commit(),1)
        self.assertEqual(self.composition.value,c.addressof(self.other.composition));self.assertEqual(self.other.ended,0)

    def test_owner_original_reference_can_be_released_during_end(self):
        self.apply();self.host.on_end=lambda:self.host.context_ref(-1)
        self.assertEqual(self.end(),1);self.assertEqual(self.host.context_refs,0)
        self.host.context_ref(1)

    def test_owner_update_does_not_use_other_context_cookie(self):
        self.apply()
        self.assertEqual(self.apply(self.other,text='wrong'),0)
        self.assertEqual(self.other.document,units('old'));self.assertEqual(self.host.document,units('かな'))

    def reentrant_start(self):
        def start(*args):
            result=RecoveryHost.start(self.host,*args)
            self.replacement(True)
            return result
        self.host.bind(self.host.manager_table,3,HR,[U32,P,P,c.POINTER(P)],start)

    def test_owner_start_reentry_does_not_overwrite_replacement_preedit(self):
        self.reentrant_start()
        self.assertEqual(self.apply(text='old request'),0)
        self.assertEqual(self.composition.value,c.addressof(self.other.composition))
        self.assertEqual(self.other.document,units('old'));self.assertEqual(self.other.ended,0)
        self.assertEqual(self.host.ended,1)

    def test_owner_commit_start_reentry_does_not_end_replacement(self):
        self.reentrant_start()
        self.assertEqual(self.commit(),1)
        self.assertEqual(self.composition.value,c.addressof(self.other.composition))
        self.assertEqual(self.other.document,units('old'));self.assertEqual(self.other.ended,0)

    def test_owner_invalid_slot_addresses_are_rejected(self):
        invalid=Slots(self.slots.lifetime,None,None,self.slots.sink)
        self.assertEqual(self.api.meltype_tsf_end_owned_composition(7,c.byref(invalid)),-1)
        invalid.composition=invalid.context=c.pointer(self.composition)
        self.assertEqual(self.api.meltype_tsf_end_owned_composition(7,c.byref(invalid)),-1)

    def test_owner_failed_get_range_releases_populated_output(self):
        self.apply();self.host.partial_range=True
        self.assertEqual(self.end(),1)
        self.assertEqual(self.host.ended,1);self.assertEqual(self.host.composition_refs,0)

    def test_owner_failed_attribute_property_releases_populated_output(self):
        self.apply();self.host.partial_property=True
        self.assertEqual(self.end(),1)
        self.assertEqual(self.host.property_refs,1);self.assertEqual(self.host.ended,1)

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
