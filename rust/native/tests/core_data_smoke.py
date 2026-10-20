# SPDX-License-Identifier: GPL-3.0-or-later
"""Exercise upstream core/data key traces through the public native ABI."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile

filename = 'meltype_native.dll' if os.name == 'nt' else 'libmeltype_native.dylib' if sys.platform == 'darwin' else 'libmeltype_native.so'
api = c.CDLL(str(Path(sys.argv[1] if len(sys.argv)>1 else Path('target/release')/filename).resolve()))
pointer = c.c_void_p
api.meltype_init.argtypes = [pointer, pointer, pointer]
api.meltype_init.restype = c.c_int
api.meltype_create_at.argtypes = [c.c_char_p]
api.meltype_create_at.restype = pointer
api.meltype_handle_key.argtypes = [pointer,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p]
api.meltype_handle_key.restype = pointer
api.meltype_set_can_delete.argtypes = [pointer,c.c_int]
api.meltype_destroy.argtypes = [pointer]
api.meltype_free.argtypes = [pointer]
assert api.meltype_init(None,None,None) == 1
fixture = json.loads((Path(__file__).resolve().parents[2] / 'core/tests/data/upstream-core-data.json').read_text(encoding='utf-8'))
with tempfile.TemporaryDirectory(prefix='meltype-core-data-') as directory:
    for index, row in enumerate(fixture['cases']):
        profile = Path(directory,str(index))
        profile.mkdir()
        (profile/'config.json').write_text(json.dumps(row['settings']),encoding='utf-8')
        for name,text in row.get('files',{}).items():
            path = profile/name
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_text(text,encoding='utf-8')
        handle = api.meltype_create_at(str(profile).encode())
        assert handle
        api.meltype_set_can_delete(handle,int(row['canDelete']))
        try:
            for key_index,(key,want) in enumerate(zip(row['keys'],row['expected'],strict=True)):
                result = api.meltype_handle_key(handle,*key,row['before'].encode(),row['after'].encode())
                assert result
                try:
                    got = json.loads(c.string_at(result))
                finally:
                    api.meltype_free(result)
                view = got['view']
                got = dict(consumed=got['consumed'],commits=got['commits'],view={k:view[k] for k in ['text','converting','suggestion']} if view else None)
                assert got == want, (row['name'],key_index,key,want,got)
        finally:
            api.meltype_destroy(handle)
print(f"Native ABI: {len(fixture['cases'])} upstream core/data key traces passed")
