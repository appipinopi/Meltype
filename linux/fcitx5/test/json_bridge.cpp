// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Standalone ABI test: production event adapter against the legacy parser and
// real Rust engine replies. No fcitx daemon, user profile or input is touched.
#include "../rust_json.hpp"
#include "../compat/json_reader.hpp"
#include <dlfcn.h>
#include <fstream>
#include <iostream>
#include <stdexcept>
using namespace meltype_fcitx;
static void check(bool condition, const std::string &message) { if (!condition) throw std::runtime_error(message); }
static bool same(const Json &a, const Json &b) {
    if (a.kind != b.kind) return false;
    switch (a.kind) {
    case Json::Kind::Null: return true;
    case Json::Kind::Bool: return a.boolean == b.boolean;
    case Json::Kind::Number: return a.number == b.number;
    case Json::Kind::String: return a.string == b.string;
    case Json::Kind::Array:
        if (a.items.size() != b.items.size()) return false;
        for (size_t i=0; i<a.items.size(); i++) if (!same(a.items[i],b.items[i])) return false;
        return true;
    case Json::Kind::Object:
        if (a.members.size() != b.members.size()) return false;
        for (const auto &member:a.members) { const auto *other=b.get(member.first); if (!other || !same(member.second,*other)) return false; }
        return true;
    }
    return false;
}
int main(int argc,char **argv) {
    try {
        check(argc==3,"library and private profile required");
        void *library=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);check(library!=nullptr,"Rust library load failed");
        auto visit=reinterpret_cast<JsonVisit>(dlsym(library,"meltype_json_visit_utf8"));check(visit!=nullptr,"UTF-8 visitor missing");
        size_t count=0;
        auto compare=[&](const std::string &text) {
            Json oldValue,newValue;
            check(JsonReader(text).read(oldValue),"legacy fixture parse failed: "+text);
            check(readRustJson(text,newValue,visit),"Rust fixture parse failed: "+text);
            check(same(oldValue,newValue),"legacy/Rust value mismatch: "+text);count++;
        };
        for (const std::string text:{"null","true","false","123.5","-2.25e3","[]","{}",
            R"({"consumed":true,"commits":[{"text":"日本😀","deleteBefore":3,"expect":"前😀"}],"view":{"text":"かな","clauses":["仮名","です"],"candidates":["仮名","かな"],"converting":true,"selectedIndex":1,"selectedClause":0,"hint":"案内","suggestion":"候補"}})",
            R"({"escaped":"\u304b\u306a\ud83d\ude00\u0000\n\t\"\\","items":[null,false,0,-1,{"a":[]}]})"}) compare(text);
        for (int i=0;i<200;i++) compare("{\"selectedIndex\":"+std::to_string(i)+",\"text\":\"候補😀\",\"candidates\":[\"一\",\"二\"]}");
        for (const std::string invalid:{"", "{", "true trailing", "{\"x\":1,}", "[1,]", "{\"x\":\"\\uXXXX\"}", "1e9999"}) {
            Json out;out.kind=Json::Kind::String;out.string="unchanged";
            check(!readRustJson(invalid,out,visit),"invalid JSON accepted");check(out.string=="unchanged","failed parse changed projection");
        }
        Json unicode;check(!readRustJson(R"("\ud800")",unicode,visit),"unpaired escape rejected");
        Json duplicate;check(readRustJson(R"({"x":1,"x":2})",duplicate,visit)&&duplicate.integer("x")==2,"Rust duplicate-key policy");
        auto stopped=[](void *,uint32_t,double,const uint8_t *,uint32_t)->int{return 0;};
        const std::string text="[1]";check(visit(reinterpret_cast<const uint8_t *>(text.data()),text.size(),nullptr,stopped)==0,"callback cancellation");
        auto create=reinterpret_cast<void *(*)(const char *)>(dlsym(library,"meltype_create_at"));
        auto key=reinterpret_cast<char *(*)(void *,int,int,int,const char *,const char *)>(dlsym(library,"meltype_handle_key"));
        auto commit=reinterpret_cast<char *(*)(void *)>(dlsym(library,"meltype_commit"));
        auto destroy=reinterpret_cast<void (*)(void *)>(dlsym(library,"meltype_destroy"));
        auto freeResult=reinterpret_cast<void (*)(void *)>(dlsym(library,"meltype_free"));
        check(create&&key&&commit&&destroy&&freeResult,"core ABI missing");
        void *session=create(argv[2]);check(session!=nullptr,"private session");
        for (char scalar:std::string("nihongo")) {char *reply=key(session,0,scalar,0,nullptr,nullptr);check(reply!=nullptr,"engine key");compare(reply);freeResult(reply);}
        char *reply=commit(session);check(reply!=nullptr,"engine commit");compare(reply);freeResult(reply);destroy(session);
        check(dlclose(library)==0,"library close");
        std::cout<<"fcitx Rust JSON: "<<count<<" reference/core projections, UTF-8/NUL/escapes, malformed input, cancellation and duplicate policy passed\n";
    } catch (const std::exception &error) {std::cerr<<error.what()<<'\n';return 1;}
}
