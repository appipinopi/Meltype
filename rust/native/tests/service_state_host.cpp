// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Production state-owner adapter: checked ABI, defaults and exception teardown.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
int wmain() {
    assert(RustServiceStateLiveCount()==0 && g_locks==0);
    {
        RustServiceStateStorage storage;
        assert(storage.value && RustServiceStateLiveCount()==1 && g_locks==1);
        assert(storage.value->refs==1 && storage.value->open && storage.value->consumed);
        assert(!storage.value->manager && !storage.value->composition && !storage.value->deactivated);
        assert(storage.value->client==TF_CLIENTID_NULL && storage.value->atom_input==TF_INVALID_GUIDATOM);
        storage.value->last_hello=0xfedcba9876543210ULL;
        storage.value->mode_epoch=0x100000001ULL;
        assert(storage.value->last_hello==0xfedcba9876543210ULL && storage.value->mode_epoch==0x100000001ULL);
    }
    assert(RustServiceStateLiveCount()==0 && g_locks==0);
    try {
        RustServiceStateStorage storage;
        assert(RustServiceStateLiveCount()==1 && g_locks==1);
        throw 7;
    }catch (int value) {assert(value==7);}
    assert(RustServiceStateLiveCount()==0 && g_locks==0);
    auto* first=new TextService();auto* second=new TextService();
    assert(first->IsOpen() && second->IsOpen());
    assert(RustServiceStateLiveCount()==2 && g_locks==2);
    first->AddRef();first->Release();assert(RustServiceStateLiveCount()==2);
    first->Release();assert(RustServiceStateLiveCount()==1 && g_locks==1);
    second->Release();assert(RustServiceStateLiveCount()==0 && g_locks==0);
    puts("Actual service state: checked Rust layout, native allocation/defaults, 64-bit fields, exception cleanup, independent COM owners and balanced shell pins passed");
}
