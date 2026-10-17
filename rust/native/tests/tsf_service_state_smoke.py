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
from tsf_subscriptions_smoke import SubscriptionHost
from tsf_candidate_smoke import windows, user
from tsf_candidate_view_smoke import GeometryHost
from tsf_abandoned_smoke import DeferredHost
from tsf_fields_smoke import CompartmentHost,OPEN
from tsf_service_com_smoke import DISPATCH,DESTROY
from tsf_edit_events_smoke import EventHost,QUERY
from tsf_open_change_smoke import Host as OpenHost
from tsf_shutdown_smoke import REQUEST as SHUTDOWN_REQUEST
SELECT=c.CFUNCTYPE(None,c.c_void_p,c.c_int,c.c_int)
CORE_NOTICE=c.CFUNCTYPE(None,c.c_void_p,c.c_uint32,c.c_int)
END=c.CFUNCTYPE(None,c.c_void_p,c.c_uint32)
FOCUS_COMMIT=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_int)
REPLY=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_void_p,c.c_uint32)
CORE_CLOCK=c.CFUNCTYPE(c.c_uint64,c.c_void_p)
NOTICE=c.CFUNCTYPE(None,c.c_void_p,c.c_int)
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
        ('edit_epoch',c.c_uint64),
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
        cls.api.meltype_tsf_service_edit_sink.argtypes=[P,P,P,P];cls.api.meltype_tsf_service_edit_sink.restype=HR
        cls.api.meltype_tsf_service_mode.argtypes=[P];cls.api.meltype_tsf_service_mode.restype=U32
        cls.api.meltype_tsf_service_write_open.argtypes=[P,c.c_int];cls.api.meltype_tsf_service_write_open.restype=c.c_int
        cls.api.meltype_tsf_service_apply_mode.argtypes=[P,U32,P,P,NOTICE];cls.api.meltype_tsf_service_apply_mode.restype=c.c_int
        cls.api.meltype_tsf_service_com_create.argtypes=[P,DISPATCH,DESTROY];cls.api.meltype_tsf_service_com_create.restype=P
        cls.api.meltype_tsf_service_core.argtypes=[P,c.c_char_p,P,P,CORE_NOTICE,CORE_CLOCK,c.c_int];cls.api.meltype_tsf_service_core.restype=c.c_int
        cls.api.meltype_tsf_service_exchange.argtypes=[P,c.c_char_p,P,P,U32,U32,c.c_int,P,REPLY,TOGGLE];cls.api.meltype_tsf_service_exchange.restype=c.c_int
        cls.api.meltype_tsf_service_focus.argtypes=[P,c.c_char_p,P,P,P,c.c_int,P,FOCUS_COMMIT,TOGGLE,CORE_NOTICE];cls.api.meltype_tsf_service_focus.restype=HR
        cls.api.meltype_tsf_service_edited.argtypes=[P,P,P,P,U32,P,QUERY,TOGGLE];cls.api.meltype_tsf_service_edited.restype=c.c_int
        cls.api.meltype_tsf_start_composition.argtypes=[P,P,P,U32];cls.api.meltype_tsf_start_composition.restype=P
        cls.api.meltype_tsf_service_opened.argtypes=[P,P,c.POINTER(Guid),P,TOGGLE,NOTICE];cls.api.meltype_tsf_service_opened.restype=HR
        cls.api.meltype_tsf_service_deactivate.argtypes=[P,P,P,SHUTDOWN_REQUEST,END,NOTICE];cls.api.meltype_tsf_service_deactivate.restype=HR
        cls.api.meltype_tsf_end_composition.argtypes=[P,P,U32];cls.api.meltype_tsf_end_composition.restype=HR
        cls.fixture=str(Path(sys.argv[2]).resolve()).encode() if len(sys.argv)>2 else None


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
        expected=[5,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
        if list(metadata)!=expected:raise AssertionError('State ABI mismatch; do not borrow this allocation')
    @classmethod
    def tearDownClass(cls):cls.ole.CoUninitialize()
    def setUp(self):
        self.owned=[];self.callbacks=[];self.before=self.live();self.callback_errors=[]
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
        self.assertEqual(self.callback_errors,[])
    def checked(self,kind):
        # ctypes otherwise prints and swallows exceptions from native callbacks.
        def decorate(fn):
            def guarded(*args):
                try:return fn(*args)
                except BaseException as error:
                    self.callback_errors.append((fn.__name__,repr(error)));return 0
            return kind(guarded)
        return decorate
    def live(self):return self.api.meltype_tsf_service_state_live_count()
    def create(self):
        data=self.api.meltype_tsf_service_state_create()
        self.assertTrue(data);self.owned.append(data);return State.from_address(data)
    def test_layout_matches_all_fields_before_borrowing(self):
        size=self.api.meltype_tsf_service_state_layout(None,0)
        result=(U32*size)();self.assertEqual(self.api.meltype_tsf_service_state_layout(result,size),size)
        expected=[5,c.sizeof(State),c.alignment(State)]+[getattr(State,name).offset for name,_ in State._fields_]
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
        @self.checked(TOGGLE)
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
    def edit_sink(self,state,host,document=True):
        return self.api.meltype_tsf_service_edit_sink(c.addressof(state),c.byref(host.document_manager) if document else None,c.byref(host.context),c.byref(host.sink) if document else None)
    def test_edit_sink_owns_source_context_and_resets_only_nonnull_bind(self):
        state=self.create();host=SubscriptionHost();state.client=17;state.own_edit=True
        try:
            self.assertEqual(self.edit_sink(state,host),0);self.assertTrue(state.edit_subscription)
            self.assertEqual(state.edit_context,c.addressof(host.context));self.assertFalse(state.own_edit)
            state.own_edit=True;self.assertEqual(self.edit_sink(state,host,False),0)
            self.assertFalse(state.edit_subscription);self.assertFalse(state.edit_context);self.assertTrue(state.own_edit)
        finally:self.edit_sink(state,host,False)
        host.verify(self)
    def test_edit_sink_reentrant_unadvise_preserves_new_binding(self):
        state=self.create();first=SubscriptionHost();second=SubscriptionHost();state.client=17
        self.edit_sink(state,first);old=state.edit_subscription
        first.sources[0].on_unadvise=lambda:self.edit_sink(state,second)
        try:
            self.assertEqual(self.edit_sink(state,first),0)
            self.assertNotEqual(state.edit_subscription,old);self.assertEqual(state.edit_context,c.addressof(second.context))
            self.assertFalse(first.sources[0].registrations);self.assertEqual(len(second.sources[0].registrations),1)
        finally:self.edit_sink(state,second,False)
        first.verify(self);second.verify(self)
    def test_edit_sink_reentrant_advise_discards_late_old_registration(self):
        state=self.create();first=SubscriptionHost();second=SubscriptionHost();state.client=17
        original=first.sources[0].advise
        def advise(this,iid,sink,cookie):
            result=original(this,iid,sink,cookie);self.edit_sink(state,second);return result
        first.bind(first.sources[0].table,3,HR,[c.POINTER(Guid),P,c.POINTER(U32)],advise)
        try:
            self.assertEqual(self.edit_sink(state,first),0)
            self.assertEqual(state.edit_context,c.addressof(second.context));self.assertFalse(first.sources[0].registrations)
            self.assertEqual(len(second.sources[0].registrations),1)
        finally:self.edit_sink(state,second,False)
        first.verify(self);second.verify(self)
    def test_edit_sink_focus_epoch_or_shutdown_during_advise_retires_late_token(self):
        for kind in ['focus','shutdown']:
            with self.subTest(kind=kind):
                state=self.create();host=SubscriptionHost();state.client=17;original=host.sources[0].advise
                def advise(this,iid,sink,cookie):
                    result=original(this,iid,sink,cookie)
                    if kind=='focus':state.focus_epoch+=1
                    else:state.shutdown_phase=1;state.deactivated=True
                    return result
                host.bind(host.sources[0].table,3,HR,[c.POINTER(Guid),P,c.POINTER(U32)],advise)
                self.assertEqual(self.edit_sink(state,host),0)
                self.assertFalse(state.edit_subscription);self.assertFalse(state.edit_context);host.verify(self)
    def test_edit_sink_partial_failures_release_all_references(self):
        for kind in ['top','source','advise']:
            with self.subTest(kind=kind):
                state=self.create();host=SubscriptionHost();state.client=17;state.own_edit=True
                if kind=='top':host.fail_top=host.partial_top=True
                elif kind=='source':host.fail_source=host.partial_source=True
                else:host.sources[0].fail=host.sources[0].partial=True
                self.assertEqual(self.edit_sink(state,host),0)
                self.assertFalse(state.edit_subscription);self.assertFalse(state.edit_context);self.assertFalse(state.own_edit)
                host.verify(self)
    def test_edit_sink_reentrant_detach_keeps_slots_empty(self):
        state=self.create();host=SubscriptionHost();state.client=17;state.edit_epoch=0xffffffffffffffff
        self.edit_sink(state,host);self.assertEqual(state.edit_epoch,0)
        def detached():
            self.assertFalse(state.edit_subscription);self.assertFalse(state.edit_context)
            self.edit_sink(state,host,False)
        host.sources[0].on_unadvise=detached
        self.assertEqual(self.edit_sink(state,host,False),0)
        self.assertFalse(state.edit_subscription);self.assertFalse(state.edit_context);host.verify(self)
    def test_edit_sink_invalid_arguments_preserve_existing_binding(self):
        state=self.create();host=SubscriptionHost();state.client=17;self.edit_sink(state,host)
        old=state.edit_subscription
        try:
            for owner,document,lifetime,sink in [(None,None,c.byref(host.context),None),(c.addressof(state),None,None,None),
                (c.addressof(state),c.byref(host.document_manager),c.byref(host.context),None)]:
                self.assertEqual(self.api.meltype_tsf_service_edit_sink(owner,document,lifetime,sink),HR(0x80070057).value)
                self.assertEqual(state.edit_subscription,old)
        finally:self.edit_sink(state,host,False)
        host.verify(self)
    def test_mode_all_state_reply_combinations_preserve_unrelated_fields(self):
        state=self.create();host=SubscriptionHost();address=c.addressof(state);notices=[]
        @NOTICE
        def notice(owner,english):
            self.assertEqual(owner,123);self.assertEqual(state.code_english,bool(english))
            notices.append(english)
        for before in range(16):
            for reply in range(32):
                state.open=bool(before&1);state.code_app=bool(before&2);state.code_english=bool(before&4);state.server_active=bool(before&8)
                self.assertEqual(self.api.meltype_tsf_service_mode(address),before)
                notices.clear();code=reply&3==3;english=code and bool(reply&4)
                self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,reply,c.byref(host.context),123,notice),1)
                expected=(before&9)|(2 if code else 0)|(4 if english else 0)
                self.assertEqual(self.api.meltype_tsf_service_mode(address),expected)
                self.assertEqual(notices,[int(english)] if bool(before&4)!=english else [])
        host.verify(self)
    def test_mode_state_is_published_before_notice_and_bar_update(self):
        state=self.create();host=SubscriptionHost();address=c.addressof(state);events=[]
        bar,_=self.create_bar(state)
        refs,updates=self.advise_bar(bar,on_update=lambda:events.append(('ui',state.code_app,state.code_english)))
        @NOTICE
        def notice(_,english):events.append(('notice',state.code_app,state.code_english))
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,7,c.byref(host.context),None,notice),1)
        self.assertEqual(events,[('notice',True,True),('ui',True,True)])
        self.assertEqual(updates,[7]);events.clear()
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,7,c.byref(host.context),None,notice),1)
        self.assertEqual(events,[])
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,0,c.byref(host.context),None,NOTICE()),1)
        self.assertEqual(events,[('ui',False,False)])
        self.api.meltype_tsf_service_remove_langbar(address,None);self.assertEqual(refs,[1]);host.verify(self)
    def test_mode_dispatch_pin_keeps_service_alive_through_notice_and_ui(self):
        state=self.create();address=c.addressof(state);destroyed=[];root=[None];events=[]
        @DISPATCH
        def dispatch(_owner,_call):return 0
        @self.checked(DESTROY)
        def destroy(_owner):
            self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address);destroyed.append(True)
        root[0]=self.api.meltype_tsf_service_com_create(None,dispatch,destroy);self.assertTrue(root[0])
        bar,_=self.create_bar(state)
        refs,updates=self.advise_bar(bar,on_update=lambda:events.append(('ui',self.live())))
        @NOTICE
        def notice(_,english):
            old=root[0];root[0]=None;self.assertEqual(release(old),1)
            self.assertEqual(destroyed,[]);events.append(('notice',self.live()))
        try:self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,7,root[0],None,notice),1)
        finally:
            if root[0]:release(root[0])
        self.assertEqual(events,[('notice',self.before+1),('ui',self.before+1)])
        self.assertEqual(destroyed,[True]);self.assertEqual(refs,[1]);self.assertEqual(updates,[7])
    def test_mode_reentrant_notice_preserves_nested_update(self):
        state=self.create();host=SubscriptionHost();address=c.addressof(state);events=[]
        @NOTICE
        def notice(_,english):
            events.append(english)
            if english:self.api.meltype_tsf_service_apply_mode(address,1,c.byref(host.context),None,notice)
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,7,c.byref(host.context),None,notice),1)
        self.assertEqual(events,[1,0]);self.assertFalse(state.code_app);self.assertFalse(state.code_english);host.verify(self)
    def test_write_open_uses_service_client_and_survives_state_destruction(self):
        state=self.create();address=c.addressof(state);host=CompartmentHost();state.manager=c.addressof(host.context);state.client=17
        self.assertEqual(self.api.meltype_tsf_service_write_open(address,0),1)
        self.assertEqual(host.writes_mode,[(bytes(OPEN),17,0)])
        def destroy():
            self.owned.remove(address);self.api.meltype_tsf_service_state_destroy(address)
        host.on_write=destroy
        self.assertEqual(self.api.meltype_tsf_service_write_open(address,99),1)
        self.assertEqual(host.writes_mode[-1],(bytes(OPEN),17,1));host.verify(self)
    def test_mode_null_inputs_do_not_mutate_state_or_notify(self):
        state=self.create();address=c.addressof(state);before=c.string_at(address,c.sizeof(State));events=[]
        notice=NOTICE(lambda *_:events.append(1))
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(address,7,None,None,notice),0)
        self.assertEqual(self.api.meltype_tsf_service_apply_mode(None,7,None,None,notice),0)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before);self.assertEqual(events,[])
        self.assertEqual(self.api.meltype_tsf_service_mode(None),0)
        self.assertEqual(self.api.meltype_tsf_service_write_open(None,1),0)
        self.assertEqual(self.api.meltype_tsf_service_write_open(address,1),0)
    def core(self,state,host,explicit=False,clock=None,notice=None,worker=None):
        clock=clock or CORE_CLOCK(lambda _:3000)
        notice=notice or CORE_NOTICE(lambda *_:None)
        return self.api.meltype_tsf_service_core(c.addressof(state),worker or self.worker,c.byref(host.context),123,notice,clock,int(explicit))
    def test_core_readiness_strict_retry_boundary_and_active_short_path(self):
        state=self.create();host=SubscriptionHost();now=[2000];times=[];events=[]
        clock=CORE_CLOCK(lambda owner:times.append(owner) or now[0]);notice=CORE_NOTICE(lambda owner,op,value:events.append((owner,op,value)))
        self.assertEqual(self.core(state,host,clock=clock,notice=notice),0);self.assertFalse(state.transport)
        now[0]=2001
        self.assertEqual(self.core(state,host,clock=clock,notice=notice),1)
        self.assertTrue(state.server_active);self.assertEqual(state.last_hello,2001)
        self.assertEqual(events,[(123,0,1)]);before=len(times)
        self.assertEqual(self.core(state,host,clock=clock,notice=notice),1);self.assertEqual(len(times),before)
        host.verify(self)
    def test_core_disabled_and_ended_service_do_not_read_clock_or_start_worker(self):
        state=self.create();host=SubscriptionHost();calls=[];clock=CORE_CLOCK(lambda _:calls.append(1) or 4000)
        for field in ['disabled','deactivated']:
            setattr(state,field,True)
            self.assertEqual(self.core(state,host,clock=clock),0);self.assertEqual(self.core(state,host,explicit=True,clock=clock),0)
            setattr(state,field,False)
        self.assertEqual(calls,[]);self.assertFalse(state.transport);host.verify(self)
    def fixture_core(self,state,host,reply,notice=None):
        if not self.fixture:self.skipTest('Pass the private framed transport fixture executable')
        root=Path(self.directory.name);response=root/'hello-reply.json';request=root/'hello-request.json'
        response.write_text(reply,encoding='utf-8')
        changes={'MELTYPE_TEST_HELLO_REPLY_FILE':str(response),'MELTYPE_TEST_HELLO_REQUEST_FILE':str(request)}
        previous={name:os.environ.get(name) for name in changes}
        try:
            os.environ.update(changes)
            self.api.meltype_tsf_service_close(c.addressof(state))
            result=self.core(state,host,explicit=True,notice=notice,worker=self.fixture)
        finally:
            for name,value in previous.items():
                if value is None:os.environ.pop(name,None)
                else:os.environ[name]=value
        return result,json.loads(request.read_text(encoding='utf-8'))
    def test_core_hello_native_process_thread_metadata_and_state_publication(self):
        state=self.create();host=SubscriptionHost();state.own_process=True;state.direct_word=True;events=[]
        notice=CORE_NOTICE(lambda owner,op,value:events.append((op,value,state.server_active,state.direct_detection,state.code_app,state.direct_word)))
        result,request=self.fixture_core(state,host,'{"active":true,"directDetection":true,"code":true}',notice)
        self.assertEqual(result,1);self.assertEqual(request['op'],'hello')
        kernel=c.WinDLL('kernel32');self.assertEqual(request['tid'],kernel.GetCurrentThreadId())
        self.assertEqual(request['process'].lower(),'python');self.assertEqual(state.last_hello,3000)
        self.assertEqual(events,[(0,1,True,True,True,False)]);host.verify(self)
    def test_core_inactive_and_non_boolean_fields_are_normalized(self):
        state=self.create();host=SubscriptionHost()
        for reply in ['{"active":false,"code":true,"directDetection":true}','{"active":1,"code":true}',
                      '{"active":true,"code":"true","directDetection":1}']:
            state.direct_word=True
            result,_=self.fixture_core(state,host,reply)
            expected=json.loads(reply);active=expected.get('active') is True
            self.assertEqual(result,int(active));self.assertEqual(state.server_active,active)
            self.assertEqual(state.code_app,active and expected.get('code') is True)
            self.assertEqual(state.direct_detection,active and expected.get('directDetection') is True);self.assertFalse(state.direct_word)
        host.verify(self)
    def test_core_invalid_json_reports_error_then_inactive(self):
        state=self.create();host=SubscriptionHost();events=[]
        notice=CORE_NOTICE(lambda owner,op,value:events.append((op,value)))
        result,_=self.fixture_core(state,host,'not-json',notice)
        self.assertEqual(result,0);self.assertEqual(events,[(1,0),(0,0)])
        self.assertFalse(state.server_active);host.verify(self)
    def test_core_reentrant_notice_supersedes_return_and_keeps_new_state(self):
        state=self.create();host=SubscriptionHost()
        def changed(owner,op,value):
            state.focus_epoch+=1;state.server_active=False;state.code_app=False
        notice=CORE_NOTICE(changed)
        result,_=self.fixture_core(state,host,'{"active":true,"code":true}',notice)
        # Notification runs after publication; return value remains the hello result,
        # while the nested generation/state change is preserved.
        self.assertEqual(result,1);self.assertFalse(state.server_active);self.assertFalse(state.code_app);host.verify(self)
    def test_core_null_arguments_do_not_mutate_state_or_start_transport(self):
        state=self.create();address=c.addressof(state);before=c.string_at(address,c.sizeof(State))
        for data,worker,lifetime in [(None,self.worker,None),(address,None,None),(address,self.worker,None)]:
            self.assertEqual(self.api.meltype_tsf_service_core(data,worker,lifetime,None,CORE_NOTICE(),CORE_CLOCK(),0),0)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before)
    def exchange(self,state,host,text=b'{"op":"hello"}',reply=None,notice=None,worker=None,timeout=5000,optional=False,lifetime=None):
        reply=reply or REPLY(lambda *_:1);notice=notice or TOGGLE(lambda *_:None)
        return self.api.meltype_tsf_service_exchange(c.addressof(state),worker or self.worker,lifetime or c.byref(host.context),text,len(text),timeout,int(optional),123,reply,notice)
    def test_exchange_native_transport_unicode_and_thread_metadata(self):
        state=self.create();host=SubscriptionHost();state.own_process=True
        if not self.fixture:self.skipTest('Pass framed transport fixture')
        root=Path(self.directory.name);response=root/'exchange-reply.json';request=root/'exchange-request.json'
        response.write_text('{"view":{"text":"試験😀"}}',encoding='utf-8')
        changes={'MELTYPE_TEST_HELLO_REPLY_FILE':str(response),'MELTYPE_TEST_HELLO_REQUEST_FILE':str(request)}
        previous={key:os.environ.get(key) for key in changes};events=[]
        @self.checked(REPLY)
        def parsed(owner,data,length):
            events.append((owner,json.loads(c.string_at(data,length))));return 1
        try:
            os.environ.update(changes)
            self.assertEqual(self.exchange(state,host,b'{"op":"probe","text":"Unicode"}',parsed,worker=self.fixture),1)
        finally:
            for key,value in previous.items():
                if value is None:os.environ.pop(key,None)
                else:os.environ[key]=value
        captured=json.loads(request.read_text(encoding='utf-8'))
        self.assertEqual(captured['tid'],c.WinDLL('kernel32').GetCurrentThreadId())
        self.assertEqual(captured['text'],'Unicode');self.assertEqual(events,[(123,{'view':{'text':'試験😀'}})])
        host.verify(self)
    def test_exchange_parse_failure_notifies_after_reply(self):
        state=self.create();host=SubscriptionHost();events=[]
        reply=REPLY(lambda owner,data,length:events.append(('reply',owner)) or 0)
        notice=TOGGLE(lambda owner:events.append(('notice',owner)))
        self.assertEqual(self.exchange(state,host,reply=reply,notice=notice),0)
        self.assertEqual(events,[('reply',123),('notice',123)]);host.verify(self)
    def test_exchange_reentrant_epoch_change_discards_result_and_notice(self):
        state=self.create();host=SubscriptionHost();events=[]
        @self.checked(REPLY)
        def parsed(owner,data,length):
            state.focus_epoch+=1;state.server_active=True;return 0
        self.assertEqual(self.exchange(state,host,reply=parsed,notice=TOGGLE(lambda _:events.append(1))),0)
        self.assertTrue(state.server_active);self.assertEqual(events,[]);host.verify(self)
    def test_exchange_reentrant_shutdown_discards_success(self):
        state=self.create();host=SubscriptionHost()
        @self.checked(REPLY)
        def parsed(owner,data,length):state.deactivated=True;return 1
        self.assertEqual(self.exchange(state,host,reply=parsed),0);host.verify(self)
    def test_exchange_nested_request_keeps_outer_reply_bytes_alive(self):
        state=self.create();host=SubscriptionHost();events=[]
        @self.checked(REPLY)
        def inner(owner,data,length):
            events.append(json.loads(c.string_at(data,length))['active']);return 1
        @self.checked(REPLY)
        def outer(owner,data,length):
            before=c.string_at(data,length)
            self.assertEqual(self.exchange(state,host,b'{"op":"reset","sid":"nested"}',reply=inner),1)
            self.assertEqual(c.string_at(data,length),before)
            events.append(json.loads(before)['active']);return 1
        self.assertEqual(self.exchange(state,host,reply=outer),1)
        self.assertEqual(events,[False,True]);host.verify(self)
    def test_exchange_ended_and_invalid_inputs_do_not_start_transport(self):
        state=self.create();host=SubscriptionHost();events=[];reply=REPLY(lambda *_:events.append(1) or 1)
        state.deactivated=True;self.assertEqual(self.exchange(state,host,reply=reply),0);state.deactivated=False
        address=c.addressof(state);unknown=c.byref(host.context)
        for data,worker,lifetime,raw,count,callback in [(None,self.worker,unknown,b'{}',2,reply),
            (address,None,unknown,b'{}',2,reply),(address,self.worker,None,b'{}',2,reply),
            (address,self.worker,unknown,None,1,reply),(address,self.worker,unknown,b'{}',2,REPLY())]:
            self.assertEqual(self.api.meltype_tsf_service_exchange(data,worker,lifetime,raw,count,5000,0,None,callback,TOGGLE()),0)
        self.assertEqual(self.exchange(state,host,text=b'{}\0garbage',reply=reply),0)
        self.assertFalse(state.transport);self.assertEqual(events,[]);host.verify(self)
    def test_exchange_zero_timeout_skips_reply_and_optional_reconnects(self):
        state=self.create();host=SubscriptionHost();events=[]
        reply=REPLY(lambda *_:events.append(1) or 1)
        self.assertEqual(self.exchange(state,host,reply=reply,timeout=0,optional=True),0)
        self.assertEqual(events,[])
        self.assertEqual(self.exchange(state,host,reply=reply,optional=True),1)
        self.assertEqual(events,[1]);host.verify(self)
    def test_exchange_native_lifetime_pins_service_during_reply(self):
        state=self.create();address=c.addressof(state);host=SubscriptionHost();events=[]
        dispatch=DISPATCH(lambda *_:0)
        @self.checked(DESTROY)
        def destroyed(owner):
            events.append('destroy');self.api.meltype_tsf_service_state_destroy(owner)
        root=self.api.meltype_tsf_service_com_create(address,dispatch,destroyed)
        self.assertTrue(root);self.owned.remove(address)
        @self.checked(REPLY)
        def parsed(owner,data,length):
            events.append('reply');release(root)
            self.assertEqual(self.live(),self.before+1);return 1
        try:
            self.assertEqual(self.exchange(state,host,reply=parsed,lifetime=root),1)
            self.assertEqual(events,['reply','destroy']);self.assertEqual(self.live(),self.before)
        finally:
            if 'reply' not in events:release(root)
    def focus(self,state,host,event=-1,document=None,commit=None,resume=None,notice=None,lifetime=None):
        return self.api.meltype_tsf_service_focus(c.addressof(state),self.worker,lifetime or c.byref(host.context),c.byref(host.sink),document,event,123,
            commit or FOCUS_COMMIT(lambda *_:1),resume or TOGGLE(lambda *_:None),notice or CORE_NOTICE(lambda *_:None))
    def test_focus_native_flags_reset_and_hello_publication(self):
        state=self.create();host=SubscriptionHost();events=[];state.direct_word=True
        resume=TOGGLE(lambda owner:events.append(('resume',owner,state.direct_word,state.caret_moved)))
        notice=CORE_NOTICE(lambda owner,op,value:events.append(('notice',owner,op,value,state.direct_word,state.caret_moved)))
        self.assertEqual(self.focus(state,host,resume=resume,notice=notice),0)
        self.assertEqual(events,[('resume',123,True,False),('notice',123,0,1,False,True)])
        self.assertTrue(state.server_active);self.assertEqual(state.focus_epoch,1);host.verify(self)
    def test_focus_native_edit_binding_and_disabled_hello_skip(self):
        state=self.create();host=SubscriptionHost();state.disabled=True;events=[]
        self.assertEqual(self.focus(state,host,document=c.byref(host.document_manager),notice=CORE_NOTICE(lambda *_:events.append(1))),0)
        self.assertTrue(state.edit_subscription);self.assertEqual(state.edit_context,c.addressof(host.context))
        self.assertEqual(events,[]);self.assertFalse(state.transport)
        self.edit_sink(state,host,False);host.verify(self)
    def test_focus_native_sync_failure_queues_async_before_resume(self):
        state=self.create();host=SubscriptionHost();state.disabled=True;state.composition=123;state.direct_word=True;events=[]
        @self.checked(FOCUS_COMMIT)
        def commit(owner,mode):events.append(('commit',owner,mode,state.direct_word));return 0
        @self.checked(TOGGLE)
        def resume(owner):events.append(('resume',owner,state.direct_word));state.composition=None
        self.assertEqual(self.focus(state,host,commit=commit,resume=resume),0)
        self.assertEqual(events,[('commit',123,0,True),('commit',123,1,True),('resume',123,True)])
        self.assertFalse(state.direct_word);self.assertTrue(state.caret_moved);host.verify(self)
    def test_focus_native_nested_focus_keeps_new_state(self):
        state=self.create();host=SubscriptionHost();state.disabled=True;events=[]
        @self.checked(TOGGLE)
        def resume(owner):
            events.append('outer')
            self.assertEqual(self.focus(state,host,resume=TOGGLE(lambda _:events.append('inner'))),0)
            state.direct_word=True;state.caret_moved=False
        self.assertEqual(self.focus(state,host,resume=resume),0)
        self.assertEqual(events,['outer','inner']);self.assertEqual(state.focus_epoch,2)
        self.assertTrue(state.direct_word);self.assertFalse(state.caret_moved);host.verify(self)
    def test_focus_native_stopping_document_is_inert(self):
        state=self.create();host=SubscriptionHost();events=[]
        for field,value in [('deactivated',True),('shutdown_phase',1)]:
            setattr(state,field,value)
            self.assertEqual(self.focus(state,host,resume=TOGGLE(lambda _:events.append(1))),0)
            setattr(state,field,0)
        self.assertEqual(events,[]);self.assertEqual(state.focus_epoch,0);self.assertFalse(state.transport);host.verify(self)
    def test_focus_native_foreground_preserves_document_state(self):
        state=self.create();host=SubscriptionHost();state.direct_word=True;events=[]
        for event in [1,0]:self.assertEqual(self.focus(state,host,event=event,resume=TOGGLE(lambda _:events.append(1))),0)
        self.assertEqual(events,[]);self.assertTrue(state.direct_word);self.assertFalse(state.caret_moved)
        self.assertEqual(state.focus_epoch,0);self.assertFalse(state.transport);host.verify(self)
    def test_focus_native_lifetime_pins_state_after_resume_releases_caller(self):
        state=self.create();address=c.addressof(state);host=SubscriptionHost();state.disabled=True;events=[]
        dispatch=DISPATCH(lambda *_:0)
        @self.checked(DESTROY)
        def destroyed(owner):events.append('destroy');self.api.meltype_tsf_service_state_destroy(owner)
        root=self.api.meltype_tsf_service_com_create(address,dispatch,destroyed);self.assertTrue(root);self.owned.remove(address)
        @self.checked(TOGGLE)
        def resume(owner):
            events.append('resume');release(root);self.assertEqual(self.live(),self.before+1)
        try:
            self.assertEqual(self.focus(state,host,lifetime=root,resume=resume),0)
            self.assertEqual(events,['resume','destroy']);self.assertEqual(self.live(),self.before)
        finally:
            if 'resume' not in events:release(root)
    def test_focus_native_invalid_arguments_preserve_state(self):
        state=self.create();host=SubscriptionHost();address=c.addressof(state);before=c.string_at(address,c.sizeof(State))
        for data,worker,lifetime,sink,document,event,commit,resume in [
            (None,self.worker,c.byref(host.context),None,None,-1,FOCUS_COMMIT(lambda *_:1),TOGGLE(lambda *_:None)),
            (address,None,c.byref(host.context),None,None,-1,FOCUS_COMMIT(lambda *_:1),TOGGLE(lambda *_:None)),
            (address,self.worker,None,None,None,-1,FOCUS_COMMIT(lambda *_:1),TOGGLE(lambda *_:None)),
            (address,self.worker,c.byref(host.context),None,c.byref(host.document_manager),-1,FOCUS_COMMIT(lambda *_:1),TOGGLE(lambda *_:None)),
            (address,self.worker,c.byref(host.context),None,None,2,FOCUS_COMMIT(lambda *_:1),TOGGLE(lambda *_:None)),
            (address,self.worker,c.byref(host.context),None,None,-1,FOCUS_COMMIT(),TOGGLE(lambda *_:None)),
            (address,self.worker,c.byref(host.context),None,None,-1,FOCUS_COMMIT(lambda *_:1),TOGGLE())]:
            self.assertEqual(self.api.meltype_tsf_service_focus(data,worker,lifetime,sink,document,event,None,commit,resume,CORE_NOTICE()),HR(0x80070057).value)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before);host.verify(self)
    def edited(self,state,host,query=None,commit=None,record=True,lifetime=None):
        return self.api.meltype_tsf_service_edited(c.addressof(state),lifetime or c.byref(host.context),c.byref(host.context),
            c.byref(host.record) if record else None,7,123,query or QUERY(lambda *_:None),commit or TOGGLE(lambda *_:None))
    def event_composition(self,state,host):
        pointer=self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7)
        self.assertTrue(pointer);state.composition=pointer;state.composition_context=c.addressof(host.context)
        self.addCleanup(host.verify,self);self.addCleanup(release,pointer);return pointer
    def test_edited_native_null_record_and_own_edit_consumption(self):
        state=self.create();host=EventHost();state.own_edit=True;events=[]
        query=QUERY(lambda *_:events.append('query'))
        self.assertEqual(self.edited(state,host,query=query,record=False),1);self.assertTrue(state.own_edit)
        self.assertEqual(self.edited(state,host,query=query),1);self.assertFalse(state.own_edit)
        self.assertEqual(events,[]);self.assertEqual(host.record_reads,0);host.verify(self)
    def test_edited_native_mode_query_precedes_moved_state(self):
        state=self.create();host=EventHost();events=[]
        @self.checked(QUERY)
        def query(owner,cookie,context,changed):
            self.assertEqual((owner,cookie,context,changed),(123,7,c.addressof(host.context),1))
            events.append(state.caret_moved)
        self.assertEqual(self.edited(state,host,query=query),1)
        self.assertEqual(events,[False]);self.assertTrue(state.caret_moved);host.verify(self)
    def test_edited_native_unchanged_or_failed_status_preserves_flags(self):
        state=self.create();host=EventHost();state.direct_word=True;events=[]
        query=QUERY(lambda owner,cookie,context,changed:events.append(changed))
        host.changed=False;self.assertEqual(self.edited(state,host,query=query),1)
        host.changed=True;host.fail_status=True;self.assertEqual(self.edited(state,host,query=query),1)
        self.assertEqual(events,[0]);self.assertTrue(state.direct_word);self.assertFalse(state.caret_moved);host.verify(self)
    def test_edited_native_outside_composition_marks_before_commit(self):
        state=self.create();host=EventHost();pointer=self.event_composition(state,host);events=[]
        state.direct_word=True;host.selection=(0,0)
        @self.checked(TOGGLE)
        def commit(owner):events.append((owner,state.caret_moved,state.direct_word))
        try:
            self.assertEqual(self.edited(state,host,commit=commit),1)
            self.assertEqual(events,[(123,True,False)]);self.assertEqual(state.composition,pointer)
        finally:state.composition=None;state.composition_context=None
    def test_edited_native_boundary_selection_keeps_composition(self):
        state=self.create();host=EventHost();self.event_composition(state,host);events=[]
        try:
            for selection in [(2,2),(3,3),(5,5),(2,5)]:
                host.selection=selection;self.assertEqual(self.edited(state,host,commit=TOGGLE(lambda _:events.append(1))),1)
            self.assertEqual(events,[]);self.assertFalse(state.caret_moved)
        finally:state.composition=None;state.composition_context=None
    def test_edited_native_reentrant_range_replacement_suppresses_commit(self):
        state=self.create();host=EventHost();self.event_composition(state,host);events=[];host.selection=(0,0)
        host.on_range=lambda:setattr(state,'composition',None)
        try:
            self.assertEqual(self.edited(state,host,commit=TOGGLE(lambda _:events.append(1))),1)
            self.assertEqual(events,[]);self.assertFalse(state.caret_moved)
        finally:state.composition=None;state.composition_context=None
    def test_edited_native_query_can_release_last_caller_reference(self):
        state=self.create();address=c.addressof(state);host=EventHost();events=[]
        dispatch=DISPATCH(lambda *_:0)
        @self.checked(DESTROY)
        def destroyed(owner):
            events.append(('destroy',state.caret_moved));self.api.meltype_tsf_service_state_destroy(owner)
        root=self.api.meltype_tsf_service_com_create(address,dispatch,destroyed);self.assertTrue(root);self.owned.remove(address)
        @self.checked(QUERY)
        def query(*args):
            events.append(('query',state.caret_moved));release(root);self.assertEqual(self.live(),self.before+1)
        try:
            self.assertEqual(self.edited(state,host,query=query,lifetime=root),1)
            self.assertEqual(events,[('query',False),('destroy',True)]);host.verify(self)
        finally:
            if not events:release(root)
    def test_edited_native_invalid_arguments_leave_state_unchanged(self):
        state=self.create();host=EventHost();address=c.addressof(state);state.own_edit=True;before=c.string_at(address,c.sizeof(State))
        for data,lifetime,query,commit in [(None,c.byref(host.context),QUERY(lambda *_:None),TOGGLE(lambda *_:None)),
            (address,None,QUERY(lambda *_:None),TOGGLE(lambda *_:None)),(address,c.byref(host.context),QUERY(),TOGGLE(lambda *_:None)),
            (address,c.byref(host.context),QUERY(lambda *_:None),TOGGLE())]:
            self.assertEqual(self.api.meltype_tsf_service_edited(data,lifetime,c.byref(host.context),c.byref(host.record),7,None,query,commit),0)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before);host.verify(self)
    def opened(self,state,host,commit=None,notice=None,guid=OPEN,lifetime=None):
        return self.api.meltype_tsf_service_opened(c.addressof(state),lifetime or c.byref(host.context),c.byref(guid),123,
            commit or TOGGLE(lambda *_:None),notice or NOTICE(lambda *_:None))
    def test_opened_native_publishes_strict_mode_and_clears_direct_word(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);events=[]
        notice=NOTICE(lambda owner,mode:events.append((owner,mode,state.open,state.direct_word)))
        try:
            for value in [-1,0,1]:
                state.direct_word=True;host.integers[bytes(OPEN)]=value
                self.assertEqual(self.opened(state,host,notice=notice),0)
                self.assertEqual(events[-1],(123,int(value!=0),value!=0,False))
            self.assertEqual(state.mode_epoch,3);host.verify(self)
        finally:state.manager=None
    def test_opened_native_close_orders_notice_commit_and_bar_update(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);state.composition=123;state.direct_word=True
        host.integers[bytes(OPEN)]=0;events=[];bar,_=self.create_bar(state)
        refs,updates=self.advise_bar(bar,on_update=lambda:events.append(('ui',state.open,state.direct_word)))
        notice=NOTICE(lambda owner,mode:events.append(('notice',state.open,state.direct_word)))
        commit=TOGGLE(lambda _:events.append(('commit',state.open,state.direct_word)))
        try:
            self.assertEqual(self.opened(state,host,commit,notice),0)
            self.assertEqual(events,[('notice',False,False),('commit',False,False),('ui',False,False)])
            self.assertEqual(updates,[7]);host.verify(self)
        finally:state.manager=None;state.composition=None
    def test_opened_native_failed_read_preserves_flags_but_commits_closed_input(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);state.composition=123
        state.open=False;state.direct_word=True;host.fail_read=True;events=[]
        try:
            self.assertEqual(self.opened(state,host,TOGGLE(lambda _:events.append('commit')),NOTICE(lambda owner,mode:events.append(('notice',mode)))),0)
            self.assertEqual(events,[('notice',0),'commit']);self.assertTrue(state.direct_word);host.verify(self)
        finally:state.manager=None;state.composition=None
    def test_opened_native_unrelated_guid_preserves_entire_state(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);before=c.string_at(c.addressof(state),c.sizeof(State))
        try:
            self.assertEqual(self.opened(state,host,guid=Guid.parse('00000000-0000-0000-0000-000000000000')),0)
            self.assertEqual(c.string_at(c.addressof(state),c.sizeof(State)),before);self.assertEqual(host.reads,[]);host.verify(self)
        finally:state.manager=None
    def test_opened_native_shutdown_and_ended_services_do_not_read_mode(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);events=[]
        try:
            for field,value in [('shutdown_phase',1),('deactivated',True)]:
                setattr(state,field,value)
                self.assertEqual(self.opened(state,host,notice=NOTICE(lambda *_:events.append(1))),0)
                setattr(state,field,0)
            self.assertEqual(events,[]);self.assertEqual(host.reads,[]);host.verify(self)
        finally:state.manager=None
    def test_opened_native_nested_notice_preserves_new_mode_and_skips_old_commit(self):
        state=self.create();host=OpenHost();state.manager=c.addressof(host.context);state.composition=123
        host.integers[bytes(OPEN)]=0;events=[];bar,_=self.create_bar(state)
        refs,updates=self.advise_bar(bar,on_update=lambda:events.append(('ui',state.open)))
        @self.checked(NOTICE)
        def notice(owner,mode):
            events.append(('notice',mode));host.integers[bytes(OPEN)]=1
            self.assertEqual(self.opened(state,host,notice=NOTICE(lambda owner,mode:events.append(('nested',mode)))),0)
        try:
            self.assertEqual(self.opened(state,host,TOGGLE(lambda _:events.append('old commit')),notice),0)
            self.assertEqual(events,[('notice',0),('nested',1),('ui',True)])
            self.assertTrue(state.open);self.assertEqual(state.mode_epoch,2);host.verify(self)
        finally:state.manager=None;state.composition=None
    def test_opened_native_manager_replacement_during_read_discards_reply(self):
        state=self.create();host=OpenHost();other=OpenHost();state.manager=c.addressof(host.context);events=[]
        host.integers[bytes(OPEN)]=0;host.on_read=lambda:setattr(state,'manager',c.addressof(other.context))
        state.direct_word=True
        try:
            self.assertEqual(self.opened(state,host,notice=NOTICE(lambda *_:events.append(1))),0)
            self.assertTrue(state.open);self.assertTrue(state.direct_word);self.assertEqual(events,[])
            host.verify(self);other.verify(self)
        finally:state.manager=None
    def test_opened_native_lifetime_pins_state_after_notice_releases_caller(self):
        state=self.create();address=c.addressof(state);host=OpenHost();events=[]
        state.manager=c.addressof(host.context);state.composition=123;host.integers[bytes(OPEN)]=0
        dispatch=DISPATCH(lambda *_:0)
        @self.checked(DESTROY)
        def destroyed(owner):
            events.append(('destroy',state.open));state.manager=None;state.composition=None
            self.api.meltype_tsf_service_state_destroy(owner)
        root=self.api.meltype_tsf_service_com_create(address,dispatch,destroyed);self.assertTrue(root);self.owned.remove(address)
        @self.checked(NOTICE)
        def notice(owner,mode):
            events.append(('notice',mode));release(root);self.assertEqual(self.live(),self.before+1)
        try:
            self.assertEqual(self.opened(state,host,TOGGLE(lambda _:events.append('commit')),notice,lifetime=root),0)
            self.assertEqual(events,[('notice',0),'commit',('destroy',False)]);host.verify(self)
        finally:
            if not events:release(root)
    def test_opened_native_invalid_arguments_preserve_state(self):
        state=self.create();host=OpenHost();address=c.addressof(state);before=c.string_at(address,c.sizeof(State))
        for data,lifetime,guid,commit in [(None,c.byref(host.context),c.byref(OPEN),TOGGLE(lambda *_:None)),
            (address,None,c.byref(OPEN),TOGGLE(lambda *_:None)),(address,c.byref(host.context),None,TOGGLE(lambda *_:None)),
            (address,c.byref(host.context),c.byref(OPEN),TOGGLE())]:
            self.assertEqual(self.api.meltype_tsf_service_opened(data,lifetime,guid,None,commit,NOTICE()),HR(0x80070057).value)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before);host.verify(self)
    def deactivate(self,state,host,request=None,end=None,notice=None,lifetime=None):
        return self.api.meltype_tsf_service_deactivate(c.addressof(state),lifetime or c.byref(host.context),123,
            request or SHUTDOWN_REQUEST(lambda *_:None),end or END(lambda *_:None),notice or NOTICE(lambda *_:None))
    def test_deactivate_native_close_metadata_precedes_stopped_state(self):
        state=self.create();host=SubscriptionHost();state.client=17;events=[]
        sid='終了"😀';self.assertEqual(self.api.meltype_tsf_service_set_sid(c.addressof(state),sid.encode()),1)
        @self.checked(SHUTDOWN_REQUEST)
        def request(owner,text,cookie,context,target):
            events.append((owner,json.loads(text),cookie,context,target,state.shutdown_phase,state.deactivated,state.client))
        self.assertEqual(self.deactivate(state,host,request=request),0)
        self.assertEqual(events,[(123,{'op':'close','sid':sid},0,None,None,1,False,17)])
        self.assertEqual((state.shutdown_phase,state.deactivated,state.client,state.focus_epoch),(2,True,0,1));host.verify(self)
    def test_deactivate_native_disabled_skips_close_and_repeated_call_is_inert(self):
        state=self.create();host=SubscriptionHost();state.disabled=True;events=[]
        request=SHUTDOWN_REQUEST(lambda *_:events.append(1))
        self.assertEqual(self.deactivate(state,host,request=request),0);self.assertEqual(self.deactivate(state,host,request=request),0)
        self.assertEqual(events,[]);self.assertEqual(state.focus_epoch,1);self.assertEqual(state.shutdown_phase,2);host.verify(self)
    def test_deactivate_native_owns_manager_edit_binding_ui_resume_and_transport_cleanup(self):
        state=self.create();host=SubscriptionHost();resume_host=GeometryHost();state.client=17
        state.manager=c.addressof(host.context);host.context_ref(1);self.edit_sink(state,host)
        state.resume=self.capture_resume(resume_host);self.create_bar(state);self.create_candidates(state)
        response=self.api.meltype_tsf_service_request(c.addressof(state),self.worker,b'{"op":"hello"}',5000,0)
        self.assertTrue(response);self.api.meltype_free(response)
        self.assertEqual(self.deactivate(state,host),0)
        for field in ['manager','edit_subscription','edit_context','resume','langbar','candidates','transport']:
            self.assertFalse(getattr(state,field),field)
        host.verify(self);resume_host.verify(self)
    def test_deactivate_native_nested_close_returns_without_duplicate_request(self):
        state=self.create();host=SubscriptionHost();events=[]
        @self.checked(SHUTDOWN_REQUEST)
        def request(*args):
            events.append('close');self.assertEqual(self.deactivate(state,host,request=SHUTDOWN_REQUEST(lambda *_:events.append('nested'))),0)
        self.assertEqual(self.deactivate(state,host,request=request),0)
        self.assertEqual(events,['close']);self.assertEqual(state.shutdown_phase,2);host.verify(self)
    def test_deactivate_native_reactivation_supersedes_old_cleanup(self):
        state=self.create();host=SubscriptionHost();bar,_=self.create_bar(state)
        @self.checked(SHUTDOWN_REQUEST)
        def request(*args):
            state.focus_epoch+=1;state.shutdown_phase=0
            self.assertEqual(self.api.meltype_tsf_service_initialize(c.addressof(state),23,0,0,b'new'),1)
        self.assertEqual(self.deactivate(state,host,request=request),0)
        self.assertEqual((state.shutdown_phase,state.deactivated,state.client,state.langbar),(0,False,23,bar));host.verify(self)
    def shutdown_composition(self,state,host):
        state.client=17
        state.composition=self.api.meltype_tsf_start_composition(c.byref(host.context),c.byref(host.sink),host.original,7)
        self.assertTrue(state.composition);state.composition_context=c.addressof(host.context);host.context_ref(1)
    def cleanup_shutdown_composition(self,state,host):
        host.cancel()
        if state.composition:release(state.composition);state.composition=None
        if state.composition_context:release(state.composition_context);state.composition_context=None
        host.verify(self)
    def test_deactivate_native_sync_composition_commit_end_then_close(self):
        state=self.create();host=DeferredHost();host.mode='sync';self.shutdown_composition(state,host);target=state.composition;events=[]
        @self.checked(SHUTDOWN_REQUEST)
        def request(owner,text,cookie,context,pointer):
            events.append((json.loads(text)['op'],cookie,context,pointer))
        @self.checked(END)
        def end(owner,cookie):
            self.assertEqual((owner,cookie),(123,7));events.append('end')
            self.api.meltype_tsf_end_composition(state.composition_context,state.composition,cookie)
        try:
            self.assertEqual(self.deactivate(state,host,request,end),0)
            self.assertEqual(events,[('commit',7,c.addressof(host.context),target),'end',('close',0,None,None)])
            self.assertFalse(state.composition);self.assertFalse(state.composition_context)
        finally:self.cleanup_shutdown_composition(state,host)
    def test_deactivate_native_late_edit_session_never_uses_stack_adapter(self):
        state=self.create();host=DeferredHost();self.shutdown_composition(state,host);events=[]
        try:
            self.assertEqual(self.deactivate(state,host,SHUTDOWN_REQUEST(lambda owner,text,*_:events.append(json.loads(text)['op'])),END(lambda *_:events.append('end'))),0)
            self.assertEqual(events,['close']);self.assertTrue(host.pending)
            pending,host.pending=host.pending,[]
            for session in pending:
                try:self.assertEqual(invoke(session,3,HR,[U32],7),HR(0x80004004).value)
                finally:release(session)
            self.assertEqual(events,['close'])
        finally:self.cleanup_shutdown_composition(state,host)
    def test_deactivate_native_refused_edit_releases_composition_slots(self):
        state=self.create();host=DeferredHost();host.mode='reject';self.shutdown_composition(state,host);events=[]
        try:
            self.assertEqual(self.deactivate(state,host,SHUTDOWN_REQUEST(lambda owner,text,*_:events.append(json.loads(text)['op'])),END(lambda *_:events.append('end'))),0)
            self.assertEqual(events,['close']);self.assertFalse(state.composition);self.assertFalse(state.composition_context)
        finally:self.cleanup_shutdown_composition(state,host)
    def test_deactivate_native_abandoned_failure_reports_notice_and_continues(self):
        state=self.create();host=SubscriptionHost();old=DeferredHost();events=[]
        state.client=17;state.abandoned=self.api.meltype_tsf_abandoned_create()
        pointer=self.api.meltype_tsf_start_composition(c.byref(old.context),c.byref(old.sink),old.original,7)
        self.assertTrue(pointer)
        try:self.assertGreaterEqual(self.api.meltype_tsf_abandoned_add(state.abandoned,c.byref(old.context),pointer,c.byref(host.context),17),0)
        finally:release(pointer)
        old.mode='reject'
        self.assertEqual(self.deactivate(state,host,notice=NOTICE(lambda owner,status:events.append((owner,status)))),0)
        self.assertEqual(len(events),1);self.assertEqual(events[0][0],123);self.assertLess(events[0][1],0)
        self.assertEqual(state.shutdown_phase,2);old.cancel();old.verify(self);host.verify(self)
    def test_deactivate_native_last_caller_release_during_close_pins_cleanup(self):
        state=self.create();address=c.addressof(state);host=SubscriptionHost();events=[];self.create_bar(state)
        dispatch=DISPATCH(lambda *_:0)
        @self.checked(DESTROY)
        def destroyed(owner):
            events.append(('destroy',state.shutdown_phase,state.deactivated,state.langbar));self.api.meltype_tsf_service_state_destroy(owner)
        root=self.api.meltype_tsf_service_com_create(address,dispatch,destroyed);self.assertTrue(root);self.owned.remove(address)
        @self.checked(SHUTDOWN_REQUEST)
        def request(*args):events.append('close');release(root);self.assertEqual(self.live(),self.before+1)
        try:
            self.assertEqual(self.deactivate(state,host,request=request,lifetime=root),0)
            self.assertEqual(events,['close',('destroy',2,True,None)]);host.verify(self)
        finally:
            if not events:release(root)
    def test_deactivate_native_invalid_arguments_preserve_state(self):
        state=self.create();host=SubscriptionHost();address=c.addressof(state);before=c.string_at(address,c.sizeof(State))
        for data,lifetime,request,end in [(None,c.byref(host.context),SHUTDOWN_REQUEST(lambda *_:None),END(lambda *_:None)),
            (address,None,SHUTDOWN_REQUEST(lambda *_:None),END(lambda *_:None)),(address,c.byref(host.context),SHUTDOWN_REQUEST(),END(lambda *_:None)),
            (address,c.byref(host.context),SHUTDOWN_REQUEST(lambda *_:None),END())]:
            self.assertEqual(self.api.meltype_tsf_service_deactivate(data,lifetime,None,request,end,NOTICE()),HR(0x80070057).value)
        self.assertEqual(c.string_at(address,c.sizeof(State)),before);host.verify(self)
if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)

