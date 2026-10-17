// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Actual OnKeyDown -> Rust key edit -> isolated core -> TSF range write.
#include "../../../native/tip/TextService.h"
#include <algorithm>
#include <cassert>
#include <cstdio>
#include <filesystem>
#include <fstream>
using namespace meltype;
namespace {
void* rangeMethods[25] = {};
void* contextMethods[17] = {};
struct Document { std::wstring text; LONG selectionStart = 0, selectionEnd = 0; int ranges = 0, selections = 0, writes = 0; };
struct Range {
    void** table = rangeMethods;
    Document* doc;
    LONG start, end, refs = 1;
    bool owned;
    Range(Document* value,LONG first,LONG last,bool heap) : doc(value),start(first),end(last),owned(heap) { ++doc->ranges; }
    ~Range() { --doc->ranges; }
    static ULONG STDMETHODCALLTYPE Add(Range* self) { return ++self->refs; }
    static ULONG STDMETHODCALLTYPE Release(Range* self) { LONG n = --self->refs; if (!n && self->owned) delete self; return n; }
    static HRESULT STDMETHODCALLTYPE Clone(Range* self,ITfRange** out) {
        *out = reinterpret_cast<ITfRange*>(new Range(self->doc,self->start,self->end,true)); return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Collapse(Range* self,TfEditCookie cookie,TfAnchor anchor) {
        assert(cookie == 7); if (anchor == TF_ANCHOR_START) self->end = self->start; else self->start = self->end; return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Empty(Range* self,TfEditCookie cookie,BOOL* out) { assert(cookie == 7); *out = self->start == self->end; return S_OK; }
    static HRESULT STDMETHODCALLTYPE Shift(Range* self,TfEditCookie cookie,LONG amount,LONG* moved,const TF_HALTCOND*) {
        assert(cookie == 7); LONG next = std::clamp(self->start + amount,0L,self->end); *moved = next-self->start; self->start = next; return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE ShiftEnd(Range* self,TfEditCookie cookie,LONG amount,LONG* moved,const TF_HALTCOND*) {
        assert(cookie==7);LONG next=std::clamp(self->end+amount,self->start,static_cast<LONG>(self->doc->text.size()));*moved=next-self->end;self->end=next;return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Read(Range* self,TfEditCookie cookie,DWORD,wchar_t* out,ULONG maximum,ULONG* count) {
        assert(cookie == 7); *count = (std::min)(maximum,static_cast<ULONG>(self->end-self->start));
        std::copy_n(self->doc->text.data()+self->start,*count,out); return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Write(Range* self,TfEditCookie cookie,DWORD,const wchar_t* value,LONG count) {
        assert(cookie == 7); self->doc->text.replace(self->start,self->end-self->start,value,count); self->end = self->start+count; ++self->doc->writes; return S_OK;
    }
};
struct Context {
    void** table = contextMethods;
    Range* selected;
    LONG refs = 1;
    static ULONG STDMETHODCALLTYPE Add(Context* self) { return ++self->refs; }
    static ULONG STDMETHODCALLTYPE Release(Context* self) { return --self->refs; }
    static HRESULT STDMETHODCALLTYPE Selection(Context* self,TfEditCookie cookie,ULONG,ULONG count,TF_SELECTION* out,ULONG* fetched) {
        assert(cookie == 7 && count == 1); auto* range = self->selected;
        range->start = range->doc->selectionStart; range->end = range->doc->selectionEnd; ++range->doc->selections;
        Range::Add(range); out->range = reinterpret_cast<ITfRange*>(range); *fetched = 1; return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Query(Context*,REFIID,void** out) { *out=nullptr;return E_NOINTERFACE; }
    static HRESULT STDMETHODCALLTYPE Edit(Context*,TfClientId client,ITfEditSession* session,DWORD flags,HRESULT* result) {
        assert(client==TF_CLIENTID_NULL && (flags==3 || flags==7));*result=session->DoEditSession(7);return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE SetSelection(Context* self,TfEditCookie cookie,ULONG count,const TF_SELECTION* value) {
        assert(cookie==7 && count==1);auto* range=reinterpret_cast<Range*>(value[0].range);
        self->selected->doc->selectionStart=range->start;self->selected->doc->selectionEnd=range->end;return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Property(Context*,REFGUID,ITfProperty** out) { *out = nullptr; return E_NOINTERFACE; }
};
}
int wmain() {
    MSG message={};PeekMessageW(&message,nullptr,0,0,PM_NOREMOVE);
    wchar_t executable[32768]={};assert(GetModuleFileNameW(nullptr,executable,ARRAYSIZE(executable)));
    auto profile=std::filesystem::path(executable).parent_path()/L"key-edit-profile";
    std::filesystem::create_directories(profile);
    {std::ofstream config(profile/L"config.json");config<<"{\"DetectionLevel\":\"Manual\"}";}
    assert(SetEnvironmentVariableW(L"MELTYPE_RUST_DATA_DIR",profile.c_str()));
    BYTE saved[256]={};assert(GetKeyboardState(saved));BYTE shifted[256]={};shifted[VK_SHIFT]=shifted[VK_LSHIFT]=0x80;assert(SetKeyboardState(shifted));
    rangeMethods[1]=reinterpret_cast<void*>(Range::Add);rangeMethods[2]=reinterpret_cast<void*>(Range::Release);
    rangeMethods[3]=reinterpret_cast<void*>(Range::Read);rangeMethods[4]=reinterpret_cast<void*>(Range::Write);
    rangeMethods[8]=reinterpret_cast<void*>(Range::Shift);rangeMethods[9]=reinterpret_cast<void*>(Range::ShiftEnd);
    rangeMethods[14]=reinterpret_cast<void*>(Range::Empty);rangeMethods[15]=reinterpret_cast<void*>(Range::Collapse);rangeMethods[23]=reinterpret_cast<void*>(Range::Clone);
    contextMethods[0]=reinterpret_cast<void*>(Context::Query);contextMethods[1]=reinterpret_cast<void*>(Context::Add);contextMethods[2]=reinterpret_cast<void*>(Context::Release);
    contextMethods[3]=reinterpret_cast<void*>(Context::Edit);contextMethods[5]=reinterpret_cast<void*>(Context::Selection);contextMethods[6]=reinterpret_cast<void*>(Context::SetSelection);
    contextMethods[12]=reinterpret_cast<void*>(Context::Property);contextMethods[13]=reinterpret_cast<void*>(Context::Property);
    Document doc={L"前😀",3,3};Range selected(&doc,3,3,false);Context context={contextMethods,&selected};
    auto* service=new TextService();BOOL eaten=FALSE;
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(reinterpret_cast<ITfContext*>(&context),VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(doc.text==L"前😀　" && doc.writes==1 && doc.selectionStart==4 && doc.selectionEnd==4);
    assert(service->OnTestKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnTestKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && !eaten);
    assert(doc.ranges==1 && selected.refs==1 && context.refs==1);
    service->Release();assert(g_locks==0);assert(SetKeyboardState(saved));
    puts("Actual key edit: public TSF key entry, privacy cache, Rust request/edit policy, packaged Rust core, UTF-16 commit/caret and key release passed");
}
