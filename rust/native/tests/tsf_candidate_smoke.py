# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Real private HWNDs: render, hit-test, timer, focus and reentrant teardown.

An optional reference DLL compares client pixels with the unchanged C++ renderer.
No text input service is registered/activated; callbacks affect only test objects.
"""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import struct
import threading
import time
import unittest

P, U, I, S = c.c_void_p, c.c_uint32, c.c_int32, c.c_size_t
class Point(c.Structure):
    _fields_ = [("x", I), ("y", I)]
class Rect(c.Structure):
    _fields_ = [("left", I), ("top", I), ("right", I), ("bottom", I)]
class Message(c.Structure):
    _fields_ = [("window", P), ("message", U), ("w", S), ("l", c.c_ssize_t), ("time", U), ("point", Point), ("private", U)]
user = c.WinDLL("user32")
kernel = c.WinDLL("kernel32")
user.GetForegroundWindow.restype = P
user.GetClassNameW.argtypes = [P, c.c_wchar_p, I]
user.GetWindowRect.argtypes = [P, c.POINTER(Rect)]
user.GetClientRect.argtypes = [P, c.POINTER(Rect)]
user.IsWindowVisible.argtypes = [P]
user.GetDpiForWindow.argtypes = [P]
user.SendMessageW.argtypes = [P, U, S, c.c_ssize_t]
user.SendMessageW.restype = c.c_ssize_t
user.PeekMessageW.argtypes = [c.POINTER(Message), P, U, U, U]
user.TranslateMessage.argtypes = [c.POINTER(Message)]
user.DispatchMessageW.argtypes = [c.POINTER(Message)]
user.DispatchMessageW.restype = c.c_ssize_t
enum_type = c.WINFUNCTYPE(I, P, c.c_ssize_t)
user.EnumThreadWindows.argtypes = [U, enum_type, c.c_ssize_t]

def pump(seconds=0.05):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        message = Message()
        while user.PeekMessageW(c.byref(message), None, 0, 0, 1):
            user.TranslateMessage(c.byref(message))
            user.DispatchMessageW(c.byref(message))
        time.sleep(0.002)

def wait(predicate, description, timeout=8):
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        if predicate(): return
        pump()
    raise AssertionError("timed out: " + description)

def windows(class_name):
    result = []
    @enum_type
    def collect(window, _):
        name = c.create_unicode_buffer(128)
        user.GetClassNameW(window, name, len(name))
        if name.value == class_name: result.append(window)
        return 1
    user.EnumThreadWindows(kernel.GetCurrentThreadId(), collect, 0)
    return result

def bounds(window, client=False):
    rect = Rect()
    assert (user.GetClientRect if client else user.GetWindowRect)(window, c.byref(rect))
    return rect.left, rect.top, rect.right, rect.bottom

select_type = c.CFUNCTYPE(None, P, I, I)
class Backend:
    def __init__(self, path, reference=False):
        self.api = c.CDLL(str(Path(path).resolve()))
        self.name = "MeltypeCandidateWindow" if reference else "MeltypeRustCandidateWindow"
        self.meaning_name = "MeltypeMeaningWindow" if reference else "MeltypeRustMeaningWindow"
        self.api.meltype_tsf_candidate_create.argtypes = [P, select_type]
        self.api.meltype_tsf_candidate_create.restype = P
        self.api.meltype_tsf_candidate_show.argtypes = [P, c.c_char_p, c.POINTER(Rect)]
        self.api.meltype_tsf_candidate_show.restype = I
        for name in ["hide", "visible", "destroy"]:
            getattr(self.api, "meltype_tsf_candidate_" + name).argtypes = [P]
        self.events = []
        self.destroy_on_click = False
        self.callback = select_type(self.selected)
        self.handle = self.api.meltype_tsf_candidate_create(123, self.callback)
        assert self.handle
    def selected(self, context, index, prediction):
        assert context == 123
        self.events.append((index, prediction))
        if self.destroy_on_click: self.close()
        else: self.api.meltype_tsf_candidate_hide(self.handle)
    def show(self, view, anchor=None):
        anchor = anchor or Rect(200, 180, 210, 200)
        assert self.api.meltype_tsf_candidate_show(self.handle, json.dumps(view, ensure_ascii=False).encode(), c.byref(anchor)) == 1
        pump()
        return next(w for w in windows(self.name) if user.IsWindowVisible(w))
    def close(self):
        if self.handle:
            self.api.meltype_tsf_candidate_destroy(self.handle)
            self.handle = None
        pump(0.01)

def view(selected=0, count=18, **extra):
    return {"candidates":[f"候補{i}" for i in range(count)], "notes":["note" for _ in range(count)], "selected":selected, **extra}

def capture(window):
    """Read only this test HWND's client surface after Direct2D/DWM finish."""
    class Header(c.Structure):
        _fields_ = [("size",U),("width",I),("height",I),("planes",c.c_uint16),("bits",c.c_uint16),("compression",U),("bytes",U),("xppm",I),("yppm",I),("used",U),("important",U)]
    gdi = c.WinDLL("gdi32")
    user.GetDC.argtypes=[P]; user.GetDC.restype=P
    user.ReleaseDC.argtypes=[P,P]
    user.UpdateWindow.argtypes=[P]
    gdi.CreateCompatibleDC.argtypes=[P];gdi.CreateCompatibleDC.restype=P
    gdi.CreateCompatibleBitmap.argtypes=[P,I,I];gdi.CreateCompatibleBitmap.restype=P
    gdi.SelectObject.argtypes=[P,P];gdi.SelectObject.restype=P
    gdi.BitBlt.argtypes=[P,I,I,I,I,P,I,I,U]
    gdi.GetDIBits.argtypes=[P,P,U,U,P,c.POINTER(Header),U]
    gdi.DeleteObject.argtypes=[P];gdi.DeleteDC.argtypes=[P]
    user.UpdateWindow(window)
    c.WinDLL("dwmapi").DwmFlush()
    _,_,width,height=bounds(window,client=True)
    dc=user.GetDC(window); memory=gdi.CreateCompatibleDC(dc)
    bitmap=gdi.CreateCompatibleBitmap(dc,width,height)
    old=gdi.SelectObject(memory,bitmap)
    try:
        assert gdi.BitBlt(memory,0,0,width,height,dc,0,0,0xcc0020)
        gdi.SelectObject(memory,old)
        header=Header();header.size=c.sizeof(header);header.width=width;header.height=-height;header.planes=1;header.bits=32
        pixels=(c.c_ubyte*(width*height*4))()
        assert gdi.GetDIBits(memory,bitmap,0,height,pixels,c.byref(header),0)==height
        data=bytes(pixels)
        assert max(data[0::4])-min(data[0::4])>10, "client capture must contain rendered text, not a blank surface"
        return width,height,data
    finally:
        gdi.SelectObject(memory,old);gdi.DeleteObject(bitmap);gdi.DeleteDC(memory);user.ReleaseDC(window,dc)

