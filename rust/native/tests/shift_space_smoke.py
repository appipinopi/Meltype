# SPDX-License-Identifier: GPL-3.0-or-later
"""Original Shift+Space traces with real converter callbacks through the C ABI."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile

filename='meltype_native.dll' if os.name=='nt' else ('libmeltype_native.dylib' if sys.platform=='darwin' else 'libmeltype_native.so')
library=sys.argv[1] if len(sys.argv)>1 else os.path.abspath(os.path.join('target','release',filename))
api=c.CDLL(library)
allocator=c.CDLL('ucrtbase') if os.name=='nt' else c.CDLL(None)
allocator.malloc.argtypes=[c.c_size_t];allocator.malloc.restype=c.c_void_p
clause_type=c.CFUNCTYPE(c.c_void_p,c.c_char_p,c.c_char_p)
candidate_type=c.CFUNCTYPE(c.c_void_p,c.c_char_p)
queries=[]
def allocate(text):
    data=text.encode()+b'\0';pointer=allocator.malloc(len(data));assert pointer
    c.memmove(pointer,data,len(data));return pointer
@clause_type
def clauses(raw,_context):
    reading=raw.decode();queries.append(reading)
    return allocate(reading+'\t【'+reading+'】')
@candidate_type
def candidates(raw):
    reading=raw.decode()
    return allocate('【'+reading+'】\n別:'+reading)
api.meltype_init.argtypes=[clause_type,candidate_type,c.c_void_p];api.meltype_init.restype=c.c_int
api.meltype_create_at.argtypes=[c.c_char_p];api.meltype_create_at.restype=c.c_void_p
api.meltype_destroy.argtypes=[c.c_void_p];api.meltype_destroy.restype=None
api.meltype_free.argtypes=[c.c_void_p];api.meltype_free.restype=None
api.meltype_handle_key.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_handle_key.restype=c.c_void_p
api.meltype_control.argtypes=[c.c_void_p,c.c_char_p];api.meltype_control.restype=c.c_void_p
assert api.meltype_init(clauses,candidates,None)==1
def result(pointer):
    assert pointer
    try:return json.loads(c.string_at(pointer).decode())
    finally:api.meltype_free(pointer)
golden=json.loads((Path(__file__).resolve().parents[2]/'core/tests/data/upstream-engine-keys.json').read_text(encoding='utf-8'))
rows=[row for row in golden['cases'] if row['case']['action']=='shift-space']
assert len(rows)==20
with tempfile.TemporaryDirectory(prefix='meltype-shift-space-') as directory:
    for number,row in enumerate(rows):
        case=row['case'];profile=Path(directory,str(number));profile.mkdir()
        handle=api.meltype_create_at(str(profile).encode());assert handle
        def key(vk=0,ch=0,modifiers=0):return result(api.meltype_handle_key(handle,vk,ch,modifiers,None,None))
        try:
            for name,value in dict(LiveConversion=False,PredictiveCandidates=False,LearningEnabled=False,DetectionLevel=case.get('level','Manual')).items():
                assert result(api.meltype_control(handle,json.dumps(dict(op='settings_set',name=name,value=value)).encode()))['saved']
            empty=key(32,32,1)
            assert empty['view'] is None and empty['commits']==[dict(deleteBefore=0,text='　')],empty
            for char in case['raw']:key(ord(char.upper()),ord(char),int(char.isupper()))
            if case['force']:key(case['force'])
            before=len(queries)
            converted=key(32,32,1)
            assert converted['commits']==[] and converted['view']['converting'],(case,converted)
            assert len(queries)>before,(case,queries)
            view=converted['view']
            assert view['text']==row['expected']['committed'],(case,view,row['expected'])
            reverse=key(32,32,1)
            assert reverse['commits']==[] and reverse['view']['selectedIndex']==len(view['candidates'])-1,(case,reverse)
            forward=key(32,32)
            assert forward['view']['selectedIndex']==0,(case,forward)
            assert key(13)['commits']==[dict(deleteBefore=0,text=row['expected']['committed'])]
        finally:api.meltype_destroy(handle)
print('Native ABI: 20 original Shift+Space conversions, backward/forward candidate cycles and empty fullwidth spaces passed')
