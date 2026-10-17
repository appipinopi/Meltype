// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual production OnEndEdit and shell callback ABI, using no real document.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
void* methods[17] = {};
struct Context {
    void** table = methods;
    ULONG refs=1;
    static ULONG STDMETHODCALLTYPE Add(Context* self) {return ++self->refs;}
    static ULONG STDMETHODCALLTYPE Release(Context* self) {return --self->refs;}
};
struct Record final : ITfEditRecord {
    ULONG refs=1;
    int reads=0;
    BOOL changed=FALSE;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_ITfEditRecord) return E_NOINTERFACE;
        *out=static_cast<ITfEditRecord*>(this);AddRef();return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override {return ++refs;}
    STDMETHODIMP_(ULONG) Release() override {return --refs;}
    STDMETHODIMP GetSelectionStatus(BOOL* out) override {++reads;*out=changed;return S_OK;}
    STDMETHODIMP GetTextAndPropertyUpdates(DWORD,const GUID**,ULONG,IEnumTfRanges**) override {return E_NOTIMPL;}
};
struct State {TextService* service;ITfContext* context;bool own=true;int queries=0,moved=0;};
}
int wmain() {
    methods[1]=reinterpret_cast<void*>(Context::Add);
    methods[2]=reinterpret_cast<void*>(Context::Release);
    Context document;
    auto* context=reinterpret_cast<ITfContext*>(&document);
    Record record;
    auto* service=new TextService();
    assert(g_locks==1);
    assert(service->OnEndEdit(context,7,&record)==S_OK && record.reads==1);
    record.changed=TRUE;
    assert(service->OnEndEdit(context,7,&record)==S_OK && record.reads==2);
    assert(service->OnEndEdit(context,7,nullptr)==S_OK && record.reads==2);
    State state{service,context};
    RustEditCallbacks callbacks = {
        &state,service->ServiceInterface<ITfTextInputProcessorEx>(),
        [](void* p)->int {auto* s=static_cast<State*>(p);bool own=s->own;s->own=false;return own?1:0;},
        [](void*,void** context)->void* {*context=nullptr;return nullptr;},
        [](void* p,unsigned int cookie,void* context,int changed) {
            auto* s=static_cast<State*>(p);
            assert(cookie==7 && context==s->context && changed==1);++s->queries;
            s->service->Release();s->service=nullptr;
            assert(g_locks==1);  // Rust pins the owner until all callbacks finish.
        },
        [](void* p,int) {auto* s=static_cast<State*>(p);assert(!s->service && g_locks==1);++s->moved;},
        [](void*) {assert(false && "No composition may be committed");},
    };
    RustEditNotification(7,context,&record,callbacks);
    assert(!state.own && state.queries==0 && state.moved==0 && record.reads==2);
    RustEditNotification(7,context,&record,callbacks);
    assert(state.queries==1 && state.moved==1 && record.reads==3);
    assert(document.refs==1 && record.refs==1 && g_locks==0);
    puts("Actual OnEndEdit/shell adapter: null/own/changed events, callback layout, mode/moved order, owner release during callback and COM/DLL cleanup passed");
}
