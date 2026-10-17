// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual C++ PipeClient -> Rust deadline transport -> packaged Rust worker.
#include "../../../native/tip/Pipe.h"
#include "../../../native/tip/Json.h"
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
    puts("Actual PipeClient: packaged worker discovery, Unicode profile, conversion/session isolation, zero timeout, optional retry and response clearing passed");
}
