// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Production state-owner adapter: checked ABI, defaults and exception teardown.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
static_assert(!std::is_polymorphic<TextService>::value,"Rust build must not emit C++ service vtables");
static_assert(!std::is_base_of<IUnknown,TextService>::value,"COM identity belongs only to Rust");
int wmain() {
    wchar_t executable[32768]={};
    assert(GetModuleFileNameW(nullptr,executable,ARRAYSIZE(executable))>0);
    const wchar_t* base=wcsrchr(executable,L'\\');base=base?base+1:executable;
    std::wstring expected(base);
    if (expected.size()>4 && _wcsicmp(expected.c_str()+expected.size()-4,L".exe")==0) expected.resize(expected.size()-4);
    assert(RustProcessName()==expected);
    assert(RustServiceStateLiveCount()==0 && g_locks==0);
    {
        RustServiceStateStorage storage;
        assert(storage.value && RustServiceStateLiveCount()==1 && g_locks==1);
        assert(RustServiceSid(storage.value).empty());
        RustServiceSetSid(storage.value,"shell-session");
        assert(RustServiceSid(storage.value)=="shell-session");
        int toggles=0;
        auto* bar=RustServiceCreateBar(storage.value,&toggles,
            [](void*)->int {return 0;},[](void* p) {++*static_cast<int*>(p);});
        assert(bar && storage.value->langbar==bar && g_locks==1);
        bar->AddRef();RustServiceUpdateBar(storage.value);
        RustServiceRemoveBar(storage.value,nullptr);assert(!storage.value->langbar);
        auto* button=static_cast<ITfLangBarItemButton*>(bar);
        assert(button->OnClick(TF_LBI_CLK_LEFT,POINT{},nullptr)==S_OK && toggles==0);
        assert(bar->Release()==0);
        RustServiceClose(storage.value);assert(!storage.value->transport);
        RustServiceClose(storage.value);
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
    for (const IID* iid : {&IID_IUnknown,&IID_ITfTextInputProcessor,&IID_ITfTextInputProcessorEx,&IID_ITfThreadMgrEventSink,&IID_ITfKeyEventSink,&IID_ITfCompositionSink,&IID_ITfDisplayAttributeProvider,&IID_ITfCompartmentEventSink,&IID_ITfTextEditSink}) {
        IUnknown* value=nullptr;assert(first->QueryInterface(*iid,reinterpret_cast<void**>(&value))==S_OK && value);
        auto** table=*reinterpret_cast<void***>(value);HMODULE native=nullptr;
        assert(GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS|GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,reinterpret_cast<LPCWSTR>(table[0]),&native));
        wchar_t path[32768]={};assert(GetModuleFileNameW(native,path,ARRAYSIZE(path)));
        const auto* file=wcsrchr(path,L'\\');assert(file && _wcsicmp(file+1,L"meltype_native.dll")==0);
        IUnknown* identity=nullptr;assert(value->QueryInterface(IID_IUnknown,reinterpret_cast<void**>(&identity))==S_OK);
        assert(identity==first->ServiceInterface<IUnknown>());identity->Release();value->Release();
    }
    assert(RustServiceStateLiveCount()==2 && g_locks==2);
    first->AddRef();first->Release();assert(RustServiceStateLiveCount()==2);
    first->Release();assert(RustServiceStateLiveCount()==1 && g_locks==1);
    second->Release();assert(RustServiceStateLiveCount()==0 && g_locks==0);
    puts("Actual service state: checked Rust layout, native allocation/defaults, 64-bit fields, exception cleanup, independent COM owners and balanced shell pins passed");
}
