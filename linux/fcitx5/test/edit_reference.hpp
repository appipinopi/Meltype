// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
// Controlled editor for comparing the former apply() order with real Rust ABI.
struct Editor {
    using Delete=int (*)(const uint8_t *,uint32_t,uint32_t,const uint8_t *,uint32_t,int);
    std::string text;
    size_t cursor;
    bool capable;
    Delete deletion;
    bool reference=false;
    int fail=-1;
    ViewTrace trace;
    int event(uint32_t op,int arg,const std::string &value) {
        trace.emplace_back(op,arg,value);
        if(static_cast<int>(op)==fail)return -1;
        if(op==9) {
            if(!capable)return 0;
            if(!reference)return deletion(reinterpret_cast<const uint8_t *>(text.data()),text.size(),cursor,reinterpret_cast<const uint8_t *>(value.data()),value.size(),arg);
            const auto start=cursor>charLength(value)?cursor-charLength(value):0;
            const auto before=text.substr(byteOffset(text,start),byteOffset(text,cursor)-byteOffset(text,start));
            return !value.empty()&&utf16Length(value)==static_cast<size_t>(arg)&&!text.empty()
                &&before.size()>=value.size()&&before.compare(before.size()-value.size(),value.size(),value)==0?static_cast<int>(charLength(value)):0;
        }
        if(op==10) {
            const auto at=byteOffset(text,cursor),from=byteOffset(text,cursor-static_cast<size_t>(arg));
            text.erase(from,at-from);cursor-=arg;
        }
        if(op==11) {text.insert(byteOffset(text,cursor),value);cursor+=charLength(value);}
        return 1;
    }
};
static int editEvent(void *context,uint32_t op,int arg,const uint8_t *text,uint32_t length) noexcept {
    try {return static_cast<Editor *>(context)->event(op,arg,std::string(reinterpret_cast<const char *>(text),length));}
    catch(...) {return -1;}
}
static void referenceApply(Editor &editor,const Json &result) {
    if(auto *commits=result.get("commits");commits&&commits->kind==Json::Kind::Array) {
        for(const auto &edit:commits->items) {
            const int remove=edit.integer("deleteBefore");
            if(remove>0) {
                const auto expected=edit.str("expect");
                const int count=editor.event(9,remove,expected);
                if(!count) {editor.event(8,0,{});return;}
                editor.event(8,0,{});editor.event(10,count,{});
            } else editor.event(12,0,{});
            const auto committed=edit.str("text");
            if(!committed.empty())editor.event(11,0,committed);
        }
    }
    for(const auto &[op,arg,text]:referenceView(result))editor.event(op,arg,text);
}
