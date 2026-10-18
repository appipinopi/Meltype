# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Reply application order and non-destructive cleanup over private TSF COM."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_commit_smoke import CommitTests, CURRENT, WRITE, P, U32, units
from tsf_com_smoke import HR, release
from tsf_range_smoke import FAIL
ACTION=c.CFUNCTYPE(None,P,U32,U32,P,c.c_char_p,HR)
class Callbacks(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('current',CURRENT),('write',WRITE),('action',ACTION)]

class ReplyTests(CommitTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass()
        cls.api.meltype_tsf_apply_reply.argtypes=[P,U32,c.c_char_p,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_update_composition.argtypes=[P,P,U32,c.c_char_p,U32,U32,U32]

    def setUp(self):
        super().setUp()
        self.events=[];self.on_action=None;self.end_native=True
        self.on_writer=None
        self.metadata=json.dumps({'sid':'応答"😀','direct':True},ensure_ascii=False).encode()
        self.action_callback=ACTION(self.action)
        self.bridge=Callbacks(123,c.addressof(self.host.context),self.current,self.writer,self.action_callback)

    def action(self,owner,op,cookie,context,text,value):
        try:
            self.assertEqual((owner,cookie,context),(123,7,c.addressof(self.host.context)))
            self.assertGreaterEqual(self.host.context_refs,2)
            data=json.loads(text) if text else None
            self.events.append((op,value,data))
            if self.on_action:self.on_action(op)
            if op==1:
                if not self.active:self.begin()
                self.assertTrue(self.api.meltype_tsf_update_composition(context,self.active,cookie,text,11,22,33))
            elif op==3:
                if self.active and self.end_native:self.api.meltype_tsf_end_composition(context,self.active,cookie)
                self.close()
        except BaseException as error:self.errors.append(repr(error))

    def write(self,*args):
        self.assertGreaterEqual(self.host.context_refs,3)
        result=super().write(*args)
        if self.on_writer:self.on_writer()
        return result

    def reply(self,**reply):
        return self.api.meltype_tsf_apply_reply(c.byref(self.host.context),7,json.dumps(reply,ensure_ascii=False).encode(),self.metadata,c.byref(self.bridge))

    def test_reply_commits_before_preedit_and_candidates(self):
        view={'text':'にほn😀','converting':False,'predictions':['日本😀'],'meaning':'語"\n注釈'}
        self.assertEqual(self.reply(active=True,commits=[{'text':'確定'}],view=view),1)
        self.assertEqual(self.writes,[units('確定')]);self.assertEqual(self.host.document,units('確定にほn😀'))
        self.assertEqual([e[0] for e in self.events],[0,1,2])
        self.assertEqual(self.events[0],(0,0,None));self.assertEqual(self.events[-1][2],view)

    def test_reply_conversion_state_is_published_before_composition(self):
        view={'text':'日本','converting':True,'clauses':['日','本'],'selectedClause':1}
        self.assertEqual(self.reply(active=True,view=view),1)
        self.assertEqual(self.events[0],(0,1,None));self.assertEqual(self.host.attributes,[(0,1,22),(1,2,33)])

    def test_reply_reconversion_matches_selected_utf16_before_preedit(self):
        self.host.document=units('前😀日本後');self.host.selection=(1,5)
        self.assertEqual(self.reply(active=True,selectionExpect='😀日本',view={'text':'再変換','converting':True}),1)
        self.assertEqual(self.host.document,units('前再変換後'))
        self.assertEqual([e[0] for e in self.events],[0,1,2])

    def test_reply_reconversion_guard_rejects_stale_prefix_and_malformed_expectations(self):
        for expected in ['new','ol','','old extra',None,42,True,{},'a'*2049]:
            self.events.clear()
            self.assertEqual(self.reply(active=True,selectionExpect=expected,commits=[{'text':'BAD'}],view={'text':'BAD'}),0)
            self.assertEqual([e[0] for e in self.events],[5])
            self.assertEqual(self.host.document,units('old'));self.assertEqual(self.writes,[])

    def test_reply_reconversion_guard_rejects_unreadable_or_existing_composition(self):
        self.host.fail_read=True
        self.assertEqual(self.reply(active=True,selectionExpect='old',view={'text':'BAD'}),0)
        self.host.fail_read=False;self.begin();self.events.clear()
        self.assertEqual(self.reply(active=True,selectionExpect='old',view={'text':'BAD'}),0)
        self.assertEqual([e[0] for e in self.events],[5]);self.assertEqual(self.host.document,units('old'))

    def test_reply_partial_read_of_longer_selection_does_not_satisfy_original_text_guard(self):
        self.host.document=units('old extra');self.host.selection=(0,9);self.host.read_limit=3
        self.assertEqual(self.reply(active=True,selectionExpect='old',view={'text':'BAD'}),0)
        self.assertEqual([e[0] for e in self.events],[5])
        self.assertEqual(self.host.document,units('old extra'));self.assertEqual(self.writes,[])

    def test_reply_inactive_empty_view_preserves_existing_preedit(self):
        self.begin();self.assertEqual(self.reply(active=False,commits=[],view=None),1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.writes,[])
        self.assertEqual([e[0] for e in self.events],[0,3,4]);self.assertEqual(self.host.ended,1)

    def test_reply_active_empty_view_erases_existing_preedit(self):
        self.begin();self.assertEqual(self.reply(active=True,view=None),1)
        self.assertEqual(self.host.document,[]);self.assertEqual(self.host.ended,1)
        self.assertEqual([e[0] for e in self.events],[0,3,4])

    def test_reply_nonempty_view_is_applied_even_if_active_false_like_original(self):
        self.assertEqual(self.reply(active=False,view={'text':'保持'}),1)
        self.assertEqual(self.host.document,units('保持'));self.assertEqual([e[0] for e in self.events],[0,1,2])

    def test_reply_empty_text_preserves_converting_flag_then_hides(self):
        self.assertEqual(self.reply(active=True,view={'text':'','converting':True}),1)
        self.assertEqual(self.events,[(0,1,None),(4,0,None)])

    def test_reply_failed_guard_resets_and_stops_preedit_and_candidates(self):
        self.host.selection=(3,3)
        self.assertEqual(self.reply(active=True,commits=[{'text':'wrong','deleteBefore':3,'expect':'bad'}],view={'text':'new'}),0)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.events,[(5,0,{'op':'reset','sid':'応答"😀','direct':True})])

    def test_reply_partial_commit_then_failed_guard_stops_remaining_view(self):
        self.assertEqual(self.reply(active=True,commits=[{'text':'safe'},{'text':'wrong','deleteBefore':4,'expect':'xxxx'}],view={'text':'new'}),0)
        self.assertEqual(self.host.document,units('safe'));self.assertEqual([e[0] for e in self.events],[5])

    def test_reply_ordinary_write_failure_still_applies_view(self):
        self.host.fail_write=True
        self.assertEqual(self.reply(active=True,commits=[{'text':'x'}],view=None),1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual([e[0] for e in self.events],[0,4])

    def test_reply_failed_clear_still_ends_composition_without_claiming_erase(self):
        self.begin();self.host.fail_write=True
        self.assertEqual(self.reply(active=True,view=None),1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.ended,1)

    def test_reply_range_failure_with_populated_output_releases_it(self):
        self.begin();self.end_native=False
        def partial(_,out):
            self.host.ref(self.host.original,1);out[0]=self.host.original;return FAIL
        self.host.bind(self.host.composition_table,3,HR,[c.POINTER(P)],partial)
        self.assertEqual(self.reply(active=True,view=None),1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual(self.host.ranges[self.host.original][2],1)

    def test_reply_reentrant_end_during_clear_does_not_end_again(self):
        self.begin()
        self.host.after_write=self.close
        self.assertEqual(self.reply(active=True,view=None),1)
        self.assertNotIn(3,[e[0] for e in self.events]);self.assertEqual(self.host.document,[])

    def test_reply_owner_can_release_original_reference_during_candidate_notification(self):
        def notify(op):
            if op==2:self.host.context_ref(-1)
        self.on_action=notify
        self.assertEqual(self.reply(active=True,view={'text':'かな'}),1)
        self.assertEqual(self.host.context_refs,0);self.host.context_ref(1)

    def test_reply_owner_can_release_original_reference_during_commit(self):
        self.on_writer=lambda:self.host.context_ref(-1)
        self.assertEqual(self.reply(active=True,commits=[{'text':'確定'}],view=None),1)
        self.assertEqual(self.host.context_refs,0);self.host.context_ref(1)

    def test_reply_missing_fields_match_empty_reply_behavior(self):
        self.begin();self.assertEqual(self.reply(),1)
        self.assertEqual(self.host.document,units('old'));self.assertEqual([e[0] for e in self.events],[0,3,4])

    def test_reply_invalid_json_or_metadata_has_no_side_effects(self):
        for reply,metadata in [(b'bad',self.metadata),(b'{}',b'{"sid":false}')]:
            self.assertEqual(self.api.meltype_tsf_apply_reply(c.byref(self.host.context),7,reply,metadata,c.byref(self.bridge)),0)
        self.assertEqual(self.events,[]);self.assertEqual(self.writes,[])

if __name__=='__main__':
    suite=unittest.TestSuite(ReplyTests(name) for name in dir(ReplyTests) if name.startswith('test_reply_'))
    result=unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
