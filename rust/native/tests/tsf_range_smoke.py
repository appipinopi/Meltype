# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Controlled ITfContext/ITfRange COM host; no real document is accessed.

Positions and document storage are UTF-16 units, matching TSF rather than Python
Unicode indices. The Rust implementation calls real SDK-layout COM vtables.
"""
import ctypes as c
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parent))
from tsf_com_smoke import Guid,P,U32,HR
W=c.c_uint16
FAIL=HR(0x80004005).value
def units(text):
    data=text.encode("utf-16-le",errors="surrogatepass")
    return list((W*(len(data)//2)).from_buffer_copy(data))
def buffer(text):
    value=units(text)
    return (W*len(value))(*value)
class Selection(c.Structure):
    _fields_=[("range",P),("active_end",HR),("interim",HR)]
RECEIVER=c.CFUNCTYPE(None,P,c.POINTER(W),U32,c.POINTER(W),U32)

class Host:
    class Object(c.Structure):
        _fields_=[("vtable",c.POINTER(P))]
    def __init__(self,text,start=None,end=None):
        self.document=units(text)
        self.selection=(len(self.document) if start is None else start,len(self.document) if end is None else end)
        self.ranges={};self.callbacks=[];self.objects=[];self.errors=[]
        self.fail_read=False;self.fail_write=False;self.fail_selection=False;self.short_final=False
        self.writes=[];self.clears=0;self.property_refs=1
        self.range_table=(P*25)();self.context_table=(P*17)();self.property_table=(P*11)()
        self.bind(self.range_table,1,U32,[],lambda this:self.ref(this,1))
        self.bind(self.range_table,2,U32,[],lambda this:self.ref(this,-1))
        self.bind(self.range_table,3,HR,[U32,U32,c.POINTER(W),U32,c.POINTER(U32)],self.read)
        self.bind(self.range_table,4,HR,[U32,U32,c.POINTER(W),HR],self.write)
        self.bind(self.range_table,8,HR,[U32,HR,c.POINTER(HR),P],self.shift_start)
        self.bind(self.range_table,9,HR,[U32,HR,c.POINTER(HR),P],self.shift_end)
        self.bind(self.range_table,14,HR,[U32,c.POINTER(HR)],self.empty)
        self.bind(self.range_table,15,HR,[U32,HR],self.collapse)
        self.bind(self.range_table,18,HR,[U32,P,HR,c.POINTER(HR)],self.compare_start)
        self.bind(self.range_table,19,HR,[U32,P,HR,c.POINTER(HR)],self.compare_end)
        self.bind(self.range_table,23,HR,[c.POINTER(P)],self.clone)
        self.bind(self.context_table,5,HR,[U32,U32,U32,c.POINTER(Selection),c.POINTER(U32)],self.get_selection)
        self.bind(self.context_table,6,HR,[U32,U32,c.POINTER(Selection)],self.set_selection)
        self.bind(self.context_table,12,HR,[c.POINTER(Guid),c.POINTER(P)],self.get_property)
        self.bind(self.property_table,1,U32,[],self.property_add)
        self.bind(self.property_table,2,U32,[],self.property_release)
        self.bind(self.property_table,10,HR,[U32,P],self.clear)
        self.context=self.Object(self.context_table);self.property=self.Object(self.property_table)
        self.original=self.new_range(*self.selection)
    def bind(self,table,index,result,args,method):
        def protected(*values):
            try: return method(*values)
            except BaseException as error:
                self.errors.append(repr(error))
                return FAIL if result is HR else 0
        callback=c.WINFUNCTYPE(result,P,*args)(protected)
        self.callbacks.append(callback);table[index]=c.cast(callback,P).value
    def new_range(self,start,end):
        value=self.Object(self.range_table);address=c.addressof(value)
        self.objects.append(value);self.ranges[address]=[start,end,1]
        return address
    def ref(self,this,delta):
        self.ranges[this][2]+=delta
        assert self.ranges[this][2]>=0
        return self.ranges[this][2]
    def read(self,this,cookie,flags,out,limit,count):
        assert cookie==7
        count[0]=0
        if self.fail_read: return FAIL
        start,end,_=self.ranges[this]
        text=self.document[start:end][:min(limit,getattr(self,'read_limit',limit))]
        for index,value in enumerate(text): out[index]=value
        count[0]=len(text);return 0
    def write(self,this,cookie,flags,text,length):
        assert cookie==7
        if self.fail_write: return FAIL
        start,end,_=self.ranges[this]
        value=list(text[:length]);self.writes.append((start,end,value))
        self.document[start:end]=value
        self.ranges[this][1]=start+length
        return 0
    def shift_start(self,this,cookie,amount,moved,halt):
        assert cookie==7
        start,end,_=self.ranges[this]
        if self.short_final and this==self.original: amount+=1
        target=max(0,min(end,start+amount))
        self.ranges[this][0]=target;moved[0]=target-start;return 0
    def shift_end(self,this,cookie,amount,moved,halt):
        assert cookie==7
        start,end,_=self.ranges[this]
        target=min(len(self.document),max(start,end+amount))
        self.ranges[this][1]=target;moved[0]=target-end;return 0
    def empty(self,this,cookie,out):
        start,end,_=self.ranges[this];out[0]=int(start==end);return 0
    def collapse(self,this,cookie,anchor):
        assert cookie==7
        value=self.ranges[this]
        if anchor==0: value[1]=value[0]
        else: value[0]=value[1]
        return 0
    def clone(self,this,out):
        start,end,_=self.ranges[this];out[0]=self.new_range(start,end);return 0
    def compare_start(self,this,cookie,other,anchor,out):
        assert cookie==7
        left=self.ranges[this][0];right=self.ranges[other][anchor]
        out[0]=(left>right)-(left<right);return 0
    def compare_end(self,this,cookie,other,anchor,out):
        assert cookie==7
        left=self.ranges[this][1];right=self.ranges[other][anchor]
        out[0]=(left>right)-(left<right);return 0
    def get_selection(self,this,cookie,index,count,out,fetched):
        assert cookie==7 and count==1
        out[0].range=self.original;self.ref(self.original,1)
        fetched[0]=1
        return FAIL if self.fail_selection else 0
    def set_selection(self,this,cookie,count,values):
        assert cookie==7 and count==1
        assert values[0].active_end==0 and values[0].interim==0
        start,end,_=self.ranges[values[0].range];self.selection=(start,end);return 0
    def get_property(self,this,guid,out):
        self.property_refs+=1;out[0]=c.addressof(self.property);return 0
    def property_add(self,_):
        self.property_refs+=1;return self.property_refs
    def property_release(self,_):
        self.property_refs-=1;return self.property_refs
    def clear(self,this,cookie,range):
        assert cookie==7 and range==self.original
        self.clears+=1;return 0
    def verify(self,test):
        test.assertEqual(self.errors,[])
        test.assertEqual(self.property_refs,1)
        test.assertTrue(all(value[2]==(1 if key==self.original else 0) for key,value in self.ranges.items()),self.ranges)

class RangeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.api=c.CDLL(str(Path(sys.argv[1]).resolve()))
        cls.api.meltype_tsf_extend_previous.argtypes=[P,U32,HR,c.POINTER(W),U32]
        cls.api.meltype_tsf_write_range.argtypes=[P,P,U32,c.POINTER(W),U32,HR]
        cls.api.meltype_tsf_read_surrounding.argtypes=[P,U32,P,RECEIVER]
    def host(self,*args):
        value=Host(*args);self.addCleanup(value.verify,self);return value
    def extend(self,host,expected,count=None):
        text=buffer(expected)
        return self.api.meltype_tsf_extend_previous(host.original,7,len(text) if count is None else count,text,len(text))
    def write(self,host,text,keep=False):
        value=buffer(text)
        return self.api.meltype_tsf_write_range(c.byref(host.context),host.original,7,value,len(value),int(keep))
    def test_verified_utf16_replacement_moves_caret_and_releases_ranges(self):
        host=self.host("before 😀")
        self.assertEqual(self.extend(host,"😀"),1)
        self.assertEqual(host.document,units("before 😀"))
        self.assertEqual(self.write(host,"あ😀"),1)
        self.assertEqual(host.document,units("before あ😀"))
        self.assertEqual(host.selection,(len(host.document),len(host.document)))
        self.assertEqual(host.clears,1)
    def test_mismatch_missing_read_length_and_nonempty_selection_never_write(self):
        for expected,count,selection,fail in [("abc",None,None,False),("😀",1,None,False),("😀",None,(0,2),False),("😀",None,None,True),("prefix😀",None,None,False)]:
            with self.subTest(expected=expected,count=count,selection=selection,fail=fail):
                host=self.host("😀",*(selection or (None,None)));host.fail_read=fail
                self.assertEqual(self.extend(host,expected,count),0)
                self.assertEqual(host.document,units("😀"));self.assertEqual(host.writes,[])
                self.assertEqual(host.ranges[host.original][:2],list(host.selection))
    def test_partial_final_shift_rejects_without_touching_document(self):
        host=self.host("abc");host.short_final=True
        self.assertEqual(self.extend(host,"abc"),0)
        self.assertEqual(host.document,units("abc"));self.assertEqual(host.writes,[])
    def test_keep_caret_failed_write_and_empty_commit(self):
        host=self.host("old")
        self.assertEqual(self.write(host,"new",keep=True),1)
        self.assertEqual(host.selection,(3,3))
        host.fail_write=True
        self.assertEqual(self.write(host,"fail"),0)
        self.assertEqual(host.document,units("oldnew"));self.assertEqual(host.selection,(3,3))
        host.fail_write=False
        self.assertEqual(self.write(host,""),1)
        self.assertEqual(host.document,units("old"))
    def test_surrounding_limits_and_selection_endpoints_use_utf16(self):
        host=self.host("a"*4500+"😀選択"+"0123456789"*3,4500,4504)
        received=[]
        callback=RECEIVER(lambda context,before,n,after,m:received.append((context,list(before[:n]),list(after[:m]))))
        self.assertEqual(self.api.meltype_tsf_read_surrounding(c.byref(host.context),7,123,callback),1)
        self.assertEqual(received,[(123,units("a"*4000),units("0123456789"*2))])
        self.assertEqual(host.selection,(4500,4504));self.assertEqual(host.writes,[])
    def test_host_selection_error_releases_populated_output_reference(self):
        host=self.host("hello");host.fail_selection=True
        received=[];callback=RECEIVER(lambda *args:received.append(args))
        self.assertEqual(self.api.meltype_tsf_read_surrounding(c.byref(host.context),7,None,callback),0)
        self.assertEqual(received,[])

if __name__=="__main__":
    unittest.main(argv=[sys.argv[0]],verbosity=2,failfast=True)
