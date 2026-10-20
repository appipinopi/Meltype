// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Legacy ABI JSON reference adapter and real shell-owner pin during application.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
#include <limits>
using namespace meltype;
struct RustApplyReplyCallbacks {
    void* owner;IUnknown* lifetime;
    void* (__cdecl *current)(void*);
    int (__cdecl *write)(void*,void*,const unsigned short*,unsigned int);
    void (__cdecl *action)(void*,unsigned int,unsigned int,void*,const char*,int);
};
std::string CommitJson(const JsonValue& reply) {
    std::string json = "[";
    const auto& edits = reply[L"commits"].array;
    for (size_t i = 0; i < edits.size(); ++i) {
        if (i) json += ',';
        const auto& edit = edits[i];
        json += "{\"text\":" + JsonString(edit[L"text"].Str()) + ",\"deleteBefore\":" + std::to_string(edit[L"deleteBefore"].Int()) +
            ",\"caretBack\":" + std::to_string(edit[L"caretBack"].Int()) + ",\"expect\":" + (edit[L"expect"].type == JsonValue::Type::String ? JsonString(edit[L"expect"].Str()) : "null") + "}";
    }
    return json+"]";
}
std::string ViewJson(const JsonValue& view) {
    std::string json = "{";
    for (const wchar_t* name : {L"candidates",L"notes",L"predictions",L"clauses"}) {
        json += JsonString(name) + ":[";
        const auto& values = view[name].array;
        for (size_t i=0;i<values.size();++i) { if (i) json += ','; json += JsonString(values[i].Str()); }
        json += "],";
    }
    for (const wchar_t* name : {L"suggestion",L"meaning",L"typed",L"text"}) json += JsonString(name) + ":" + JsonString(view[name].Str()) + ',';
    for (const wchar_t* name : {L"selectedIndex",L"selectedPrediction",L"selectedClause"}) json += JsonString(name) + ":" + std::to_string(view[name].Int()) + ',';
    json += std::string("\"converting\":") + (view[L"converting"].Bool() ? "true}" : "false}");
    return json;
}
bool RustApplyReply(TfEditCookie cookie,ITfContext* context,const JsonValue& reply,const std::string& metadata,const RustApplyReplyCallbacks& callbacks) {
    auto module=LoadLibraryW(L"meltype_native.dll");assert(module);
    auto apply=reinterpret_cast<int (__cdecl *)(void*,unsigned int,const char*,const char*,const RustApplyReplyCallbacks*)>(GetProcAddress(module,"meltype_tsf_apply_reply"));assert(apply);
    const std::string json=std::string("{\"active\":")+(reply[L"active"].Bool()?"true":"false")+
        ",\"commits\":"+CommitJson(reply)+",\"view\":"+(reply[L"view"].IsNull()?"null":ViewJson(reply[L"view"]))+"}";
    auto result=apply(context,cookie,json.c_str(),metadata.c_str(),&callbacks);FreeLibrary(module);return result!=0;
}
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
    // The native value projection must match this original MSVC conversion on
    // both target ABIs, including fractional boundaries and overflow.
    const std::pair<const char*,int> integers[]={
        {"2147483647.9",(std::numeric_limits<int>::max)()},
        {"2147483648",(std::numeric_limits<int>::min)()},
        {"-2147483648.9",(std::numeric_limits<int>::min)()},
        {"-2147483649",(std::numeric_limits<int>::min)()},
        {"1e500",(std::numeric_limits<int>::min)()},{"-1.9",-1},
    };
    for (const auto& item:integers) {JsonValue number;assert(ParseJson(item.first,number));assert(number.Int()==item.second);}
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
    puts("Legacy SDK reply adapter: Unicode/escaped view and arrays, converting-before-preedit/candidates, borrowed context and real shell-owner release passed");
}
