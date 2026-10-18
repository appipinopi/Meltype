# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Full key-edit policy against private TSF ranges; no real key injection."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_range_smoke import Host, P, U32, HR, W, FAIL, units
from tsf_com_smoke import Guid, invoke, release
from tsf_key_state_smoke import user
from tsf_key_entry_smoke import Entry, WOULD, MARK, COMPOSING, TOGGLE, HANDLE

READ=c.CFUNCTYPE(U32,P)
UPDATE=c.CFUNCTYPE(None,P,U32,U32)
EDIT=c.CFUNCTYPE(HR,P,U32,U32,P)
REQUEST=c.CFUNCTYPE(HR,P,c.c_char_p,c.POINTER(U32))
WRITE=c.CFUNCTYPE(HR,P,U32,P,P,W)
LOG=c.CFUNCTYPE(None,P,U32,HR)
class Callbacks(c.Structure):
    _fields_=[("owner",P),("lifetime",P),("read",READ),("update",UPDATE),("edit",EDIT),
              ("request",REQUEST),("write",WRITE),("log",LOG)]

class KeyHost(Host):
    def __init__(self):
        super().__init__('前😀後',3,3)
        self.context_refs=1
        self.mode,self.pending,self.locks="sync",None,[]
        self.read_count=0
        self.bind(self.context_table,0,HR,[c.POINTER(Guid),c.POINTER(P)],self.query)
        self.bind(self.context_table,1,U32,[],lambda _:self.context_ref(1))
        self.bind(self.context_table,2,U32,[],lambda _:self.context_ref(-1))
        self.bind(self.context_table,3,HR,[U32,P,U32,c.POINTER(HR)],self.edit)

    def context_ref(self,delta):
        self.context_refs+=delta
        assert self.context_refs>=0
        return self.context_refs

    def query(self,_,iid,out):
        out[0]=None
        return HR(0x80004002).value

    def edit(self,_,client,session,flags,result):
        assert client==17 and flags==7
        self.locks.append(flags)
        if self.mode=="reject":result[0]=0;return FAIL
        if self.mode=="async":
            invoke(session,1,U32,[]);self.pending=session;result[0]=0x40300;return 0
        result[0]=invoke(session,3,HR,[U32],7)
        return 0

    def read(self,*args):
        self.read_count+=1
        return super().read(*args)

    def verify(self,test):
        test.assertEqual(self.context_refs,1)
        test.assertIsNone(self.pending)
        super().verify(test)

class KeyEditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_handle_key.argtypes=[P,U32,P,U32,W,c.c_char_p,c.POINTER(Callbacks)]
        cls.api.meltype_tsf_handle_key.restype=HR
        cls.api.meltype_tsf_key_state_create.restype=P
        cls.api.meltype_tsf_key_state_destroy.argtypes=[P]
        cls.api.meltype_tsf_write_range.argtypes=[P,P,U32,c.POINTER(W),U32,HR]
        cls.api.meltype_tsf_key_event.argtypes=[P,U32,P,U32,c.c_uint64,c.POINTER(Entry)]
        user.GetKeyboardState.argtypes=[c.POINTER(c.c_ubyte)]
        user.SetKeyboardState.argtypes=[c.POINTER(c.c_ubyte)]

    def setUp(self):
        self.host=KeyHost();self.token=self.api.meltype_tsf_key_state_create()
        self.flags=6;self.own=False;self.reply=113;self.responds=True;self.secret=False;self.write_success=True
        self.events,self.requests,self.errors=[],[],[]
        self.on_secret=self.on_request=self.on_apply=self.on_mode=None
        self.metadata=json.dumps({"sid":'入力"😀',"process":'C:\\app.exe'},ensure_ascii=False).encode()
        def protect(function,fallback):
            def guarded(*args):
                try:return function(*args)
                except BaseException as error:self.errors.append(repr(error));return fallback
            return guarded
        self.callbacks=[READ(protect(lambda _:self.flags,0)),UPDATE(protect(self.update,None)),
            EDIT(protect(self.edit,1)),REQUEST(protect(self.request,0)),WRITE(protect(self.write,0)),LOG(protect(self.log,None))]
        self.bridge=Callbacks(123,c.addressof(self.host.context),*self.callbacks)
        self.keyboard=(c.c_ubyte*256)();self.assertTrue(user.GetKeyboardState(self.keyboard))
        self.assertTrue(user.SetKeyboardState((c.c_ubyte*256)()))

    def tearDown(self):
        if self.host.pending:release(self.host.pending);self.host.pending=None
        self.api.meltype_tsf_key_state_destroy(self.token)
        self.assertTrue(user.SetKeyboardState(self.keyboard))
        self.host.verify(self);self.assertEqual(self.errors,[])

    def update(self,owner,op,value):
        self.assertEqual(owner,123);self.assertGreaterEqual(self.host.context_refs,2)
        self.events.append(("update",op,value))
        if op==0:self.flags=(self.flags&~192)|(64 if value&1 else 0)|(128 if value&2 else 0)
        elif op==1:self.own=bool(value)
        elif op==2:self.flags&=~16
        elif op==6:
            self.assertFalse(self.own);self.flags|=2

    def edit(self,owner,op,cookie,context):
        self.assertEqual((owner,cookie,context),(123,7,c.addressof(self.host.context)))
        self.assertGreaterEqual(self.host.context_refs,4)
        self.events.append(("edit",op))
        if op==0:
            if self.on_secret:self.on_secret()
            return int(self.secret)
        if op in [1,2]:self.flags&=~1
        if op==3 and self.on_mode:self.on_mode()
        if op==4 and self.on_apply:self.on_apply()
        return 0

    def request(self,owner,text,reply):
        self.assertTrue(self.own);self.assertGreaterEqual(self.host.context_refs,5)
        self.requests.append(json.loads(text));self.events.append(("request",))
        reply[0]=self.reply
        if self.on_request:self.on_request()
        return int(self.responds)

    def write(self,owner,cookie,context,range,ch):
        self.assertTrue(self.own);self.assertGreaterEqual(self.host.context_refs,4)
        self.assertGreaterEqual(self.host.ranges[range][2],2)
        self.events.append(("write",ch))
        if not self.write_success:return 0
        value=W(ch)
        result=self.api.meltype_tsf_write_range(context,range,cookie,c.byref(value),1,0)
        if result:self.flags&=~1
        return result

    def log(self,owner,kind,code):self.events.append(("log",kind,code))

    def call(self,key=65,ch=97,context=True):
        return self.api.meltype_tsf_handle_key(c.byref(self.host.context) if context else None,17,self.token,key,ch,self.metadata,c.byref(self.bridge))

    def modifiers(self,*keys):
        value=(c.c_ubyte*256)()
        for key in keys:value[key]=0x80
        self.assertTrue(user.SetKeyboardState(value))

    def replayed(self):return any(e[:2]==("update",8) for e in self.events)

    def test_request_context_flags_unicode_and_apply_order(self):
        self.flags|=16|32
        self.assertEqual(self.call(),0)
        self.assertEqual(self.requests,[{"op":"key",**json.loads(self.metadata),"vk":65,"ch":97,"mods":0,
            "code":True,"direct":False,"canDelete":True,"moved":True,"private":True,"before":"前😀","after":"後"}])
        self.assertLess(self.events.index(("edit",3)),self.events.index(("edit",4)))
        self.assertEqual(self.host.locks,[7]);self.assertFalse(self.own);self.assertFalse(self.replayed())

    def test_composition_suppresses_surrounding_text_read(self):
        self.flags|=1
        self.call()
        self.assertTrue(self.requests[0]['composing']);self.assertNotIn('before',self.requests[0])
        self.assertEqual(self.host.read_count,0)

    def test_secret_during_composition_commits_then_passes_without_core(self):
        self.flags|=1|8
        self.call()
        self.assertIn(("edit",1),self.events);self.assertNotIn(("edit",0),self.events)
        self.assertEqual(self.requests,[]);self.assertEqual(self.host.read_count,0);self.assertTrue(self.replayed())

    def test_unchecked_secret_blocks_without_reading_text(self):
        self.flags=2;self.secret=True
        self.call()
        self.assertIn(("edit",0),self.events);self.assertEqual(self.requests,[])
        self.assertEqual(self.host.read_count,0);self.assertTrue(self.replayed())

    def test_privacy_query_refreshes_metadata_but_modifiers_are_captured_before_edit(self):
        self.flags=2;self.modifiers(0x10,0xa0)
        def changed():self.flags|=16|32;self.modifiers(0x11,0xa2)
        self.on_secret=changed
        self.call()
        self.assertEqual(self.requests[0]['mods'],1)
        self.assertTrue(self.requests[0]['private']);self.assertTrue(self.requests[0]['moved'])

    def test_transport_failure_inserts_printable_character_before_returning(self):
        self.responds=False;self.flags|=1
        self.call(ch=0x3042)
        self.assertLess(self.events.index(("edit",2)),self.events.index(("write",0x3042)))
        self.assertEqual(self.host.document,units('前😀あ後'))
        self.assertTrue(self.flags&64);self.assertFalse(self.replayed());self.assertFalse(self.own)

    def test_transport_failure_shortcuts_are_replayed_without_inserting(self):
        self.responds=False;self.modifiers(0x11,0xa2)
        self.call()
        self.assertEqual(self.host.writes,[]);self.assertTrue(self.replayed())

    def test_failed_selection_disables_deletion_and_fallback_insertion(self):
        self.host.fail_selection=True;self.responds=False
        self.call()
        self.assertFalse(self.requests[0]['canDelete']);self.assertEqual(self.host.writes,[])
        self.assertTrue(self.replayed())

    def test_nonempty_selection_cannot_enable_guarded_deletion(self):
        self.host.selection=(0,3);self.host.ranges[self.host.original][:2]=[0,3]
        self.call()
        self.assertFalse(self.requests[0]['canDelete'])
        self.assertEqual(self.host.document,units('前😀後'))

    def selection(self,text,start=0,end=None):
        self.host.document=units(text)
        self.host.selection=(start,len(self.host.document) if end is None else end)
        self.host.ranges[self.host.original][:2]=self.host.selection

    def test_convert_sends_bounded_selection_in_direct_mode(self):
        self.selection('前nihongo後',1,8);self.flags=4
        self.call(28,0)
        self.assertEqual(self.requests[0]['op'],'reconvert')
        self.assertEqual(self.requests[0]['text'],'nihongo')
        self.assertTrue(self.requests[0]['direct']);self.assertFalse(self.requests[0]['canDelete'])
        self.assertEqual((self.requests[0]['before'],self.requests[0]['after']),('前','後'))
        self.assertIn(('edit',4),self.events)

    def test_convert_reconverts_private_selection_but_not_caret_or_existing_composition(self):
        self.call(28,0);self.assertEqual(self.requests[-1]['op'],'key')
        self.selection('nihongo');self.flags|=32
        self.call(28,0);self.assertEqual(self.requests[-1]['op'],'reconvert')
        self.assertTrue(self.requests[-1]['private'])
        self.flags=(self.flags&~32)|1
        self.call(28,0);self.assertEqual(self.requests[-1]['op'],'key')

    def test_convert_rejects_overlong_unreadable_or_invalid_utf16_selection(self):
        for text in ['a'*1025,'\ud800']:
            self.selection(text);self.call(28,0)
            self.assertEqual(self.requests[-1]['op'],'key')
        self.selection('nihongo');self.host.fail_read=True
        self.call(28,0);self.assertEqual(self.requests[-1]['op'],'key')

    def test_changed_selection_resets_without_applying_or_replaying_convert(self):
        self.selection('go go',0,2)
        def moved():
            if self.requests[-1]['op']=='reconvert':
                self.host.selection=(3,5);self.host.ranges[self.host.original][:2]=[3,5]
        self.on_request=moved
        self.call(28,0)
        self.assertEqual([r['op'] for r in self.requests],['reconvert','reset'])
        self.assertFalse(self.replayed());self.assertTrue(self.flags&128)
        self.assertNotIn(('edit',3),self.events);self.assertNotIn(('edit',4),self.events)
        self.assertEqual(self.host.document,units('go go'));self.assertEqual(self.host.writes,[])

    def test_changed_selected_text_cancels_even_when_transport_failed(self):
        self.selection('nihongo');self.responds=False
        def changed():
            if self.requests[-1]['op']=='reconvert': self.host.document=units('changed')
        self.on_request=changed
        self.call(28,0)
        self.assertEqual([r['op'] for r in self.requests],['reconvert','reset'])
        self.assertFalse(self.replayed());self.assertNotIn(('edit',4),self.events)
        self.assertFalse(self.flags&64);self.assertEqual(self.host.writes,[])

    def test_mode_notification_cannot_retarget_to_identical_text(self):
        self.selection('go go',0,2)
        def moved():self.host.ranges[self.host.original][:2]=[3,5]
        self.on_mode=moved
        self.call(28,0)
        self.assertEqual([r['op'] for r in self.requests],['reconvert','reset'])
        self.assertNotIn(('edit',4),self.events);self.assertFalse(self.replayed())

    def test_nested_composition_is_not_reset_by_cancelled_reconversion(self):
        self.selection('nihongo')
        self.on_request=lambda:setattr(self,'flags',self.flags|1)
        self.call(28,0)
        self.assertEqual([r['op'] for r in self.requests],['reconvert'])
        self.assertTrue(self.flags&1);self.assertNotIn(('edit',4),self.events)
        self.assertFalse(self.replayed())

    def test_failed_core_and_failed_write_replay_once(self):
        self.responds=False;self.write_success=False
        self.call()
        self.assertEqual([e for e in self.events if e[:2]==('update',8)],[('update',8,65)])
        self.assertTrue(self.flags&64);self.assertFalse(self.own)

    def test_direct_unconsumed_ascii_is_inserted_without_replay(self):
        self.flags=4;self.reply=97
        self.call()
        self.assertEqual(self.host.document,units('前😀a後'));self.assertFalse(self.replayed())

    def test_direct_space_non_ascii_and_existing_output_do_not_insert_twice(self):
        for key,ch,reply in [(32,32,97),(65,0x3042,97),(65,97,33),(65,97,65)]:
            self.flags=4;self.reply=reply;self.events.clear();self.call(key,ch)
            self.assertTrue(self.replayed());self.assertEqual(self.host.writes,[])

    def test_failed_direct_write_replays_and_logs(self):
        self.flags=4;self.reply=97;self.write_success=False
        self.call()
        self.assertTrue(self.replayed());self.assertIn(("log",1,0),self.events)

    def test_switch_to_japanese_is_deferred_until_own_edit_clears(self):
        self.flags=4;self.reply=121
        self.call()
        self.assertIn(("update",6,1),self.events)
        self.assertLess(self.events.index(("update",1,0)),self.events.index(("update",6,1)))

    def test_inactive_response_clears_detection_and_refreshes_hello(self):
        self.flags|=1;self.reply=118
        self.call()
        self.assertIn(("update",3,0),self.events);self.assertIn(("update",5,0),self.events)

    def test_apply_failure_blocks_open_and_replays(self):
        self.flags=4;self.reply=121
        self.on_apply=lambda:setattr(self,'flags',(self.flags|64)&~128)
        self.call()
        self.assertNotIn(("update",6,1),self.events);self.assertTrue(self.replayed())

    def test_refused_edit_commits_old_composition_async_then_replays(self):
        self.host.mode='reject';self.flags|=1
        self.assertEqual(self.call(),FAIL)
        self.assertEqual(self.requests,[]);self.assertIn(("update",7,1),self.events);self.assertTrue(self.replayed())

    def test_delayed_sync_edit_is_cancelled_and_never_uses_stack_callbacks_later(self):
        self.host.mode='async'
        self.assertEqual(self.call(),HR(0x80004004).value)
        events=list(self.events)
        self.assertEqual(invoke(self.host.pending,3,HR,[U32],7),HR(0x80004004).value)
        self.assertEqual(self.events,events);self.assertEqual(self.requests,[]);self.assertTrue(self.replayed())
        release(self.host.pending);self.host.pending=None

    def test_request_can_release_original_owner_before_apply_and_cleanup(self):
        self.on_request=lambda:self.host.context_ref(-1)
        self.assertEqual(self.call(),0);self.assertEqual(self.host.context_refs,0)
        self.host.context_ref(1);self.assertFalse(self.own)

    def test_null_context_and_invalid_metadata(self):
        self.assertEqual(self.call(context=False),HR(0x80070057).value)
        self.assertTrue(self.replayed());self.events.clear();self.metadata=b'{"sid":17}'
        self.assertEqual(self.call(),HR(0x80070057).value);self.assertEqual(self.events,[])

    def test_passed_key_control_information_is_consumed_once(self):
        self.modifiers(0x11,0xa2)
        callbacks=[WOULD(lambda *a:0),MARK(lambda *a:None),COMPOSING(lambda *a:0),TOGGLE(lambda *a:None),HANDLE(lambda *a:None)]
        entry=Entry(123,c.addressof(self.host.context),*callbacks)
        self.api.meltype_tsf_key_event(self.token,1,c.byref(self.host.context),37,0,c.byref(entry))
        self.call();self.call()
        self.assertEqual(self.requests[0]['passed'],37);self.assertTrue(self.requests[0]['ctrl'])
        self.assertNotIn('passed',self.requests[1])

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
