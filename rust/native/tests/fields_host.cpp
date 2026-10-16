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
struct Manager final : ITfCompartmentMgr {
    ULONG refs = 1;
    Compartment open,disabled,empty;
    STDMETHODIMP QueryInterface(REFIID iid,void** out) override {
        *out=nullptr;
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
    assert(manager.refs==1 && manager.open.refs==1 && manager.disabled.refs==1 && manager.empty.refs==1);
    assert(g_locks==0);
    puts("Actual TSF field adapters: input-scope API loading, mode reads/writes, client id, reentrant notification, refused write, disabled/empty context and COM cleanup passed");
}
