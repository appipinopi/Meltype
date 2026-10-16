# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Commit-batch ordering and guarded failure over controlled TSF COM ranges."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_composition_smoke import CompositionHost
from tsf_range_smoke import units
from tsf_com_smoke import P,U32,release

CURRENT=c.CFUNCTYPE(P,P)
WRITE=c.CFUNCTYPE(c.c_int32,P,P,c.POINTER(c.c_uint16),U32)
class BatchHost(CompositionHost):
    def get_selection(self,*args):
        self.ranges[self.original][:2]=self.selection
        return super().get_selection(*args)
    def clear(self,_,cookie,range):
        assert cookie==7 and range in self.ranges
        self.clears+=1;return 0

class CommitTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_apply_commits.argtypes=[P,U32,c.c_char_p,P,CURRENT,WRITE]
        cls.api.meltype_tsf_write_range.argtypes=[P,P,U32,c.POINTER(c.c_uint16),U32,c.c_int32]
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32]
        cls.api.meltype_tsf_start_composition.restype=P
        cls.api.meltype_tsf_end_composition.argtypes=[P,P,U32]
    def setUp(self):
        self.host=BatchHost();self.active=None;self.keep=False;self.move_caret=False
        self.callbacks=[];self.errors=[];self.writes=[]
        self.current=CURRENT(lambda _:self.active)
        self.writer=WRITE(self.write)
        self.addCleanup(self.host.verify,self)
        self.addCleanup(self.close)
    def tearDown(self):
        self.assertEqual(self.errors,[])
    def close(self):
        if self.active:release(self.active);self.active=None
    def write(self,owner,range,text,length):
        try:
            assert owner==123
            self.writes.append(list(text[:length]))
            result=self.api.meltype_tsf_write_range(c.byref(self.host.context),range,7,text,length,int(self.keep))
            if self.active:
                self.api.meltype_tsf_end_composition(c.byref(self.host.context),self.active,7)
                self.close()
            if self.move_caret:self.host.selection=(0,0)
            return result
        except BaseException as error:
            self.errors.append(repr(error));return 0
    def apply(self,edits):
        return self.api.meltype_tsf_apply_commits(c.byref(self.host.context),7,json.dumps(edits,ensure_ascii=False).encode(),123,self.current,self.writer)
    def begin(self):
        self.active=self.api.meltype_tsf_start_composition(c.byref(self.host.context),c.byref(self.host.sink),None,7)
        self.assertTrue(self.active)
    def test_multiple_commits_append_after_previous_edit_despite_caret_move(self):
        self.keep=True;self.move_caret=True
        self.assertEqual(self.apply([{"text":"日"},{"text":"😀"},{"text":"本"}]),1)
        self.assertEqual(self.host.document,units("日😀本"))
        self.assertEqual(self.host.selection,(0,0))
        self.assertEqual(self.writes,[units("日"),units("😀"),units("本")])
    def test_guarded_replacement_then_append_uses_utf16_prefix(self):
        self.host.selection=(3,3)
        self.assertEqual(self.apply([{"text":"日本","deleteBefore":3,"expect":"old"},{"text":"😀"}]),1)
        self.assertEqual(self.host.document,units("日本😀"))
        self.assertEqual(self.host.selection,(4,4))
    def test_failed_guard_stops_batch_before_later_insertions(self):
        self.host.selection=(3,3)
        self.assertEqual(self.apply([{"text":"wrong","deleteBefore":3,"expect":"bad"},{"text":"later"}]),0)
        self.assertEqual(self.host.document,units("old"));self.assertEqual(self.writes,[])
    def test_active_composition_skips_external_correction_then_commits_its_range(self):
        self.begin()
        self.assertEqual(self.apply([{"text":"wrong","deleteBefore":3,"expect":"old"},{"text":"確定"},{"text":"続き"}]),1)
        self.assertEqual(self.host.document,units("確定続き"))
        self.assertEqual(self.host.ended,1);self.assertIsNone(self.active)
        self.assertEqual(self.writes,[units("確定"),units("続き")])
    def test_deletion_write_failure_stops_following_commits(self):
        self.host.selection=(3,3);self.host.fail_write=True
        self.assertEqual(self.apply([{"text":"new","deleteBefore":3,"expect":"old"},{"text":"later"}]),0)
        self.assertEqual(self.host.document,units("old"))
        self.assertEqual(self.writes,[units("new")])
    def test_missing_selection_for_replacement_is_rejected(self):
        self.host.fail_selection=True
        self.assertEqual(self.apply([{"text":"new","deleteBefore":3,"expect":"old"}]),0)
        self.assertEqual(self.host.document,units("old"));self.assertEqual(self.writes,[])
    def test_no_commits_and_invalid_protocol_do_not_modify_document(self):
        self.assertEqual(self.apply([]),1)
        self.assertEqual(self.apply([{"text":"first"},{"text":"bad","expect":123}]),0)
        self.assertEqual(self.host.document,units("old"));self.assertEqual(self.writes,[])
    def test_guarded_undo_empty_commit_then_following_insertion(self):
        self.host.selection=(3,3)
        self.assertEqual(self.apply([{"text":"","deleteBefore":3,"expect":"old"},{"text":"new"}]),1)
        self.assertEqual(self.host.document,units("new"));self.assertEqual(self.host.selection,(3,3))
        self.assertEqual(self.writes,[[],units("new")])
    def test_later_guard_failure_preserves_completed_commit_and_stops_rest(self):
        self.assertEqual(self.apply([{"text":"safe"},{"text":"bad","deleteBefore":3,"expect":"no!"},{"text":"later"}]),0)
        self.assertEqual(self.host.document,units("safe"));self.assertEqual(self.writes,[units("safe")])

if __name__=="__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
