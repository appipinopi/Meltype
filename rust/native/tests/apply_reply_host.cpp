// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Production JSON adapter and real shell-owner pin during response application.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
void* methods[17]={};
struct Context {
    void** table=methods;ULONG refs=1;
    static ULONG STDMETHODCALLTYPE Add(Context* s) {return ++s->refs;}
    static ULONG STDMETHODCALLTYPE Release(Context* s) {return --s->refs;}
};
struct State {TextService* service;Context* context;std::vector<unsigned int> actions;};
}
int wmain() {
    methods[1]=reinterpret_cast<void*>(Context::Add);methods[2]=reinterpret_cast<void*>(Context::Release);
    Context context;State state{new TextService(),&context,{}};
    RustApplyReplyCallbacks callbacks={
        &state,state.service->ServiceInterface<ITfTextInputProcessorEx>(),
        [](void*)->void* {return nullptr;},
        [](void*,void*,const unsigned short*,unsigned int)->int {assert(false);return 0;},
        [](void* p,unsigned int op,unsigned int cookie,void* context,const char* json,int value) {
            auto* s=static_cast<State*>(p);assert(cookie==7 && context==s->context && s->context->refs>=2 && g_locks==1);
            s->actions.push_back(op);
            if (op==0) {assert(!json && value==1);return;}
            assert(op==1 || op==2);JsonValue view;assert(ParseJson(json,view));
            assert(view[L"text"].Str()==L"日本😀" && view[L"converting"].Bool());
            assert(view[L"clauses"].array.size()==2 && view[L"clauses"].array[1].Str()==L"本😀");
            assert(view[L"notes"].array[0].Str()==L"引用\"と\n注釈" && view[L"predictions"].array[0].Str()==L"次の語");
            assert(view[L"selectedPrediction"].Int()==-1 && view[L"selectedClause"].Int()==1);
            if (op==2) {s->service->Release();s->service=nullptr;assert(g_locks==1);}
        },
    };
    JsonValue reply;
    assert(ParseJson(u8"{\"active\":true,\"commits\":[],\"view\":{\"text\":\"日本😀\",\"converting\":true,\"clauses\":[\"日\",\"本😀\"],\"notes\":[\"引用\\\"と\\n注釈\"],\"predictions\":[\"次の語\"],\"selectedPrediction\":-1,\"selectedClause\":1}}",reply));
    assert(RustApplyReply(7,reinterpret_cast<ITfContext*>(&context),reply,"{\"sid\":\"reply\",\"direct\":false}",callbacks));
    assert((state.actions==std::vector<unsigned int>{0,1,2}) && context.refs==1 && !state.service && g_locks==0);
    puts("Actual reply adapter: Unicode/escaped view and arrays, converting-before-preedit/candidates, borrowed context and real shell-owner release passed");
}
