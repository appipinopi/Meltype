# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Real child processes, deadlines, bounded pipes and Windows job-tree teardown."""
import ctypes as c
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import time
import unittest
P=c.c_void_p
kernel=c.WinDLL("kernel32")
kernel.OpenProcess.argtypes=[c.c_uint32,c.c_int,c.c_uint32];kernel.OpenProcess.restype=P
kernel.WaitForSingleObject.argtypes=[P,c.c_uint32]
kernel.CloseHandle.argtypes=[P]

class TransportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.worker=str(Path(sys.argv[2]).resolve())
        cls.fixture=str(Path(sys.argv[3]).resolve())
        cls.api.meltype_tip_transport_create.argtypes=[c.c_char_p];cls.api.meltype_tip_transport_create.restype=P
        cls.api.meltype_tip_transport_request.argtypes=[P,c.c_char_p,c.c_uint32,c.c_int];cls.api.meltype_tip_transport_request.restype=P
        cls.api.meltype_tip_transport_destroy.argtypes=[P]
        cls.api.meltype_free.argtypes=[P]

    def setUp(self):
        self.directory=tempfile.TemporaryDirectory(prefix="meltype-transport-")
        self.root=Path(self.directory.name)
        self.transports=[]
        self.previous=os.environ.get("MELTYPE_RUST_DATA_DIR")
        os.environ["MELTYPE_RUST_DATA_DIR"]=str(self.root/'profile')
        (self.root/'profile').mkdir()
        (self.root/'profile'/'config.json').write_text(json.dumps({"DetectionLevel":"Manual"}),encoding='utf-8')

    def tearDown(self):
        for handle in self.transports:self.api.meltype_tip_transport_destroy(handle)
        if self.previous is None:os.environ.pop("MELTYPE_RUST_DATA_DIR",None)
        else:os.environ["MELTYPE_RUST_DATA_DIR"]=self.previous
        # Destruction initiates job termination without blocking the input thread.
        # Wait for this fixture's executable/file handles before removing its files.
        deadline=time.monotonic()+3
        while True:
            try:self.directory.cleanup();break
            except PermissionError:
                if time.monotonic()>=deadline:raise
                time.sleep(.02)

    def create(self,worker=None):
        handle=self.api.meltype_tip_transport_create((worker or self.fixture).encode())
        self.assertTrue(handle);self.transports.append(handle);return handle

    def call(self,handle,text="pid",timeout=5000,optional=True):
        raw=self.api.meltype_tip_transport_request(handle,text.encode(),timeout,int(optional))
        if not raw:return None
        try:return json.loads(c.string_at(raw))
        finally:self.api.meltype_free(raw)

    def process(self,pid):
        return kernel.OpenProcess(0x100000,False,pid)

    def stopped(self,handle):
        if handle:
            try:self.assertEqual(kernel.WaitForSingleObject(handle,3000),0)
            finally:kernel.CloseHandle(handle)

    def test_optional_deadline_kills_worker_and_reconnects_without_old_response(self):
        handle=self.create();first=self.call(handle);process=self.process(first['pid'])
        start=time.monotonic()
        self.assertIsNone(self.call(handle,f"delay|800|{self.root/'pid'}",100))
        self.assertLess(time.monotonic()-start,0.8)
        self.stopped(process)
        second=self.call(handle)
        self.assertNotEqual(second['pid'],first['pid']);self.assertEqual(second['serial'],1)

    def test_mandatory_timeout_backs_off_then_reconnects(self):
        handle=self.create();first=self.call(handle)
        self.assertIsNone(self.call(handle,f"delay|800|{self.root/'pid'}",100,False))
        start=time.monotonic();self.assertIsNone(self.call(handle));self.assertLess(time.monotonic()-start,0.25)
        time.sleep(3.05)
        second=self.call(handle);self.assertNotEqual(second['pid'],first['pid'])

    def test_blocked_stdin_write_is_bounded(self):
        handle=self.create();first=self.call(handle,"stopreading");process=self.process(first['pid'])
        start=time.monotonic();self.assertIsNone(self.call(handle,"x"*65536,100));self.assertLess(time.monotonic()-start,0.8)
        self.stopped(process);self.assertEqual(self.call(handle)['serial'],1)

    def test_close_terminates_idle_worker_and_descendants(self):
        handle=self.create();first=self.call(handle,"child")
        worker,child=self.process(first['pid']),self.process(first['child'])
        self.assertTrue(child)
        self.api.meltype_tip_transport_destroy(handle)
        self.stopped(worker);self.stopped(child)
        self.assertIsNone(self.call(handle))

    def test_independent_connections_and_concurrent_busy_call(self):
        first,second=self.create(),self.create();self.call(first);self.call(second)
        result=[];marker=self.root/'running'
        thread=threading.Thread(target=lambda:result.append(self.call(first,f"delay|1000|{marker}",5000)))
        thread.start()
        try:
            deadline=time.monotonic()+3
            while not marker.exists() and time.monotonic()<deadline:time.sleep(.01)
            self.assertTrue(marker.exists())
            start=time.monotonic();self.assertIsNone(self.call(first));self.assertLess(time.monotonic()-start,.25)
            self.assertIsNotNone(self.call(second,timeout=500))
            self.api.meltype_tip_transport_destroy(first)
        finally:thread.join(3)
        self.assertFalse(thread.is_alive());self.assertEqual(result,[None])

    def test_invalid_response_or_exit_resets_connection(self):
        for command in ["oversize","invalid","exit"]:
            handle=self.create();first=self.call(handle)
            self.assertIsNone(self.call(handle,command))
            second=self.call(handle);self.assertNotEqual(first['pid'],second['pid'])
            self.assertEqual(second['serial'],1)

    def test_invalid_path_and_oversized_input_do_not_start_worker(self):
        self.assertFalse(self.api.meltype_tip_transport_create(b"relative.exe"))
        self.assertFalse(self.api.meltype_tip_transport_create(str(self.root/'missing.exe').encode()))
        handle=self.create();self.assertIsNone(self.call(handle,"x"*65537));self.assertEqual(self.call(handle)['serial'],1)

    def test_zero_deadline_does_not_execute_late_request(self):
        handle=self.create();marker=self.root/'late'
        self.assertIsNone(self.call(handle,f"delay|1|{marker}",0))
        self.assertIsNotNone(self.call(handle));self.assertFalse(marker.exists())

    def test_actual_worker_protocol_and_session_isolation(self):
        folder=self.root/'ワーカー 空白';folder.mkdir();worker=folder/'meltype-tip-worker.exe';shutil.copy2(self.worker,worker)
        first,second=self.create(str(worker)),self.create(str(worker))
        hello=self.call(first,json.dumps({"op":"hello"}))
        self.assertTrue(hello['active'])
        request={"op":"key","sid":"same","vk":75,"ch":107,"code":True}
        self.assertTrue(self.call(first,json.dumps(request))['consumed'])
        request.update(vk=65,ch=97)
        response=self.call(first,json.dumps(request))
        self.assertEqual(response['view']['text'],'か')
        request.update(composing=True)
        self.assertFalse(self.call(second,json.dumps(request))['active'])
        self.assertTrue(self.call(first,json.dumps({"op":"reset","sid":"same"}))['active'])

if __name__=="__main__":unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