def save_preview(index, name, image):
    directory=os.environ.get("MELTYPE_CANDIDATE_PREVIEW")
    if directory:
        directory=Path(directory);directory.mkdir(parents=True,exist_ok=True)
        width,height,pixels=image
        header=struct.pack('<IiiHHIIiiII',40,width,-height,1,32,0,len(pixels),0,0,0,0)
        (directory/f'{index}-{name}.bmp').write_bytes(struct.pack('<2sIHHI',b'BM',54+len(pixels),0,0,54)+header+pixels)

class CandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        ole = c.WinDLL("ole32")
        assert ole.CoInitializeEx(None, 2) in [0, 1]
        cls.addClassCleanup(ole.CoUninitialize)
    def setUp(self):
        self.backend = Backend(sys.argv[1])
        self.addCleanup(self.backend.close)
    def test_page_geometry_click_mapping_and_focus(self):
        foreground = user.GetForegroundWindow()
        window = self.backend.show(view(10, typed="kouho", suggestion="もしかして"))
        self.assertEqual(user.GetForegroundWindow(), foreground)
        self.assertEqual(user.SendMessageW(window, 0x21, 0, 0), 3) # MA_NOACTIVATE
        _, _, _, height = bounds(window, client=True)
        scale = user.GetDpiForWindow(window) / 96
        self.assertEqual(height, int((8+22+37+9*32+24)*scale+0.5))
        # The second row on page two is the absolute index 10.
        y = int((4+22+37+32+16)*scale)
        user.SendMessageW(window, 0x202, 0, y << 16)
        self.assertEqual(self.backend.events, [(10, 0)])
        self.assertFalse(self.backend.api.meltype_tsf_candidate_visible(self.backend.handle))
    def test_prediction_click_and_reentrant_destroy(self):
        window = self.backend.show(view(0, 3, prediction=True))
        scale = user.GetDpiForWindow(window) / 96
        self.backend.destroy_on_click = True
        user.SendMessageW(window, 0x202, 0, int((4+32+16)*scale) << 16)
        self.assertEqual(self.backend.events, [(1, 1)])
        self.assertFalse(windows(self.backend.name))
    def test_meaning_delay_and_hide_cancels_timer(self):
        self.backend.show(view(0, 3, meaning="候補の意味。折り返す文章です。"))
        self.backend.api.meltype_tsf_candidate_hide(self.backend.handle)
        pump(1.6)
        self.assertFalse(any(user.IsWindowVisible(w) for w in windows(self.backend.meaning_name)))
        start = time.monotonic()
        self.backend.show(view(1, 3, meaning="別の候補の意味です。"))
        wait(lambda: any(user.IsWindowVisible(w) for w in windows(self.backend.meaning_name)), "meaning timer")
        self.assertGreaterEqual(time.monotonic()-start, 1.4)
        self.backend.api.meltype_tsf_candidate_hide(self.backend.handle)
        self.assertFalse(any(user.IsWindowVisible(w) for w in windows(self.backend.meaning_name)))
    def test_screen_edge_placement_and_suggestion_only(self):
        width, height = user.GetSystemMetrics(0), user.GetSystemMetrics(1)
        window = self.backend.show(view(0,3), Rect(width-2,height-2,width-1,height-1))
        left, top, right, bottom = bounds(window)
        self.assertGreaterEqual(left,0); self.assertGreaterEqual(top,0)
        self.assertLessEqual(right,width); self.assertLessEqual(bottom,height)
        window = self.backend.show({"candidates":[], "selected":-1, "typed":"foo", "suggestion":"もしかして"})
        scale = user.GetDpiForWindow(window) / 96
        self.assertEqual(bounds(window,client=True)[3], int((8+22+32)*scale+0.5))
        user.SendMessageW(window,0x202,0,int(30*scale)<<16)
        self.assertEqual(self.backend.events,[])
    def test_stale_and_foreign_tokens_fail_without_windows(self):
        result = []
        worker = threading.Thread(target=lambda: result.append(self.backend.api.meltype_tsf_candidate_visible(self.backend.handle)))
        worker.start(); worker.join(timeout=5)
        self.assertFalse(worker.is_alive()); self.assertEqual(result,[0])
        stale = self.backend.handle
        self.backend.close()
        self.assertEqual(self.backend.api.meltype_tsf_candidate_show(stale,b"{}",c.byref(Rect())),0)
        self.assertEqual(self.backend.api.meltype_tsf_candidate_visible(stale),0)
        self.backend.api.meltype_tsf_candidate_destroy(stale)
    def test_multiple_ui_threads_keep_independent_windows_and_callbacks(self):
        barrier=threading.Barrier(2)
        errors=[];results=[]
        def worker(prediction):
            backend=None;ole=c.WinDLL("ole32")
            initialized=ole.CoInitializeEx(None,2) in [0,1]
            try:
                assert initialized
                backend=Backend(sys.argv[1])
                window=backend.show(view(0,3,prediction=prediction))
                barrier.wait(timeout=5)
                scale=user.GetDpiForWindow(window)/96
                user.SendMessageW(window,0x202,0,int((4+32+16)*scale)<<16)
                results.append(backend.events)
            except BaseException as error: errors.append(error)
            finally:
                if backend: backend.close()
                if initialized: ole.CoUninitialize()
        threads=[threading.Thread(target=worker,args=(prediction,),daemon=True) for prediction in [False,True]]
        for thread in threads: thread.start()
        for thread in threads: thread.join(timeout=10)
        self.assertFalse(any(thread.is_alive() for thread in threads))
        self.assertEqual(errors,[])
        self.assertCountEqual(results,[[(1,0)],[(1,1)]])
    def test_rendering_matches_upstream_cpp(self):
        if len(sys.argv)<3: self.skipTest("Pass the test-only upstream renderer DLL")
        reference=Backend(sys.argv[2],reference=True)
        self.addCleanup(reference.close)
        cases=[view(0,3),view(10,typed="kouho",suggestion="もしかして"),view(1,3,prediction=True),
            {"candidates":[],"selected":-1,"typed":"foo","suggestion":"もしかして"},
            {"candidates":["橋","箸","😀","🧑‍💻"],"notes":["bridge","chopsticks"],"selected":2},
            view(1,3,meaning="候補の意味。長い文章を表示して、正しい位置で折り返すことを確認します。"),
        ]
        for index,case in enumerate(cases):
            with self.subTest(case=index):
                images=[];geometry=[]
                for backend in [self.backend,reference]:
                    window=backend.show(case)
                    user.SendMessageW(window,0x2a3,0,0) # remove pointer hover
                    geometry.append(bounds(window));images.append(capture(window))
                    if case.get("meaning"):
                        user.SendMessageW(window,0x113,1,0)
                        meaning=next(w for w in windows(backend.meaning_name) if user.IsWindowVisible(w))
                        images.append(capture(meaning))
                    backend.api.meltype_tsf_candidate_hide(backend.handle)
                    pump()
                self.assertEqual(geometry[0],geometry[1])
                if case.get("meaning"):
                    pairs=[(images[0],images[2]),(images[1],images[3])]
                else: pairs=[(images[0],images[1])]
                for pane,(actual,expected) in enumerate(pairs):
                    save_preview(f'{index}-{pane}',"rust",actual);save_preview(f'{index}-{pane}',"cpp",expected)
                    self.assertEqual(actual[:2],expected[:2])
                    self.assertTrue(actual[2]==expected[2], f'case {index}, pane {pane}: Rust pixels differ from upstream')

if __name__ == "__main__":
    unittest.main(argv=[sys.argv[0]], verbosity=2, failfast=True)
