// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual C++ PipeClient -> Rust deadline transport -> packaged Rust worker.
#include "../../../native/tip/Pipe.h"
#include "../../../native/tip/Json.h"
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <filesystem>
#include <fstream>
#include <cstdio>
using namespace meltype;
int wmain() {
    wchar_t executable[32768]={};assert(GetModuleFileNameW(nullptr,executable,ARRAYSIZE(executable)));
    auto profile=std::filesystem::path(executable).parent_path()/L"transport-profile";
    std::filesystem::create_directories(profile);
    {std::ofstream config(profile/L"config.json");config<<"{\"DetectionLevel\":\"Manual\"}";}
    assert(SetEnvironmentVariableW(L"MELTYPE_RUST_DATA_DIR",profile.c_str()));
    PipeClient first,second;std::string response;
    assert(first.Transact("{\"op\":\"hello\"}",response,5000));
    JsonValue value;assert(ParseJson(response,value) && value[L"active"].Bool());
    assert(first.Transact("{\"op\":\"key\",\"sid\":\"host\",\"code\":true,\"vk\":75,\"ch\":107}",response,5000));
    assert(ParseJson(response,value) && value[L"consumed"].Bool());
    assert(first.Transact("{\"op\":\"key\",\"sid\":\"host\",\"code\":true,\"vk\":65,\"ch\":97}",response,5000));
    assert(ParseJson(response,value) && value[L"view"][L"text"].Str()==L"か");
    assert(second.Transact("{\"op\":\"key\",\"sid\":\"host\",\"composing\":true,\"ch\":97}",response,5000));
    assert(ParseJson(response,value) && !value[L"active"].Bool());
    response="stale";
    assert(!first.Transact("{\"op\":\"hello\"}",response,0,true) && response.empty());
    assert(first.Transact("{\"op\":\"reset\",\"sid\":\"host\"}",response,5000,true));
    first.Close();second.Close();
    struct CoreState {TextService* service;unsigned long long epoch=0,last=1000;unsigned int flags=0,published=0;int hellos=0,notices=0;RustCoreStateCallbacks* callbacks=nullptr;} state{new TextService()};
    RustCoreStateCallbacks callbacks={
        &state,static_cast<ITfTextInputProcessorEx*>(state.service),&state.epoch,
        [](void* p,unsigned long long* last)->unsigned int {auto* s=static_cast<CoreState*>(p);*last=s->last;return s->flags;},
        [](void* p,unsigned int op,unsigned long long value) {auto* s=static_cast<CoreState*>(p);switch (op) {
            case 0:s->last=value;break;
            case 1:s->published=static_cast<unsigned int>(value);s->flags=(value&1)?2:0;break;
            case 2:assert(value==1 && s->published==7);++s->notices;s->service->Release();s->service=nullptr;assert(g_locks==1);break;
        }},
        [](void* p) {auto* s=static_cast<CoreState*>(p);++s->hellos;RustCoreHello(L"試験😀",*s->callbacks);},
        [](void*,const char* json,unsigned int* flags)->int {JsonValue value;assert(ParseJson(json,value));assert(value[L"op"].Str()==L"hello" && value[L"process"].Str()==L"試験😀");*flags=7;return 1;},
        [](void*)->unsigned long long {return 5000;},
    };
    state.callbacks=&callbacks;
    assert(RustCoreReady(callbacks) && state.hellos==1 && state.notices==1 && state.last==5000);
    assert(!state.service && state.flags==2 && g_locks==0);
    puts("Actual core adapters: packaged worker, conversion/isolation, timeout/optional policy, Unicode hello, clock/state ABI and owner release passed");
}
