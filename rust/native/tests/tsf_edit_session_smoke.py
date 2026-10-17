# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Exercise Rust ITfEditSession through the SDK's real ITfContext ABI.

A controlled in-process context schedules/refuses requests, so no user's text
or input source is modified. COM interface calls are actual x64/x86 vtable calls.
"""
import ctypes as c
from pathlib import Path
import sys
import threading
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_com_smoke import Guid, P, U32, HR, UNKNOWN, NO_INTERFACE, INVALID, invoke, release, assert_rust_vtable

EDIT = Guid.parse("aa80e803-2021-11d2-93e0-0060b067b86e")
SYNCHRONOUS = HR(0x80040208).value
ASYNC = 0x40300
ACTION = c.CFUNCTYPE(HR, P, U32)
DESTROY = c.CFUNCTYPE(None, P)
REQUEST = c.WINFUNCTYPE(HR, P, U32, P, U32, c.POINTER(HR))

class EditTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_request_edit_session.argtypes=[P,U32,U32,P,ACTION,DESTROY]
        cls.api.meltype_tsf_request_edit_session.restype=HR
    def setUp(self):
        self.calls=[];self.drops=[];self.pending=None;self.requests=[];self.mode="sync"
        self.action_result=0
        self.action=ACTION(self.run_action)
        self.destroy=DESTROY(lambda context:self.drops.append(context))
        self.errors=[]
        self.dispatch=REQUEST(self.dispatch_request)
        # ITfContext::RequestEditSession is the first method after IUnknown.
        # The borrowed context is never queried/refcounted by RequestEditSession.
        self.vtable=(P*4)(None,None,None,c.cast(self.dispatch,P))
        self.context=(P*1)(c.cast(self.vtable,P))
        self.addCleanup(self.cancel)
    def tearDown(self):
        self.assertEqual(self.errors,[])
    def dispatch_request(self, *args):
        try: return self.request(*args)
        except BaseException as error:
            self.errors.append(str(error))
            return HR(0x80004005).value
    def run_action(self, context, cookie):
        self.calls.append((context,cookie))
        return self.action_result
    def request(self, _this, client, session, flags, result):
        self.requests.append((client,flags))
        assert_rust_vtable(session)
        identity=P()
        assert invoke(session,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(EDIT),c.byref(identity))==0
        assert identity.value==session
        release(identity)
        rejected=P(123)
        assert invoke(session,0,HR,[c.POINTER(Guid),c.POINTER(P)],c.byref(Guid.parse("00000001-0000-0000-c000-000000000046")),c.byref(rejected))==NO_INTERFACE
        assert rejected.value is None
        if self.mode=="reject":
            result[0]=0
            return SYNCHRONOUS
        if self.mode=="async":
            invoke(session,1,U32,[])
            self.pending=session
            result[0]=ASYNC
            return 0
        result[0]=invoke(session,3,HR,[U32],0x12345678)
        return 0
    def cancel(self):
        if self.pending:
            value=self.pending;self.pending=None
            release(value)
    def submit(self, context=True, action=True):
        return self.api.meltype_tsf_request_edit_session(c.cast(self.context,P) if context else None,17,6,123,self.action if action else ACTION(),self.destroy)
    def test_sync_cookie_flags_identity_and_action_failure(self):
        self.action_result=HR(0x80004005).value
        self.assertEqual(self.submit(),self.action_result)
        self.assertEqual(self.requests,[(17,6)])
        self.assertEqual(self.calls,[(123,0x12345678)])
        self.assertEqual(self.drops,[123])
    def test_rejected_request_destroys_action_without_execution(self):
        self.mode="reject"
        self.assertEqual(self.submit(),SYNCHRONOUS)
        self.assertEqual(self.calls,[])
        self.assertEqual(self.drops,[123])
    def test_async_retains_action_after_return_until_host_finishes(self):
        self.mode="async"
        self.assertEqual(self.submit(),ASYNC)
        self.assertEqual(self.calls,[])
        self.assertEqual(self.drops,[])
        self.assertEqual(invoke(self.pending,3,HR,[U32],77),0)
        self.assertEqual(self.calls,[(123,77)])
        self.assertEqual(self.drops,[])
        self.cancel()
        self.assertEqual(self.drops,[123])
    def test_async_cancel_drops_without_touching_document(self):
        self.mode="async"
        self.assertEqual(self.submit(),ASYNC)
        self.cancel()
        self.assertEqual(self.calls,[])
        self.assertEqual(self.drops,[123])
    def test_wrong_thread_cannot_execute_captured_sta_action(self):
        self.mode="async";self.submit()
        result=[]
        worker=threading.Thread(target=lambda:result.append(invoke(self.pending,3,HR,[U32],99)))
        worker.start();worker.join(timeout=5)
        self.assertFalse(worker.is_alive())
        self.assertEqual(result,[HR(0x8001010e).value])
        self.assertEqual(self.calls,[])
        self.cancel();self.assertEqual(self.drops,[123])
    def test_invalid_context_and_callback_release_transferred_ownership(self):
        self.assertEqual(self.submit(context=False),INVALID)
        self.assertEqual(self.submit(action=False),INVALID)
        self.assertEqual(self.drops,[123,123])
        self.assertEqual(self.requests,[])

if __name__=="__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
