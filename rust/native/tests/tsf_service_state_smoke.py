# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Native state layout/defaults, independent storage and balanced lifetimes."""
import ctypes as c
import json
import os
import tempfile
import time
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_com_smoke import Guid, HR, invoke, release
from tsf_startup_smoke import StartupHost
from tsf_candidate_smoke import windows, user
from tsf_candidate_view_smoke import GeometryHost
from tsf_abandoned_smoke import DeferredHost
SELECT=c.CFUNCTYPE(None,c.c_void_p,c.c_int,c.c_int)
READ=c.CFUNCTYPE(c.c_int,c.c_void_p)
TOGGLE=c.CFUNCTYPE(None,c.c_void_p)
class Point(c.Structure):
    _fields_=[('x',HR),('y',HR)]
P=c.c_void_p
U32=c.c_uint32
class Stale(c.Structure):
    _fields_=[("tries",U32),("key",U32),("time",c.c_uint64)]
class State(c.Structure):
    _fields_=[
        ('refs',c.c_int32),
        ('manager',P),
        ('client',U32),
        ('thread_subscription',P),
        ('key_subscription',P),
        ('mode_subscription',P),
        ('edit_subscription',P),
        ('edit_context',P),
        ('composition',P),
        ('composition_context',P),
        ('atom_input',U32),
        ('atom_converted',U32),
        ('atom_target',U32),
        ('keys',P),
        ('resume',P),
        ('abandoned',P),
        ('last_hello',c.c_uint64),
        ('focus_epoch',c.c_uint64),
        ('mode_epoch',c.c_uint64),
        ('shutdown_phase',U32),
        ('server_active',c.c_bool),
        ('direct_detection',c.c_bool),
        ('direct_word',c.c_bool),
        ('open',c.c_bool),
        ('code_app',c.c_bool),
        ('code_english',c.c_bool),
        ('disabled',c.c_bool),
        ('own_process',c.c_bool),
        ('last_converting',c.c_bool),
        ('caret_moved',c.c_bool),
        ('own_edit',c.c_bool),
        ('secret_checked',c.c_bool),
        ('secret_composing',c.c_bool),
        ('private_field',c.c_bool),
        ('keep_caret',c.c_bool),
        ('deactivated',c.c_bool),
        ('key_failed',c.c_bool),
        ('consumed',c.c_bool),
        ('scope_source',P),
        ('pass_reason',P),
        ('sid',P),
        ('transport',P),
        ('langbar',P),
        ('candidates',P),
    ]
class StateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.ole=c.WinDLL('ole32')
        if cls.ole.CoInitializeEx(None,2) not in (0,1):raise AssertionError('private COM apartment failed')
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_service_state_create.restype=P
        cls.api.meltype_tsf_service_state_destroy.argtypes=[P]
        cls.api.meltype_tsf_service_state_destroy.restype=None
        cls.api.meltype_tsf_service_state_layout.argtypes=[c.POINTER(U32),U32]
        cls.api.meltype_tsf_service_state_layout.restype=U32
        cls.api.meltype_tsf_service_state_live_count.restype=U32
        cls.api.meltype_tsf_service_set_sid.argtypes=[P,P];cls.api.meltype_tsf_service_set_sid.restype=c.c_int
        cls.api.meltype_tsf_service_sid.argtypes=[P];cls.api.meltype_tsf_service_sid.restype=P
        cls.api.meltype_tsf_service_request.argtypes=[P,c.c_char_p,c.c_char_p,U32,c.c_int];cls.api.meltype_tsf_service_request.restype=P
        cls.api.meltype_tsf_service_close.argtypes=[P];cls.api.meltype_tsf_service_close.restype=None
        cls.api.meltype_tsf_service_initialize.argtypes=[P,U32,c.c_int,c.c_int,c.c_char_p];cls.api.meltype_tsf_service_initialize.restype=c.c_int
        cls.api.meltype_tsf_service_clear_resume.argtypes=[P];cls.api.meltype_tsf_service_clear_resume.restype=None
        cls.api.meltype_tsf_key_state_create.restype=P
        cls.api.meltype_tsf_key_stale_write.argtypes=[P,U32,U32,c.c_uint64]
        cls.api.meltype_tsf_key_stale_read.argtypes=[P,c.POINTER(Stale)]
        cls.api.meltype_tsf_capture_resume.argtypes=[P,P,U32];cls.api.meltype_tsf_capture_resume.restype=P
        cls.api.meltype_tsf_abandoned_create.restype=P
        cls.api.meltype_tsf_abandoned_add.argtypes=[P,P,P,P,U32];cls.api.meltype_tsf_abandoned_add.restype=HR

        cls.api.meltype_tip_transport_request.argtypes=[P,c.c_char_p,U32,c.c_int];cls.api.meltype_tip_transport_request.restype=P
        cls.api.meltype_free.argtypes=[P]
        cls.api.meltype_tsf_service_create_langbar.argtypes=[P,P,READ,TOGGLE];cls.api.meltype_tsf_service_create_langbar.restype=P
        cls.api.meltype_tsf_service_remove_langbar.argtypes=[P,P];cls.api.meltype_tsf_service_remove_langbar.restype=None
        cls.api.meltype_tsf_service_update_langbar.argtypes=[P];cls.api.meltype_tsf_service_update_langbar.restype=None
        cls.api.meltype_tsf_service_create_candidates.argtypes=[P,P,SELECT];cls.api.meltype_tsf_service_create_candidates.restype=P
        cls.api.meltype_tsf_service_show_candidates.argtypes=[P,P,P,U32,c.c_char_p];cls.api.meltype_tsf_service_show_candidates.restype=c.c_int
        for operation in ['hide','drop']:
            getattr(cls.api,'meltype_tsf_service_'+operation+'_candidates').argtypes=[P]
            getattr(cls.api,'meltype_tsf_service_'+operation+'_candidates').restype=None
        cls.api.meltype_tsf_candidate_visible.argtypes=[P];cls.api.meltype_tsf_candidate_visible.restype=c.c_int
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32];cls.api.meltype_tsf_start_composition.restype=P

        cls.worker=str(Path(sys.argv[1]).resolve().with_name('meltype-tip-worker.exe')).encode()

        size=cls.api.meltype_tsf_service_state_layout(None,0)
        metadata=(U32*size)();cls.api.meltype_tsf_service_state_layout(metadata,size)
        expected=[4,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
        if list(metadata)!=expected:raise AssertionError('State ABI mismatch; do not borrow this allocation')
    @classmethod
    def tearDownClass(cls):cls.ole.CoUninitialize()
    def setUp(self):
        self.owned=[];self.callbacks=[];self.before=self.live()
        self.directory=tempfile.TemporaryDirectory(prefix='meltype-service-state-')
        self.old_data=os.environ.get('MELTYPE_RUST_DATA_DIR')
        os.environ['MELTYPE_RUST_DATA_DIR']=self.directory.name

    def tearDown(self):
        for data in self.owned:self.api.meltype_tsf_service_state_destroy(data)
        self.assertEqual(self.live(),self.before)
        if self.old_data is None:os.environ.pop('MELTYPE_RUST_DATA_DIR',None)
        else:os.environ['MELTYPE_RUST_DATA_DIR']=self.old_data
        deadline=time.monotonic()+3
        while True:
            try:self.directory.cleanup();break
            except PermissionError:
                if time.monotonic()>=deadline:raise
                time.sleep(.02)
    def live(self):return self.api.meltype_tsf_service_state_live_count()
    def create(self):
        data=self.api.meltype_tsf_service_state_create()
        self.assertTrue(data);self.owned.append(data);return State.from_address(data)
    def test_layout_matches_all_fields_before_borrowing(self):
        size=self.api.meltype_tsf_service_state_layout(None,0)
        result=(U32*size)();self.assertEqual(self.api.meltype_tsf_service_state_layout(result,size),size)
        expected=[4,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
        self.assertEqual(list(result),expected)
    def test_short_metadata_buffer_is_untouched(self):
        size=self.api.meltype_tsf_service_state_layout(None,0);result=(U32*(size+2))(*([0xaabbccdd]*(size+2)))
        self.assertEqual(self.api.meltype_tsf_service_state_layout(result,size-1),size)
        self.assertEqual(list(result),[0xaabbccdd]*(size+2))
    def test_defaults_match_upstream_service(self):
        state=self.create()
        for name,kind in State._fields_:
            with self.subTest(field=name):
                expected=1 if name=='refs' else name in ['open','consumed'] if kind==c.c_bool else None if kind==P else 0
                self.assertEqual(getattr(state,name),expected)
    def test_independent_instances_and_64bit_times(self):
        first=self.create();second=self.create();self.assertEqual(self.live(),self.before+2)
        first.server_active=True;first.open=False;first.last_hello=0xfedcba9876543210;first.focus_epoch=0x100000001
        self.assertFalse(second.server_active);self.assertTrue(second.open);self.assertEqual(second.last_hello,0)
        self.assertEqual(first.last_hello,0xfedcba9876543210);self.assertEqual(first.focus_epoch,0x100000001)
    def test_null_destroy_is_inert(self):
        self.api.meltype_tsf_service_state_destroy(None);self.assertEqual(self.live(),self.before)
    def test_independent_ui_threads_balance_storage(self):
        errors=[]
        def work():
            try:
                for _ in range(100):
                    data=self.api.meltype_tsf_service_state_create()
                    if not data:raise AssertionError('allocation failed')
                    state=State.from_address(data);state.open=False;state.mode_epoch=123456789
                    self.api.meltype_tsf_service_state_destroy(data)
            except BaseException as e:errors.append(repr(e))
        threads=[threading.Thread(target=work) for _ in range(4)]
        for thread in threads:thread.start()
        for thread in threads:thread.join()
        self.assertEqual(errors,[]);self.assertEqual(self.live(),self.before)
    def test_sid_copy_self_assignment_replace_and_independence(self):
        first=self.create();second=self.create();address=c.addressof(first)
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(address)),b'')
        source=c.create_string_buffer('日本語-session'.encode())
        self.assertEqual(self.api.meltype_tsf_service_set_sid(address,source),1)
        source[0]=b'X'
        borrowed=self.api.meltype_tsf_service_sid(address)
        self.assertEqual(c.string_at(borrowed),'日本語-session'.encode())
        self.assertEqual(self.api.meltype_tsf_service_set_sid(address,borrowed),1)
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(address)),'日本語-session'.encode())
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(c.addressof(second))),b'')
        self.assertEqual(self.api.meltype_tsf_service_set_sid(address,c.c_char_p(b'new')),1)
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(address)),b'new')
        self.assertEqual(self.api.meltype_tsf_service_set_sid(address,c.c_char_p(b'')),1)
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(address)),b'')
    def request(self,state,value):
        raw=self.api.meltype_tsf_service_request(c.addressof(state),self.worker,json.dumps(value).encode(),5000,1)
        self.assertTrue(raw)
        try:return json.loads(c.string_at(raw))
        finally:self.api.meltype_free(raw)
    def test_transport_owned_by_state_closes_and_reopens(self):
        state=self.create();self.assertFalse(state.transport)
        self.request(state,{'op':'hello','process':'test'})
        old=state.transport;self.assertTrue(old)
        self.request(state,{'op':'hello','process':'test'})
        self.assertEqual(state.transport,old)
        self.api.meltype_tsf_service_close(c.addressof(state));self.assertFalse(state.transport)
        self.assertFalse(self.api.meltype_tip_transport_request(old,b'{}',100,1))
        self.api.meltype_tsf_service_close(c.addressof(state))
        self.request(state,{'op':'hello','process':'test'})
        self.assertTrue(state.transport);self.assertNotEqual(state.transport,old)
    def test_destroy_retires_connection_without_explicit_close(self):
        first=self.create();second=self.create()
        self.request(first,{'op':'hello','process':'first'})
        self.request(second,{'op':'hello','process':'second'})
        old=first.transport;self.assertNotEqual(old,second.transport)
        address=c.addressof(first);self.owned.remove(address)
        self.api.meltype_tsf_service_state_destroy(address)
        self.assertFalse(self.api.meltype_tip_transport_request(old,b'{}',100,1))
        self.request(second,{'op':'hello','process':'second'})
    def test_null_resource_arguments_are_inert(self):
        state=self.create();address=c.addressof(state)
        self.assertEqual(self.api.meltype_tsf_service_set_sid(None,c.c_char_p(b'test')),0)
        self.assertEqual(self.api.meltype_tsf_service_set_sid(address,None),0)
        self.assertFalse(self.api.meltype_tsf_service_sid(None))
        self.api.meltype_tsf_service_close(None)
        for owner,worker,request in [(None,self.worker,b'{}'),(address,None,b'{}'),(address,self.worker,None)]:
            self.assertFalse(self.api.meltype_tsf_service_request(owner,worker,request,10,1))
        self.assertFalse(state.transport)
    def create_bar(self,state,mode=None):
        mode=mode if mode is not None else [False]
        @READ
        def read(owner):return int(mode[0])
        @TOGGLE
        def toggle(owner):mode[0]=not mode[0]
        self.callbacks.extend([read,toggle])
        bar=self.api.meltype_tsf_service_create_langbar(c.addressof(state),123,read,toggle)
        self.assertTrue(bar);return bar,mode
    def bar_text(self,bar):
        text=P();self.assertEqual(invoke(bar,11,HR,[c.POINTER(P)],c.byref(text)),0)
        allocator=c.WinDLL('oleaut32');allocator.SysFreeString.argtypes=[P]
        try:return c.wstring_at(text)
        finally:allocator.SysFreeString(text)
    def click_bar(self,bar):self.assertEqual(invoke(bar,7,HR,[HR,Point,P],0,Point(),None),0)
    def test_bar_replacement_detaches_retained_old_reference(self):
        state=self.create();old,mode=self.create_bar(state)
        invoke(old,1,U32,[]);self.assertEqual(self.bar_text(old),'A')
        current,_=self.create_bar(state,[True]);self.assertEqual(state.langbar,current)
        self.click_bar(old);self.assertFalse(mode[0]);self.assertEqual(self.bar_text(old),'あ')
        self.assertEqual(release(old),0)
        self.api.meltype_tsf_service_remove_langbar(c.addressof(state),None)
        self.assertFalse(state.langbar)
        self.api.meltype_tsf_service_remove_langbar(c.addressof(state),None)
    def test_bar_destruction_detaches_context_from_retained_button(self):
        state=self.create();bar,mode=self.create_bar(state);invoke(bar,1,U32,[])
        address=c.addressof(state);self.owned.remove(address)
        self.api.meltype_tsf_service_state_destroy(address)
        self.click_bar(bar);self.assertFalse(mode[0]);self.assertEqual(self.bar_text(bar),'あ')
        self.assertEqual(release(bar),0)
    def test_bar_manager_removal_preserves_reentrant_new_bar(self):
        state=self.create();old,mode=self.create_bar(state);invoke(old,1,U32,[])
        host=StartupHost();removed=[];replacement=[]
        def remove(_,bar):
            removed.append(bar);self.assertFalse(state.langbar)
            self.click_bar(bar);self.assertFalse(mode[0])
            replacement.append(self.create_bar(state,[True])[0]);return 0
        host.bind(host.bar_manager_table,6,HR,[P],remove)
        self.api.meltype_tsf_service_remove_langbar(c.addressof(state),c.addressof(host.context))
        self.assertEqual(removed,[old]);self.assertEqual(state.langbar,replacement[0])
        self.assertEqual(host.bar_manager_refs,1);self.assertEqual(release(old),0)
    def test_bar_failed_manager_query_releases_partial_reference(self):
        state=self.create();self.create_bar(state)
        host=StartupHost();host.fail_bar_query=host.partial_bar_query=True
        self.api.meltype_tsf_service_remove_langbar(c.addressof(state),c.addressof(host.context))
        self.assertFalse(state.langbar);self.assertEqual(host.bar_manager_refs,1)
    def advise_bar(self,bar,on_update=None,on_drop=None):
        refs=[1];events=[]
        class Sink(c.Structure):_fields_=[('table',c.POINTER(P))]
        @c.WINFUNCTYPE(HR,P,c.POINTER(Guid),c.POINTER(P))
        def query(_,iid,out):refs[0]+=1;out[0]=c.addressof(sink);return 0
        @c.WINFUNCTYPE(U32,P)
        def add(_):refs[0]+=1;return refs[0]
        @c.WINFUNCTYPE(U32,P)
        def drop(_):
            refs[0]-=1
            if on_drop:on_drop(refs[0])
            return refs[0]
        @c.WINFUNCTYPE(HR,P,U32)
        def update(_,flags):
            events.append(flags)
            if on_update:on_update()
            return 0
        table=(P*4)(*[c.cast(fn,P).value for fn in [query,add,drop,update]])
        sink=Sink(table);self.callbacks.extend([query,add,drop,update,table,sink])
        source=P();source_id=Guid.parse('4ea48a35-60ae-446f-8fd6-e6a8d82459f7')
        sink_id=Guid.parse('57dbe1a0-de25-11d2-afdd-00105a2799b5')
        self.assertEqual(invoke(bar,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(source_id),c.byref(source)),0)
        cookie=U32()
        self.assertEqual(invoke(source,3,HR,[c.POINTER(Guid),P,c.POINTER(U32)],c.byref(sink_id),c.byref(sink),c.byref(cookie)),0)
        release(source);return refs,events
    def test_bar_update_survives_service_destruction_in_notification(self):
        state=self.create();bar,_=self.create_bar(state);address=c.addressof(state)
        def destroy():
            self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        refs,events=self.advise_bar(bar,on_update=destroy)
        self.api.meltype_tsf_service_update_langbar(address)
        self.assertEqual(events,[7]);self.assertEqual(refs,[1]);self.assertEqual(self.live(),self.before)
    def test_bar_create_detects_reentrant_replacement_from_sink_release(self):
        state=self.create();old,_=self.create_bar(state);replacement=[]
        def released(refs):
            if refs==1:replacement.append(self.create_bar(state,[True])[0])
        refs,_=self.advise_bar(old,on_drop=released)
        read=READ(lambda _:0);toggle=TOGGLE(lambda _:None);self.callbacks.extend([read,toggle])
        result=self.api.meltype_tsf_service_create_langbar(c.addressof(state),123,read,toggle)
        self.assertFalse(result);self.assertEqual(len(replacement),1)
        self.assertEqual(state.langbar,replacement[0]);self.assertEqual(refs,[1])
    def test_bar_stopping_and_null_state_are_inert(self):
        state=self.create();state.shutdown_phase=1
        read=READ(lambda _:0);toggle=TOGGLE(lambda _:None)
        self.assertFalse(self.api.meltype_tsf_service_create_langbar(c.addressof(state),None,read,toggle))
        self.assertFalse(self.api.meltype_tsf_service_create_langbar(None,None,read,toggle))
        self.api.meltype_tsf_service_update_langbar(None);self.api.meltype_tsf_service_remove_langbar(None,None)
        self.assertFalse(state.langbar)
    def create_candidates(self,state,on_select=None):
        events=[]
        @SELECT
        def select(owner,index,prediction):
            events.append((owner,index,prediction))
            if on_select:on_select()
        self.callbacks.append(select)
        token=self.api.meltype_tsf_service_create_candidates(c.addressof(state),456,select)
        self.assertTrue(token);self.assertEqual(state.candidates,token);return token,events
    def show_candidates(self,state,context=None,composition=None,view=None):
        view=view or {'predictions':['日本','😀'],'selectedPrediction':1}
        return self.api.meltype_tsf_service_show_candidates(c.addressof(state),context,composition,7,json.dumps(view,ensure_ascii=False).encode())
    def candidate_windows(self):return windows('MeltypeRustCandidateWindow')
    def click_candidate(self,window):
        scale=user.GetDpiForWindow(window)/96
        user.SendMessageW(window,0x202,0,(int(52*scale)<<16)|10)
    def test_candidates_hide_show_drop_and_stale_window(self):
        state=self.create();token,events=self.create_candidates(state)
        self.assertEqual(self.show_candidates(state),1);self.assertTrue(self.api.meltype_tsf_candidate_visible(token))
        window=self.candidate_windows()[0]
        self.api.meltype_tsf_service_hide_candidates(c.addressof(state))
        self.assertEqual(state.candidates,token);self.assertFalse(self.api.meltype_tsf_candidate_visible(token))
        self.assertEqual(self.show_candidates(state),1)
        self.api.meltype_tsf_service_drop_candidates(c.addressof(state))
        self.assertFalse(state.candidates);self.assertFalse(self.api.meltype_tsf_candidate_visible(token))
        user.SendMessageW(window,0x202,0,10|(52<<16));self.assertEqual(events,[])
        self.api.meltype_tsf_service_drop_candidates(c.addressof(state))
        self.assertFalse(self.candidate_windows());self.assertEqual(self.show_candidates(state),0)
    def test_candidates_replacement_retires_old_window(self):
        state=self.create();old,_=self.create_candidates(state);self.assertEqual(self.show_candidates(state),1)
        current,_=self.create_candidates(state);self.assertNotEqual(old,current)
        self.assertFalse(self.api.meltype_tsf_candidate_visible(old));self.assertFalse(self.candidate_windows())
        self.assertEqual(self.show_candidates(state),1)
    def test_candidates_click_can_destroy_own_service(self):
        state=self.create();address=c.addressof(state)
        def destroy():
            self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        token,events=self.create_candidates(state,destroy);self.assertEqual(self.show_candidates(state),1)
        self.click_candidate(self.candidate_windows()[0])
        self.assertEqual(events,[(456,1,1)]);self.assertFalse(self.candidate_windows())
        self.assertFalse(self.api.meltype_tsf_candidate_visible(token));self.assertEqual(self.live(),self.before)
    def test_candidates_anchor_callback_can_destroy_service(self):
        state=self.create();address=c.addressof(state);token,_=self.create_candidates(state)
        host=GeometryHost()
        composition=self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7)
        self.assertTrue(composition)
        def destroy():
            host.on_extent=None;self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        host.on_extent=destroy
        try:
            result=self.show_candidates(state,c.byref(host.context),composition,{'converting':True,'candidates':['前','文節'],'clauses':['前','😀','文節','後'],'selectedClause':2})
            self.assertEqual(result,0);self.assertEqual(self.live(),self.before)
            self.assertFalse(self.api.meltype_tsf_candidate_visible(token));self.assertFalse(self.candidate_windows())
        finally:release(composition)
        host.verify(self)
    def test_candidates_independent_service_owners(self):
        first=self.create();second=self.create();old,_=self.create_candidates(first);current,_=self.create_candidates(second)
        self.assertNotEqual(old,current);self.assertEqual(self.show_candidates(first),1);self.assertEqual(self.show_candidates(second),1)
        self.assertEqual(len(self.candidate_windows()),2)
        address=c.addressof(first);self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        self.assertFalse(self.api.meltype_tsf_candidate_visible(old));self.assertTrue(self.api.meltype_tsf_candidate_visible(current))
        self.assertEqual(len(self.candidate_windows()),1)
    def test_candidates_null_and_stopping_state_are_inert(self):
        state=self.create();state.shutdown_phase=1;select=SELECT(lambda *_:None)
        self.assertFalse(self.api.meltype_tsf_service_create_candidates(c.addressof(state),None,select))
        self.assertFalse(self.api.meltype_tsf_service_create_candidates(None,None,select))
        self.assertEqual(self.api.meltype_tsf_service_show_candidates(None,None,None,7,b'{}'),0)
        self.api.meltype_tsf_service_hide_candidates(None);self.api.meltype_tsf_service_drop_candidates(None)
        self.assertFalse(state.candidates)
    def test_initialize_matches_exact_upstream_reset_and_preserves_other_fields(self):
        state=self.create();address=c.addressof(state)
        for name,kind in State._fields_:
            if kind==c.c_bool:setattr(state,name,True)
        state.mode_epoch=0xffffffffffffffff;state.last_hello=1234;state.client=99
        state.focus_epoch=777;state.shutdown_phase=0;state.pass_reason=1234;state.scope_source=5678
        before={name:getattr(state,name) for name,_ in State._fields_}
        self.assertEqual(self.api.meltype_tsf_service_initialize(address,17,0,-1,b'new-session'),1)
        changed={'mode_epoch':0,'client':17,'deactivated':False,'server_active':False,'direct_detection':False,
                 'direct_word':False,'last_hello':0,'caret_moved':False,'own_edit':False,'secret_checked':False,
                 'last_converting':False,'pass_reason':None,'key_failed':False,'consumed':True,'own_process':True,'disabled':False}
        for name,_ in State._fields_:
            if name=='sid':continue
            with self.subTest(field=name):self.assertEqual(getattr(state,name),changed.get(name,before[name]))
        self.assertEqual(c.string_at(self.api.meltype_tsf_service_sid(address)),b'new-session')
        self.assertEqual(self.api.meltype_tsf_service_initialize(address,21,1,0,b'second'),1)
        self.assertEqual((state.client,state.mode_epoch,state.disabled,state.own_process),(21,1,True,False))
    def test_initialize_null_arguments_leave_all_state_unchanged(self):
        state=self.create();address=c.addressof(state);before=c.string_at(address,c.sizeof(State))
        self.assertEqual(self.api.meltype_tsf_service_initialize(address,17,1,1,None),0)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before)
        self.assertEqual(self.api.meltype_tsf_service_initialize(None,17,1,1,b'test'),0)
    def capture_resume(self,host):
        composition=self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7)
        self.assertTrue(composition)
        try:token=self.api.meltype_tsf_capture_resume(c.byref(host.context),composition,7)
        finally:release(composition)
        self.assertTrue(token);return token
    def test_clear_resume_detaches_and_releases_owned_com_references(self):
        state=self.create();host=GeometryHost();state.resume=self.capture_resume(host)
        self.assertGreater(host.context_refs,1)
        self.api.meltype_tsf_service_clear_resume(c.addressof(state))
        self.assertFalse(state.resume);host.verify(self)
        self.api.meltype_tsf_service_clear_resume(c.addressof(state));self.api.meltype_tsf_service_clear_resume(None)
    def test_clear_resume_release_can_destroy_service_after_slot_detach(self):
        state=self.create();host=GeometryHost();state.resume=self.capture_resume(host)
        address=c.addressof(state);original=host.context_ref;events=[]
        def reference(delta):
            if delta==-1 and not events:
                events.append(state.resume);self.assertFalse(state.resume)
                self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
            return original(delta)
        host.context_ref=reference
        self.api.meltype_tsf_service_clear_resume(address)
        self.assertEqual(events,[None]);self.assertEqual(self.live(),self.before);host.verify(self)
    def test_destroy_owns_key_resume_and_abandoned_token_cleanup(self):
        state=self.create();address=c.addressof(state)
        keys=self.api.meltype_tsf_key_state_create();self.assertTrue(keys);state.keys=keys
        self.api.meltype_tsf_key_stale_write(keys,8,65,123456789)
        stale=Stale();self.api.meltype_tsf_key_stale_read(keys,c.byref(stale));self.assertEqual(stale.tries,8)
        resume_host=GeometryHost();state.resume=self.capture_resume(resume_host)
        abandoned_host=DeferredHost()
        composition=self.api.meltype_tsf_start_composition(c.byref(abandoned_host.context),c.byref(abandoned_host.sink),abandoned_host.original,7)
        self.assertTrue(composition)
        try:
            state.abandoned=self.api.meltype_tsf_abandoned_create();self.assertTrue(state.abandoned)
            self.assertEqual(self.api.meltype_tsf_abandoned_add(state.abandoned,c.byref(abandoned_host.context),composition,c.byref(abandoned_host.context),17),262912)
        finally:release(composition)
        self.assertGreater(resume_host.context_refs,1);self.assertGreater(abandoned_host.context_refs,1)
        self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        self.api.meltype_tsf_key_stale_read(keys,c.byref(stale));self.assertEqual((stale.tries,stale.key,stale.time),(0,0,0))
        resume_host.verify(self)
        # A scheduled edit still pins its COM inputs until the host cancels it.
        self.assertGreater(abandoned_host.context_refs,1)
        abandoned_host.cancel();abandoned_host.verify(self)
if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)

