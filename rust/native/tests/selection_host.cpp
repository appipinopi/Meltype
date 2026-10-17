// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Real RustPipe adapter, queued COM session and real TextService owner pin.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
void* methods[4]={};
struct Context {
    void** table=methods;ULONG refs=1;ITfEditSession* pending=nullptr;
    static ULONG STDMETHODCALLTYPE Add(Context* s) {return ++s->refs;}
    static ULONG STDMETHODCALLTYPE Release(Context* s) {return --s->refs;}
    static HRESULT STDMETHODCALLTYPE Edit(Context* s,TfClientId client,ITfEditSession* session,DWORD flags,HRESULT* result) {
        assert(client==17 && flags==(TF_ES_ASYNCDONTCARE|TF_ES_READWRITE) && !s->pending);
        session->AddRef();s->pending=session;*result=TF_S_ASYNC;return S_OK;
    }
    void Run() {auto* session=pending;pending=nullptr;assert(session->DoEditSession(7)==S_OK);session->Release();}
};
struct State {TextService* service;Context* context;bool own=false;int requests=0;};
}
int wmain() {
    methods[1]=reinterpret_cast<void*>(Context::Add);methods[2]=reinterpret_cast<void*>(Context::Release);methods[3]=reinterpret_cast<void*>(Context::Edit);
    Context context;State state{new TextService(),&context};
    RustCommitCallbacks callbacks={
        &state,static_cast<ITfTextInputProcessorEx*>(state.service),
        [](void* p,void** context,void** sink)->void* {auto* s=static_cast<State*>(p);*context=s->context;*sink=nullptr;return nullptr;},
        [](void* p,unsigned int op,unsigned int cookie,int value) {assert(op==0 && cookie==7 && value==1);static_cast<State*>(p)->own=true;},
        [](void* p,const char* text,unsigned int cookie,void* context,void* target) {
            auto* s=static_cast<State*>(p);JsonValue request;
            assert(s->own && cookie==7 && context==s->context && !target && ParseJson(text,request));
            assert(request[L"op"].Str()==L"select" && request[L"sid"].Str()==L"選択\"😀");
            assert(request[L"index"].Int()==-1 && request[L"prediction"].Bool());
            assert(g_locks==1 && !s->service);++s->requests;
        },
    };
    const std::string metadata="{\"sid\":"+JsonString(L"選択\"😀")+"}";
    assert(RustSelectCandidate(17,-1,true,metadata,callbacks)==TF_S_ASYNC);
    assert(context.pending && context.refs==2 && state.requests==0 && !state.own);
    state.service->Release();state.service=nullptr;assert(g_locks==1);
    context.Run();assert(state.requests==1 && context.refs==1 && g_locks==0);
    puts("Actual selection adapter: prediction/index/Unicode ABI, async scheduling, own-edit order, context retention and real shell-owner release passed");
}
