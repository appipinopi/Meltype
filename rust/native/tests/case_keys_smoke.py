# SPDX-License-Identifier: GPL-3.0-or-later
"""Original F9/F10 visible case changes through actual native scalar keys."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile
filename='meltype_native.dll' if os.name=='nt' else ('libmeltype_native.dylib' if sys.platform=='darwin' else 'libmeltype_native.so')
library=sys.argv[1] if len(sys.argv)>1 else os.path.abspath(os.path.join('target','release',filename))
api=c.CDLL(library)
api.meltype_init.argtypes=[c.c_void_p,c.c_void_p,c.c_void_p];api.meltype_init.restype=c.c_int
api.meltype_create_at.argtypes=[c.c_char_p];api.meltype_create_at.restype=c.c_void_p
api.meltype_destroy.argtypes=[c.c_void_p];api.meltype_destroy.restype=None
api.meltype_free.argtypes=[c.c_void_p];api.meltype_free.restype=None
api.meltype_handle_key.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_handle_key.restype=c.c_void_p
api.meltype_control.argtypes=[c.c_void_p,c.c_char_p];api.meltype_control.restype=c.c_void_p
assert api.meltype_init(None,None,None)==1
def result(pointer):
    assert pointer
    try:return json.loads(c.string_at(pointer).decode())
    finally:api.meltype_free(pointer)
fixture=json.loads((Path(__file__).resolve().parents[2]/'core/tests/data/upstream-case-keys.json').read_text(encoding='utf-8'))
assert len(fixture['cases'])==96
with tempfile.TemporaryDirectory(prefix='meltype-case-keys-') as directory:
    for number,row in enumerate(fixture['cases']):
        case=row['case'];profile=Path(directory,str(number));profile.mkdir()
        handle=api.meltype_create_at(str(profile).encode());assert handle
        def key(vk=0,ch=0,mods=0):return result(api.meltype_handle_key(handle,vk,ch,mods,None,None))
        try:
            for name,value in dict(LiveConversion=False,PredictiveCandidates=False,LearningEnabled=False,SigilWordsDirect=False,DetectionLevel=case['level']).items():
                assert result(api.meltype_control(handle,json.dumps(dict(op='settings_set',name=name,value=value)).encode()))['saved']
            for char in case['raw']:key(ord(char.upper()),ord(char),int(char.isupper()))
            for vk,expected in zip(case['keys'],row['expected'],strict=True):
                value=key(vk);view=value['view'];assert value['commits']==[]
                actual=dict(consumed=value['consumed'],text=view['text'] if view else None,converting=view['converting'] if view else False)
                assert actual==expected,(case['name'],vk,actual,expected)
            assert key(13)['commits']==[dict(deleteBefore=0,text=row['expected'][-1]['text'])]
        finally:api.meltype_destroy(handle)
print('Native ABI: 96 original F9/F10 case-switch sequences and final commits passed')
