# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Real OS word-callback bridge, resolved outside all native session locks."""
import ctypes as c,json,sys,tempfile
from pathlib import Path
api=c.CDLL(str(Path(sys.argv[1]).resolve()))
P=c.c_void_p
Word=c.CFUNCTYPE(c.c_int,c.c_char_p)
api.meltype_init.argtypes=[P,P,P]
api.meltype_create_at.argtypes=[c.c_char_p];api.meltype_create_at.restype=P
api.meltype_destroy.argtypes=[P]
api.meltype_handle_key.argtypes=[P,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_handle_key.restype=P
api.meltype_macos_handle_key.argtypes=api.meltype_handle_key.argtypes;api.meltype_macos_handle_key.restype=P
api.meltype_set_can_delete.argtypes=[P,c.c_int]
api.meltype_undo_commit.argtypes=[P,c.c_char_p];api.meltype_undo_commit.restype=P
api.meltype_free.argtypes=[P]
api.meltype_commit.argtypes=[P];api.meltype_commit.restype=P
api.meltype_word_check_count.restype=c.c_uint64
api.meltype_set_private.argtypes=[P,c.c_int]
api.meltype_set_direct.argtypes=[P,c.c_int]
seen=[]
accepted=True
@Word
def word(value):
    seen.append(value.decode())
    return int(accepted and value in (b"puretuma",b"puretum"))
def read(pointer):
    assert pointer
    try:return json.loads(c.string_at(pointer))
    finally:api.meltype_free(pointer)
with tempfile.TemporaryDirectory(prefix="meltype-word-provider-") as temporary:
    root=Path(temporary)
    (root/"config.json").write_text('{"DetectionLevel":"Conservative","LiveConversion":false,"PredictiveCandidates":false,"SigilWordsDirect":false}',encoding="utf-8")
    def handle():
        value=api.meltype_create_at(str(root).encode())
        assert value
        return value
    def type_word(h):
        for ch in "puretuma":
            result=read(api.meltype_handle_key(h,ord(ch.upper()),ord(ch),0,b"we use ",None))
        return result
    api.meltype_init(None,None,word)
    first=handle()
    try:
        positive=type_word(first)
        print(json.dumps(positive,ensure_ascii=True))
        assert positive["view"]["text"]=="puretuma",positive
        invalid=read(api.meltype_macos_handle_key(first,7,-1,0,None,None))
        assert invalid["view"]==positive["view"],invalid
    finally:api.meltype_destroy(first)
    calls=len(seen)
    second=handle()
    try:assert type_word(second)["view"]["text"]=="puretuma"
    finally:api.meltype_destroy(second)
    assert len(seen)==calls,"positive/negative prefix cache missed"
    accepted=False
    api.meltype_init(None,None,word)
    third=handle()
    try:
        negative=type_word(third)
        assert negative["view"]["text"]!="puretuma",negative
    finally:api.meltype_destroy(third)
    assert len(seen)>calls,"re-registration did not invalidate evidence"
    balanced=root/"balanced";balanced.mkdir()
    (balanced/"config.json").write_text('{"DetectionLevel":"Balanced","LiveConversion":false,"PredictiveCandidates":false}',encoding="utf-8")
    accepted=True;api.meltype_init(None,None,word)
    h=api.meltype_create_at(str(balanced).encode())
    try:
        for ch in "puretum":positive=read(api.meltype_handle_key(h,ord(ch.upper()),ord(ch),0,None,None))
        assert positive["view"]["text"]=="puretum",positive
        committed=read(api.meltype_handle_key(h,32,32,0,None,None))
        assert [edit["text"] for edit in committed["commits"]]==["puretum "],committed
        api.meltype_set_can_delete(h,1)
        restored=read(api.meltype_undo_commit(h,b"puretum "))
        assert restored["view"] and restored["view"]["text"]=="puretum",restored
    finally:api.meltype_destroy(h)
    for private in (True,False):
        h=handle();calls=len(seen)
        try:
            (api.meltype_set_private if private else api.meltype_set_direct)(h,1)
            type_word(h)
            assert len(seen)==calls,"private/direct input queried host checker"
        finally:api.meltype_destroy(h)
    errors=[]
    @Word
    def reentrant(value):
        try:
            other=handle();api.meltype_destroy(other)
            api.meltype_word_check_count()
        except BaseException as error:errors.append(str(error))
        return int(value==b"puretuma")
    api.meltype_init(None,None,reentrant)
    h=handle()
    try:assert type_word(h)["view"]["text"]=="puretuma"
    finally:api.meltype_destroy(h)
    assert not errors,errors
    victim=handle()
    destroyed=[]
    @Word
    def destroy_current(value):
        destroyed.append(value)
        api.meltype_destroy(victim)
        return 1
    api.meltype_init(None,None,destroy_current)
    read(api.meltype_handle_key(victim,80,112,0,b"we use ",None))
    assert not api.meltype_handle_key(victim,85,117,0,b"we use ",None)
    assert len(destroyed)==1,"destroyed session continued requesting OS evidence"
    api.meltype_destroy(victim)
    @Word
    def replace_provider(value):
        api.meltype_init(None,None,None)
        return 1
    api.meltype_init(None,None,replace_provider)
    h=handle()
    try:
        read(api.meltype_handle_key(h,80,112,0,b"we use ",None))
        # The first one-letter prefix requires no host word lookup; a two-letter
        # event must stop rather than applying the replaced provider's answer.
        pointer=api.meltype_handle_key(h,85,117,0,b"we use ",None)
        assert not pointer,"changed provider continued current key"
        committed=read(api.meltype_commit(h))
        assert [edit["text"] for edit in committed["commits"]]==["p"],committed
    finally:api.meltype_destroy(h)
    api.meltype_init(None,None,None)
print("OS word evidence: classification, positive/negative cache, provider replacement, private/direct suppression and reentrant session access passed")
