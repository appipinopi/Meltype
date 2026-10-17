# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Candidate/prediction scheduling over private TSF hosts, no desktop input."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_commit_request_smoke import CommitTests, Callbacks, P, U32, HR, ASYNC, SYNCHRONOUS

class SelectionTests(CommitTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_select_candidate.argtypes=[U32,HR,HR,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_select_candidate.restype=HR

    def setUp(self):
        super().setUp()
        self.context_index=0
        self.hosts[0].mode='async'

    def current(self,owner,context,sink):
        target=super().current(owner,context,sink)
        context[0]=c.addressof(self.hosts[self.context_index].context) if self.context_index is not None else None
        return target

    def request(self,owner,text,cookie,context,target):
        self.assertEqual((owner,cookie),(123,7))
        self.assertEqual(context,c.addressof(self.hosts[self.context_index].context))
        self.assertEqual(target,self.compositions[self.selected] if self.has_composition else None)
        self.assertGreaterEqual(self.hosts[0].context_refs,2)
        self.assertTrue(self.own);self.assertFalse(self.keep)
        self.requests.append(json.loads(text))
        if self.on_request:self.on_request()

    def select(self,index=2,prediction=False):
        return self.api.meltype_tsf_select_candidate(17,index,int(prediction),self.metadata,c.byref(self.bridge))

    def test_selection_async_request_is_delayed_and_sets_only_own_edit(self):
        self.assertEqual(self.select(),ASYNC)
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])
        self.hosts[0].flush()
        self.assertEqual(self.requests,[{'op':'select','sid':'確定"😀','index':2}])
        self.assertEqual(self.events,[(0,7,1)]);self.assertEqual(self.hosts[0].requests,[(17,6)])
        self.assertTrue(self.own);self.assertEqual(self.hosts[0].ended,0)

    def test_selection_prediction_and_signed_index_are_preserved(self):
        for index in [-2147483648,-1,0,2147483647]:
            self.select(index,True);self.hosts[0].flush()
            self.assertEqual(self.requests[-1],{'op':'select','sid':'確定"😀','index':index,'prediction':True})

    def test_selection_cancel_releases_owner_context_target_without_request(self):
        self.select();self.assertEqual(self.hosts[0].context_refs,3);self.assertEqual(self.hosts[0].composition_refs,2)
        self.hosts[0].cancel()
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])
        self.assertEqual(self.hosts[0].context_refs,1);self.assertEqual(self.hosts[0].composition_refs,1)

    def test_selection_changed_composition_does_nothing(self):
        self.select();self.selected=1;self.hosts[0].flush()
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])

    def test_selection_changed_context_with_same_composition_does_nothing(self):
        self.select();self.context_index=1;self.hosts[0].flush()
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])

    def test_selection_ended_composition_does_nothing(self):
        self.select();self.has_composition=False;self.hosts[0].flush()
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])

    def test_selection_null_composition_with_context_matches_original(self):
        self.has_composition=False
        self.assertEqual(self.select(),ASYNC)
        self.assertEqual(self.hosts[0].composition_refs,1)
        self.hosts[0].flush()
        self.assertEqual(len(self.requests),1);self.assertEqual(self.events,[(0,7,1)])

    def test_selection_new_composition_after_null_snapshot_is_ignored(self):
        self.has_composition=False;self.select();self.has_composition=True;self.hosts[0].flush()
        self.assertEqual(self.requests,[]);self.assertEqual(self.events,[])

    def test_selection_missing_context_does_not_schedule(self):
        self.context_index=None
        self.assertEqual(self.select(),0);self.assertEqual(self.hosts[0].requests,[])
        self.assertEqual(self.events,[])

    def test_selection_refused_session_releases_without_changing_own_edit(self):
        self.hosts[0].mode='reject'
        self.assertEqual(self.select(),SYNCHRONOUS)
        self.assertEqual(self.hosts[0].context_refs,1);self.assertEqual(self.hosts[0].composition_refs,1)
        self.assertFalse(self.own);self.assertEqual(self.events,[])

    def test_selection_host_can_execute_immediately(self):
        self.hosts[0].mode='sync'
        self.assertEqual(self.select(),0);self.assertEqual(len(self.requests),1)
        self.assertEqual(self.hosts[0].context_refs,1);self.assertTrue(self.own)

    def test_selection_pending_owner_survives_original_reference_release(self):
        self.select();self.hosts[0].context_ref(-1)
        self.hosts[0].flush()
        self.assertEqual(len(self.requests),1);self.assertEqual(self.hosts[0].context_refs,0)
        self.hosts[0].context_ref(1)

    def test_selection_request_can_release_original_owner(self):
        self.on_request=lambda:self.hosts[0].context_ref(-1)
        self.select();self.hosts[0].flush()
        self.assertEqual(self.hosts[0].context_refs,0);self.hosts[0].context_ref(1)
        self.assertEqual(self.events,[(0,7,1)])

    def test_selection_queued_second_click_is_ignored_after_first_ends_target(self):
        self.select(1);self.select(2)
        self.on_request=lambda:setattr(self,'has_composition',False)
        self.hosts[0].flush()
        self.assertEqual([r['index'] for r in self.requests],[1])
        self.assertEqual(self.events,[(0,7,1)])

    def test_selection_invalid_metadata_does_not_schedule(self):
        for metadata in [b'{"sid":12}',b'bad',b'\xff']:
            self.metadata=metadata;self.assertEqual(self.select(),HR(0x80070057).value)
        self.assertEqual(self.events,[]);self.assertEqual(self.hosts[0].requests,[])

if __name__=='__main__':
    suite=unittest.TestSuite(SelectionTests(name) for name in dir(SelectionTests) if name.startswith('test_selection_'))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
