// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Links the actual Rust preview shell objects to exercise its RunEditSession
// adapter and captured C++ lambda lifetimes without registering/activating TSF.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
#include <memory>
using namespace meltype;
namespace {
struct Context;
struct ContextVtable {
    void* query; void* addref; void* release;
    HRESULT (STDMETHODCALLTYPE* request)(Context*,TfClientId,ITfEditSession*,DWORD,HRESULT*);
};
struct Context {
    const ContextVtable* vtable;
    bool deferred = false;
    bool reject = false;
    ITfEditSession* pending = nullptr;
    DWORD flags = 0;
    static HRESULT STDMETHODCALLTYPE Request(Context* self,TfClientId client,ITfEditSession* session,DWORD flags,HRESULT* result) {
        assert(client == 17);
        self->flags = flags;
        ITfEditSession* identity = nullptr;
        assert(session->QueryInterface(IID_ITfEditSession,reinterpret_cast<void**>(&identity)) == S_OK);
        assert(identity == session); identity->Release();
        // The actual COM implementation must come from the Rust DLL.
        HMODULE module = nullptr;
        const void* method = (*reinterpret_cast<void***>(session))[3];
        assert(GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,reinterpret_cast<const wchar_t*>(method),&module));
        wchar_t path[32768]; assert(GetModuleFileNameW(module,path,ARRAYSIZE(path)));
        const wchar_t* name = wcsrchr(path,L'\\');
        assert(name && _wcsicmp(name+1,L"meltype_native.dll") == 0);
        if (self->reject) { *result = E_FAIL; return TF_E_SYNCHRONOUS; }
        if (self->deferred) {
            session->AddRef(); self->pending = session; *result = TF_S_ASYNC;
        } else { *result = session->DoEditSession(1234); }
        return S_OK;
    }
};
const ContextVtable kVtable = {nullptr,nullptr,nullptr,Context::Request};
struct Guard { int* drops; LONG* pinAtDrop; ~Guard() { *pinAtDrop = g_locks; ++*drops; } };
}
int wmain() {
    Context context = {&kVtable};
    auto* host = reinterpret_cast<ITfContext*>(&context);
    constexpr DWORD flags = TF_ES_SYNC | TF_ES_READWRITE;
    int drops = 0, calls = 0;
    LONG pinAtDrop = -1;
    auto execute = [&]() {
        auto capture = std::shared_ptr<Guard>(new Guard{&drops,&pinAtDrop});
        return RunEditSession(host,17,flags,[capture,&calls](TfEditCookie cookie) {
            assert(cookie == 1234); ++calls; return E_ACCESSDENIED;
        });
    };
    assert(execute() == E_ACCESSDENIED);
    assert(calls == 1 && drops == 1 && g_locks == 0 && context.flags == flags);
    assert(RunEditSession(host,17,flags,[](TfEditCookie) -> HRESULT { throw 1; }) == E_FAIL);
    assert(g_locks == 0);
    context.reject = true;
    assert(execute() == TF_E_SYNCHRONOUS);
    assert(calls == 1 && drops == 2 && g_locks == 0);
    context.reject = false; context.deferred = true;
    assert(execute() == TF_S_ASYNC);
    assert(calls == 1 && drops == 2 && g_locks == 1);
    assert(context.pending->DoEditSession(1234) == E_ACCESSDENIED);
    assert(calls == 2 && drops == 2 && g_locks == 1);
    context.pending->Release(); context.pending = nullptr;
    assert(drops == 3 && g_locks == 0 && pinAtDrop == 1);
    // Cancellation must destroy captures and release the shell pin, without
    // invoking a document-edit lambda that is no longer wanted.
    assert(execute() == TF_S_ASYNC);
    context.pending->Release(); context.pending = nullptr;
    assert(calls == 2 && drops == 4 && g_locks == 0 && pinAtDrop == 1);
    void* abandoned = RustAbandonedCreate();
    assert(abandoned != nullptr);
    RustAbandonedReset(abandoned);
    assert(!RustAbandonedForget(abandoned,nullptr));
    RustAbandonedDestroy(abandoned);
    puts("Actual TSF shell edit adapter: Rust COM dispatch, sync/refused/async/cancel paths, captured lambda lifetime, DLL pins and old-input API loading passed");
    return 0;
}
