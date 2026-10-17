// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual production OnEndEdit and shell callback ABI, using no real document.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
// Legacy callback ABI reference helper; production uses native follow-ups.
bool RustServiceEdited(RustServiceState* state,IUnknown* life,ITfContext* context,ITfEditRecord* record,TfEditCookie cookie,void* adapter,void (__cdecl *query)(void*,unsigned int,void*,int),void (__cdecl *commit)(void*)) {
    auto module=LoadLibraryW(L"meltype_native.dll");assert(module);
    auto call=reinterpret_cast<int (__cdecl *)(void*,void*,void*,void*,unsigned int,void*,void (__cdecl *)(void*,unsigned int,void*,int),void (__cdecl *)(void*))>(GetProcAddress(module,"meltype_tsf_service_edited"));assert(call);
    auto result=call(state,life,context,record,cookie,adapter,query,commit);FreeLibrary(module);return result!=0;
}
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
struct State {TextService* service;ITfContext* context;RustServiceState* native;int queries=0;};
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
    {
        RustServiceStateStorage storage;
        State state{service,context,storage.value};
        state.native->own_edit=true;
        auto query=[](void* p,unsigned int cookie,void* context,int changed) {
            auto* s=static_cast<State*>(p);
            assert(cookie==7 && context==s->context && changed==1);++s->queries;
            assert(!s->native->caret_moved);
            s->service->Release();s->service=nullptr;
            assert(g_locks==2); // Both the Rust service pin and fixture storage.
        };
        auto commit=[](void*) {assert(false && "No composition may be committed");};
        auto* lifetime=service->ServiceInterface<ITfTextInputProcessorEx>();
        assert(RustServiceEdited(state.native,lifetime,context,&record,7,&state,query,commit));
        assert(!state.native->own_edit && state.queries==0 && !state.native->caret_moved && record.reads==2);
        assert(RustServiceEdited(state.native,lifetime,context,&record,7,&state,query,commit));
        assert(!state.service && state.queries==1 && state.native->caret_moved && record.reads==3);
        assert(document.refs==1 && record.refs==1 && g_locks==1);
    }
    assert(g_locks==0 && RustServiceStateLiveCount()==0);
    puts("Actual OnEndEdit/shell adapter: null/own/changed events, callback layout, mode/moved order, owner release during callback and COM/DLL cleanup passed");
}
