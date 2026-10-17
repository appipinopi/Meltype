# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Core readiness/hello/request control with private COM owner and transport."""
import ctypes as c
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_fields_smoke import CompartmentHost
from tsf_com_smoke import P,U32
from tsf_range_smoke import units
READ=c.CFUNCTYPE(U32,P,c.POINTER(c.c_uint64))
UPDATE=c.CFUNCTYPE(None,P,U32,c.c_uint64)
HELLO=c.CFUNCTYPE(None,P)
REQUEST=c.CFUNCTYPE(c.c_int,P,c.c_char_p,c.POINTER(U32))
CLOCK=c.CFUNCTYPE(c.c_uint64,P)
FLAGS=c.CFUNCTYPE(U32,P)
TRANSACT=c.CFUNCTYPE(c.c_int,P,P,U32,U32,c.c_int,c.POINTER(P),c.POINTER(U32))
PARSE=c.CFUNCTYPE(c.c_int,P,P,U32)
NOTICE=c.CFUNCTYPE(None,P)
class State(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('read',READ),('update',UPDATE),('hello',HELLO),('request',REQUEST),('clock',CLOCK)]
class Exchange(c.Structure):
    _fields_=[('owner',P),('lifetime',P),('epoch',c.POINTER(c.c_uint64)),('read',FLAGS),('transact',TRANSACT),('parse',PARSE),('notice',NOTICE)]
class CoreLinkTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_core_ready.argtypes=[c.POINTER(State)]
        cls.api.meltype_tsf_core_hello.argtypes=[P,U32,c.POINTER(State)]
        cls.api.meltype_tsf_core_request.argtypes=[P,U32,U32,c.c_int,c.POINTER(Exchange)]
    def setUp(self):
        self.host=CompartmentHost();self.epoch=c.c_uint64();self.flags=0
        self.time=5000;self.last=1000;self.clock_reads=0
        self.events=[];self.errors=[];self.reply_flags=7;self.hello_ok=True
        self.direct=self.code=False;self.word=True
        self.on_hello=self.on_update=self.on_transact=None
        self.transport_ok=self.parse_ok=True;self.exchanges=[];self.parsed=[];self.notices=0
        self.response=c.create_string_buffer(b'{"active":true}')
        self.process=units('編集"😀')
        def guard(fn,default=None):
            def run(*args):
                try:
                    self.assertEqual(args[0],123);self.assertGreaterEqual(self.host.context_refs,1)
                    return fn(*args)
                except BaseException as e:self.errors.append(repr(e));return default
            return run
        self.callbacks=[READ(guard(self.read,4)),UPDATE(guard(self.update)),HELLO(guard(lambda _:self.hello())),REQUEST(guard(self.hello_request,0)),CLOCK(guard(self.clock,0))]
        self.state=State(123,c.addressof(self.host.context),c.pointer(self.epoch),*self.callbacks)
        self.exchange_callbacks=[FLAGS(guard(lambda _:self.flags,4)),TRANSACT(guard(self.transact,0)),PARSE(guard(self.parse,0)),NOTICE(guard(self.notice))]
        self.exchange=Exchange(123,c.addressof(self.host.context),c.pointer(self.epoch),*self.exchange_callbacks)
    def tearDown(self):self.host.verify(self);self.assertEqual(self.errors,[])
    def read(self,owner,last):last[0]=self.last;return self.flags
    def clock(self,owner):self.clock_reads+=1;return self.time
    def update(self,owner,op,value):
        self.events.append((op,value))
        if op==0:self.last=value
        if op==1:self.flags=(self.flags&~2)|(2 if value&1 else 0);self.direct=bool(value&2);self.code=bool(value&4);self.word=False
        if self.on_update:self.on_update(op)
    def hello_request(self,owner,text,flags):
        self.events.append(('request',json.loads(text)))
        flags[0]=self.reply_flags
        if self.on_hello:self.on_hello()
        return int(self.hello_ok)
    def hello(self):
        data=(c.c_uint16*len(self.process))(*self.process)
        return self.api.meltype_tsf_core_hello(data,len(data),c.byref(self.state))
    def ready(self):return self.api.meltype_tsf_core_ready(c.byref(self.state))
    def transact(self,owner,data,length,timeout,optional,out,count):
        self.exchanges.append((c.string_at(data,length),timeout,optional))
        if not self.transport_ok:return 0
        out[0]=c.addressof(self.response);count[0]=len(self.response.raw)-1
        if self.on_transact:self.on_transact()
        return 1
    def parse(self,owner,data,length):self.parsed.append(c.string_at(data,length) if length else b'');return int(self.parse_ok)
    def notice(self,owner):self.notices+=1
    def request(self,text=b'{"op":"key","sid":"a"}',timeout=5000,optional=0):
        data=c.create_string_buffer(text)
        return self.api.meltype_tsf_core_request(data,len(text),timeout,optional,c.byref(self.exchange))
    def test_core_ready_active_skips_clock_and_hello(self):
        self.flags=2;self.assertEqual(self.ready(),1);self.assertEqual(self.clock_reads,0);self.assertEqual(self.events,[])
    def test_core_ready_disabled_even_if_active_is_inert(self):
        self.flags=3;self.assertEqual(self.ready(),0);self.assertEqual(self.clock_reads,0)
    def test_core_ready_ended_is_inert(self):
        self.flags=4;self.assertEqual(self.ready(),0);self.assertEqual(self.events,[])
    def test_core_ready_exact_retry_boundary_and_refresh(self):
        self.time=3000;self.assertEqual(self.ready(),0);self.assertEqual(self.events,[])
        self.time=3001;self.assertEqual(self.ready(),1);self.assertEqual(self.last,3001)
        self.assertTrue(self.direct and self.code);self.assertFalse(self.word)
    def test_core_ready_failed_refresh_waits_another_interval(self):
        self.hello_ok=False;self.assertEqual(self.ready(),0);self.assertEqual(self.last,5000)
        before=len(self.events);self.time=7000;self.ready();self.assertEqual(len(self.events),before)
    def test_core_hello_unicode_request_and_state_publication_order(self):
        self.assertEqual(self.hello(),1)
        self.assertEqual(self.events,[(0,5000),('request',{'op':'hello','process':'編集"😀'}),(1,7),(2,1)])
        self.assertEqual(self.flags,2);self.assertTrue(self.direct and self.code);self.assertFalse(self.word)
    def test_core_hello_inactive_reply_clears_detection_and_code(self):
        self.reply_flags=6;self.direct=self.code=True;self.assertEqual(self.hello(),0)
        self.assertFalse(self.direct or self.code or self.word);self.assertEqual(self.events[-2:],[(1,0),(2,0)])
    def test_core_hello_failed_request_clears_state(self):
        self.flags=2;self.hello_ok=False;self.assertEqual(self.hello(),0)
        self.assertEqual(self.flags,0);self.assertEqual(self.events[-2:],[(1,0),(2,0)])
    def test_core_hello_disabled_has_no_timestamp_or_request(self):
        self.flags=1;self.assertEqual(self.hello(),0);self.assertEqual(self.last,1000);self.assertEqual(self.events,[])
    def test_core_hello_ended_during_request_skips_old_publication(self):
        self.on_hello=lambda:setattr(self,'flags',4);self.hello()
        self.assertEqual(len(self.events),2);self.assertEqual(self.flags,4);self.assertTrue(self.word)
    def test_core_hello_new_epoch_during_request_preserves_new_state(self):
        def replace():self.epoch.value+=1;self.flags=2
        self.on_hello=replace;self.hello();self.assertEqual(len(self.events),2);self.assertTrue(self.word)
    def test_core_hello_disabled_during_timestamp_update_skips_request(self):
        self.on_update=lambda op:setattr(self,'flags',1) if op==0 else None
        self.hello();self.assertEqual(self.events,[(0,5000)])
    def test_core_hello_owner_survives_original_release(self):
        self.on_hello=lambda:self.host.context_ref(-1);self.hello()
        self.assertEqual(self.host.context_refs,0);self.host.context_ref(1)
    def test_core_request_preserves_bytes_and_forwards_deadline_policy(self):
        body='{"op":"key","sid":"通知😀"}'.encode()
        self.assertEqual(self.request(body,100,1),1);self.assertEqual(self.exchanges,[(body,100,1)])
        self.assertEqual(self.parsed,[b'{"active":true}'])
    def test_core_request_own_process_attaches_current_thread(self):
        self.flags=8;self.request();body=json.loads(self.exchanges[0][0])
        self.assertEqual(body['tid'],c.windll.kernel32.GetCurrentThreadId());self.assertEqual(body['op'],'key')
    def test_core_request_trailing_whitespace_preserves_original_insertion_rule(self):
        self.flags=8;self.request(b'{"op":"key"} ');self.assertEqual(self.exchanges[0][0],b'{"op":"key"} ')
    def test_core_request_empty_object_preserves_original_byte_insertion(self):
        self.flags=8;self.request(b'{}');self.assertTrue(self.exchanges[0][0].startswith(b'{,"tid":'))
    def test_core_request_ended_skips_transport(self):
        self.flags=4;self.assertEqual(self.request(),0);self.assertEqual(self.exchanges,[])
    def test_core_request_transport_failure_skips_parser_and_notice(self):
        self.transport_ok=False;self.assertEqual(self.request(),0);self.assertEqual(self.parsed,[]);self.assertEqual(self.notices,0)
    def test_core_request_parse_failure_notifies_once(self):
        self.parse_ok=False;self.assertEqual(self.request(),0);self.assertEqual(self.notices,1)
    def test_core_request_ended_during_transport_skips_parser(self):
        self.on_transact=lambda:setattr(self,'flags',4);self.assertEqual(self.request(),0)
        self.assertEqual(self.parsed,[]);self.assertEqual(self.notices,0)
    def test_core_request_new_epoch_during_transport_skips_parser(self):
        self.on_transact=lambda:setattr(self.epoch,'value',1);self.assertEqual(self.request(),0);self.assertEqual(self.parsed,[])
    def test_core_request_owner_survives_original_release(self):
        self.on_transact=lambda:self.host.context_ref(-1);self.assertEqual(self.request(),1)
        self.assertEqual(self.host.context_refs,0);self.host.context_ref(1)

if __name__=='__main__':unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
