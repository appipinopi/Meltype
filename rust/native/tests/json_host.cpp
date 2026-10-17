// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
// Production C++ value adapter compared with the immutable upstream parser.
#include "../../../native/tip/Json.h"
#include "../../../native/tip/Globals.h"
#include <cassert>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <cstdlib>
#define ParseJson UpstreamParseJson
#define JsonString UpstreamJsonString
#define ToUtf8 UpstreamToUtf8
#define FromUtf8 UpstreamFromUtf8
#include "../../../dist/json-reference/original.inc"
#undef ParseJson
#undef JsonString
#undef ToUtf8
#undef FromUtf8
using namespace meltype;
namespace {
bool Equal(const JsonValue& a,const JsonValue& b) {
    if (a.type!=b.type) return false;
    switch (a.type) {
        case JsonValue::Type::Null:return true;
        case JsonValue::Type::Bool:return a.boolean==b.boolean;
        case JsonValue::Type::Number:return a.number==b.number && (a.number!=0 || std::signbit(a.number)==std::signbit(b.number));
        case JsonValue::Type::String:return a.string==b.string;
        case JsonValue::Type::Array:
            if (a.array.size()!=b.array.size()) return false;
            for (size_t i=0;i<a.array.size();++i) if (!Equal(a.array[i],b.array[i])) return false;
            return true;
        case JsonValue::Type::Object:
            if (a.object.size()!=b.object.size()) return false;
            for (const auto& pair:a.object) {auto found=b.object.find(pair.first);if (found==b.object.end() || !Equal(pair.second,found->second)) return false;}
            return true;
    }
    return false;
}
unsigned int comparisons=0;
void Check(const std::string& text) {
    JsonValue oldValue,newValue;
    bool oldOk=UpstreamParseJson(text,oldValue),newOk=ParseJson(text,newValue);
    if (oldOk!=newOk || (oldOk && !Equal(oldValue,newValue))) {
        fprintf(stderr,"JSON parser differs at comparison %u (%zu bytes)\n",comparisons,text.size());abort();
    }
    ++comparisons;
}
}
int wmain() {
    for (const char* text:{"null","true","false","0","-0","-1.25e+10","1e9999","+",".","1e+","01","e","[1,true,null,{},[]]","{\"a\":1,\"a\":2}","{\"commits\":[{\"text\":\"日本😀\",\"replace\":2}],\"view\":{\"typed\":\"a\",\"clauses\":[]}}","\"\\ud800\\u0000\\udc00\\ud83d\\ude00\"","","[1,]","{\"a\":}","{}x","\"\\x\"","\"\\u000z\"","\"unfinished"}) Check(text);
    for (int depth=0;depth<36;++depth) {
        Check(std::string(depth,'[')+"0"+std::string(depth,']'));
        Check(std::string(depth,'[')+std::string(depth,']'));
    }
    Check(std::string("1\0e5",4));Check(std::string("\0",1));
    for (unsigned int unit=0;unit<65536;++unit) {
        std::wstring text(1,static_cast<wchar_t>(unit));
        auto oldQuote=UpstreamJsonString(text),newQuote=JsonString(text);
        if (oldQuote!=newQuote) {fprintf(stderr,"JSON quote differs at UTF16 unit %04x\n",unit);abort();}
        Check(oldQuote);
    }
    for (unsigned int i=0;i<1024;++i) {
        std::wstring text={L'前',0,static_cast<wchar_t>(i),static_cast<wchar_t>(0xd800+(i%1024)),static_cast<wchar_t>(0xdc00+(i%1024)),L'"',L'\\',L'\n'};
        auto quoted=UpstreamJsonString(text);assert(quoted==JsonString(text));
        Check("{\"x\":"+quoted+",\"y\":["+std::to_string(i)+",{\"a\":false}],\"x\":"+quoted+"}");
    }
    for (unsigned int byte=0;byte<256;++byte) Check(std::string("\"a")+static_cast<char>(byte)+"b\"");
    for (const std::string& bytes:{std::string("\xed\xa0\x80"),std::string("\xf0\x80\x80\xaf"),std::string("\xe2\x82"),std::string("\xc0\xaf")}) Check("\""+bytes+"\"");
    JsonValue unchanged;unchanged.type=JsonValue::Type::String;unchanged.string=L"keep";
    assert(!ParseJson("[1,]",unchanged) && unchanged.Str()==L"keep");
    assert(ParseJson("{\"active\":false}",unchanged) && unchanged.type==JsonValue::Type::Object && unchanged.string.empty());
    int callbacks=0;
    auto cancelled=[](void* p,unsigned int,double,const unsigned short*,unsigned int)->int {++*static_cast<int*>(p);return 0;};
    assert(!RustJsonVisit("{\"a\":[1,2]}",&callbacks,cancelled) && callbacks==1);
    callbacks=0;assert(!RustJsonVisit("[1,]",&callbacks,cancelled) && callbacks==0);
    assert(g_locks==0);
    printf("Actual Rust JSON bridge: %u upstream parse comparisons, 65536 BMP quotes, UTF16 surrogates/NUL, depth/numbers, atomic replacement and callback cancellation passed\n",comparisons);
}
