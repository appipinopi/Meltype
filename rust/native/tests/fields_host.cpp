// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual shell adapters against SDK interfaces, without registering an IME.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
struct Compartment final : ITfCompartment {
    ULONG refs = 1;
    LONG value = 0;
    bool refuse = false;
    std::function<void()> changed;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_ITfCompartment) return E_NOINTERFACE;
        *out=static_cast<ITfCompartment*>(this);AddRef();return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs; }
    STDMETHODIMP_(ULONG) Release() override { return --refs; }
    STDMETHODIMP SetValue(TfClientId client,const VARIANT* data) override {
        assert(client==17 && data->vt==VT_I4);
        if (refuse) return E_ACCESSDENIED;
        value=data->lVal;
        if (changed) changed();
        return S_OK;
    }
    STDMETHODIMP GetValue(VARIANT* data) override {
        VariantInit(data);data->vt=VT_I4;data->lVal=value;return S_OK;
    }
};
struct EventSink final : ITfThreadMgrEventSink {
    ULONG refs=1;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_ITfThreadMgrEventSink) return E_NOINTERFACE;
        *out=static_cast<ITfThreadMgrEventSink*>(this);AddRef();return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override {return ++refs;}
    STDMETHODIMP_(ULONG) Release() override {return --refs;}
    STDMETHODIMP OnInitDocumentMgr(ITfDocumentMgr*) override {return S_OK;}
    STDMETHODIMP OnUninitDocumentMgr(ITfDocumentMgr*) override {return S_OK;}
    STDMETHODIMP OnSetFocus(ITfDocumentMgr*,ITfDocumentMgr*) override {return S_OK;}
    STDMETHODIMP OnPushContext(ITfContext*) override {return S_OK;}
    STDMETHODIMP OnPopContext(ITfContext*) override {return S_OK;}
};
struct Source final : ITfSource {
    ULONG refs=1;
    IUnknown* sink=nullptr;
    int removals=0;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
        if (iid!=IID_IUnknown && iid!=IID_ITfSource) return E_NOINTERFACE;
        *out=static_cast<ITfSource*>(this);AddRef();return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override {return ++refs;}
    STDMETHODIMP_(ULONG) Release() override {return --refs;}
    STDMETHODIMP AdviseSink(REFIID iid,IUnknown* owner,DWORD* cookie) override {
        assert(iid==IID_ITfThreadMgrEventSink && !sink);
        HRESULT hr=owner->QueryInterface(iid,reinterpret_cast<void**>(&sink));
        if (SUCCEEDED(hr)) *cookie=33;
        return hr;
    }
    STDMETHODIMP UnadviseSink(DWORD cookie) override {
        assert(cookie==33 && sink);
        IUnknown* old=sink;sink=nullptr;old->Release();++removals;return S_OK;
    }
};
struct Manager final : ITfCompartmentMgr {
    ULONG refs = 1;
    Compartment open,disabled,empty;
    Source source;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
        if (iid==IID_ITfSource) {source.AddRef();*out=static_cast<ITfSource*>(&source);return S_OK;}
        if (iid!=IID_IUnknown && iid!=IID_ITfCompartmentMgr) return E_NOINTERFACE;
        *out=static_cast<ITfCompartmentMgr*>(this);AddRef();return S_OK;
    }
    STDMETHODIMP_(ULONG) AddRef() override { return ++refs; }
    STDMETHODIMP_(ULONG) Release() override { return --refs; }
    STDMETHODIMP GetCompartment(REFGUID guid,ITfCompartment** out) override {
        *out=nullptr;
        if (guid==GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) *out=&open;
        else if (guid==GUID_COMPARTMENT_KEYBOARD_DISABLED) *out=&disabled;
        else if (guid==GUID_COMPARTMENT_EMPTYCONTEXT) *out=&empty;
        else return E_INVALIDARG;
        (*out)->AddRef();return S_OK;
    }
    STDMETHODIMP ClearCompartment(TfClientId,REFGUID) override { return E_NOTIMPL; }
    STDMETHODIMP EnumCompartments(IEnumGUID**) override { return E_NOTIMPL; }
};
}
int wmain() {
    Manager manager;
    // These adapters only query IUnknown for ITfCompartmentMgr. The controlled
    // object intentionally exposes no real thread-manager/context operations.
    auto* unknown=static_cast<ITfCompartmentMgr*>(&manager);
    auto* thread=reinterpret_cast<ITfThreadMgr*>(unknown);
    auto* context=reinterpret_cast<ITfContext*>(unknown);
    assert(RustInputScope(7,nullptr)==0);
    assert(RustReadOpen(thread)==0);
    int observed=-1;
    manager.open.changed=[&] {observed=RustReadOpen(thread);};
    RustWriteOpen(thread,17,true);
    assert(observed==1 && manager.open.value==1);
    RustWriteOpen(thread,17,false);
    assert(observed==0 && manager.open.value==0);
    manager.open.refuse=true;
    RustWriteOpen(thread,17,true);
    assert(manager.open.value==0);
    assert(!RustContextDisabled(context));
    manager.disabled.value=1;
    assert(RustContextDisabled(context));
    manager.disabled.value=0;manager.empty.value=-1;
    assert(RustContextDisabled(context));
    assert(RustContextDisabled(nullptr));
    EventSink sink;
    void* subscription=RustSubscribe(0,unknown,17,&sink);
    assert(subscription && sink.refs==2 && manager.source.refs==2);
    RustUnsubscribe(subscription);
    RustUnsubscribe(subscription);
    assert(sink.refs==1 && manager.source.refs==1 && manager.source.removals==1 && !manager.source.sink);
    assert(manager.refs==1 && manager.open.refs==1 && manager.disabled.refs==1 && manager.empty.refs==1);
    auto* service=new TextService();
    assert(service->OnChange(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)==S_OK);
    {
        RustServiceStateStorage storage;
        storage.value->manager=thread;storage.value->composition=reinterpret_cast<ITfComposition*>(1);
        storage.value->direct_word=true;
        struct ModeState {TextService* service;RustServiceState* native;int commits=0,logs=0;} state{service,storage.value};
        auto commit=[](void* p) {
            auto* s=static_cast<ModeState*>(p);
            assert(!s->native->open && !s->native->direct_word && s->logs==1);++s->commits;
            s->service->Release();s->service=nullptr;assert(g_locks==2);
        };
        auto notice=[](void* p,int open) {
            auto* s=static_cast<ModeState*>(p);assert(!open && !s->native->open && !s->commits);++s->logs;
        };
        auto* lifetime=service->ServiceInterface<ITfTextInputProcessorEx>();
        assert(RustServiceOpened(state.native,lifetime,GUID_NULL,&state,commit,notice)==S_OK && state.logs==0);
        assert(RustServiceOpened(state.native,lifetime,GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&state,commit,notice)==S_OK);
        assert(!state.native->open && state.commits==1 && state.logs==1 && !state.service);
        assert(manager.refs==1 && manager.open.refs==1 && g_locks==1);
        state.native->manager=nullptr;state.native->composition=nullptr;
    }
    assert(g_locks==0 && RustServiceStateLiveCount()==0);
    puts("Actual TSF field adapters: scopes/modes, subscriptions, open-notice publication/commit/refresh order, public null-manager notice and owner release passed");
}
