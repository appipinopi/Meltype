// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Production RustPipe mode adapter ABI and real shell owner lifetime.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
void* methods[17]={};
struct Context {
    void** table=methods;
    ULONG refs=1;
    bool refuse=false;
    static HRESULT STDMETHODCALLTYPE Query(Context*,REFIID,void** out) {*out=nullptr;return E_NOINTERFACE;}
    static ULONG STDMETHODCALLTYPE Add(Context* s) {return ++s->refs;}
    static ULONG STDMETHODCALLTYPE Release(Context* s) {return --s->refs;}
    static HRESULT STDMETHODCALLTYPE Edit(Context* s,TfClientId client,ITfEditSession* session,DWORD flags,HRESULT* result) {
        assert(client==17 && flags==(TF_ES_SYNC|TF_ES_READ));
        if (s->refuse) {*result=S_OK;return E_ACCESSDENIED;}
        *result=session->DoEditSession(7);return S_OK;
    }
};
struct State {
    TextService* service;
    Context* context;
    unsigned int flags=3;
    int queries=0,opens=0,notices=0,requests=0;
};
}
int wmain() {
    methods[0]=reinterpret_cast<void*>(Context::Query);
    methods[1]=reinterpret_cast<void*>(Context::Add);
    methods[2]=reinterpret_cast<void*>(Context::Release);
    methods[3]=reinterpret_cast<void*>(Context::Edit);
    Context document;
    State state{new TextService(),&document};
    void* token=RustKeyStateCreate();assert(token);
    RustModeCallbacks callbacks={
        &state,static_cast<ITfTextInputProcessorEx*>(state.service),
        [](void* p)->unsigned int {return static_cast<State*>(p)->flags;},
        [](void*)->int {return 1;},
        [](void* p,unsigned int cookie,void* context)->int {
            auto* s=static_cast<State*>(p);
            assert(cookie==7 && context==s->context && s->context->refs>=3);
            ++s->queries;return 1; // Password: no range/text operations are exposed.
        },
        [](void* p,int open) {auto* s=static_cast<State*>(p);s->flags=(s->flags&~1u)|(open?1u:0u);++s->opens;},
        [](void* p,const char* text)->int {
            auto* s=static_cast<State*>(p);JsonValue request;
            assert(ParseJson(text,request));
            assert(request[L"op"].Str()==L"toggle" && request[L"japanese"].Bool());
            assert(request[L"sid"].Str()==L"モード\"😀" && request[L"process"].Str()==L"app.exe");
            assert(request[L"before"].IsNull()); // Refused synchronous read: tracked-line fallback.
            ++s->requests;
            s->service->Release();s->service=nullptr;
            assert(g_locks==1);return 1;
        },
        [](void* p,int japanese) {auto* s=static_cast<State*>(p);assert(japanese && !s->service && g_locks==1);++s->notices;},
    };
    auto* context=reinterpret_cast<ITfContext*>(&document);
    std::string metadata="{\"sid\":"+JsonString(L"モード\"😀")+",\"process\":\"app.exe\"}";
    assert(!RustCodeLine(context,token,17,metadata,callbacks,true));
    assert(state.queries==1 && state.requests==0 && document.refs==1);
    assert(RustToggleMode(context,nullptr,token,17,metadata,callbacks));
    assert(state.queries==2 && state.opens==1 && !(state.flags&1));
    document.refuse=true;
    assert(RustCodeLine(context,token,17,metadata,callbacks,true));
    assert(state.requests==1 && state.notices==1 && document.refs==1 && g_locks==0);
    RustKeyStateDestroy(token);
    puts("Actual mode adapters: callback ABI, privacy-before-text, code-line fallback, Unicode metadata and owner release during request passed");
}
