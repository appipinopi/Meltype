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
void* compositionMethods[7]={};
void* managerMethods[7]={};
struct Document { std::wstring text; LONG selectionStart = 0, selectionEnd = 0; int ranges = 0, selections = 0, writes = 0; ITfContext* context=nullptr; };
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
    static HRESULT STDMETHODCALLTYPE CompareStart(Range* s,TfEditCookie ec,ITfRange* other,TfAnchor anchor,LONG* out) {
        assert(ec==7);auto* r=reinterpret_cast<Range*>(other);LONG right=anchor==TF_ANCHOR_START?r->start:r->end;
        *out=(s->start>right)-(s->start<right);return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE CompareEnd(Range* s,TfEditCookie ec,ITfRange* other,TfAnchor anchor,LONG* out) {
        assert(ec==7);auto* r=reinterpret_cast<Range*>(other);LONG right=anchor==TF_ANCHOR_START?r->start:r->end;
        *out=(s->end>right)-(s->end<right);return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE GetContext(Range* s,ITfContext** out) {*out=s->doc->context;(*out)->AddRef();return S_OK;}
};
struct Composition {
    void** table=compositionMethods;ULONG refs=0;Range* range=nullptr;ITfCompositionSink* sink=nullptr;int ended=0;
    static ULONG STDMETHODCALLTYPE Add(Composition* s) {return ++s->refs;}
    static ULONG STDMETHODCALLTYPE Release(Composition* s) {
        ULONG count=--s->refs;if (!count) {Range::Release(s->range);s->range=nullptr;s->sink->Release();s->sink=nullptr;}return count;
    }
    static HRESULT STDMETHODCALLTYPE GetRange(Composition* s,ITfRange** out) {assert(s->refs);Range::Add(s->range);*out=reinterpret_cast<ITfRange*>(s->range);return S_OK;}
    static HRESULT STDMETHODCALLTYPE End(Composition* s,TfEditCookie ec) {assert(ec==7 && s->refs);++s->ended;return S_OK;}
};
struct Manager {
    void** table=managerMethods;ULONG refs=1;Composition* composition;
    static ULONG STDMETHODCALLTYPE Add(Manager* s) {return ++s->refs;}
    static ULONG STDMETHODCALLTYPE Release(Manager* s) {return --s->refs;}
    static HRESULT STDMETHODCALLTYPE Start(Manager* s,TfEditCookie ec,ITfRange* range,ITfCompositionSink* sink,ITfComposition** out) {
        assert(ec==7 && !s->composition->refs);ITfRange* cloned=nullptr;
        Range::Clone(reinterpret_cast<Range*>(range),&cloned);auto* c=s->composition;
        c->range=reinterpret_cast<Range*>(cloned);c->sink=sink;sink->AddRef();c->refs=1;
        *out=reinterpret_cast<ITfComposition*>(c);return S_OK;
    }
};
struct Context {
    void** table = contextMethods;
    Range* selected;
    LONG refs = 1;
    Manager* manager=nullptr;
    static ULONG STDMETHODCALLTYPE Add(Context* self) { return ++self->refs; }
    static ULONG STDMETHODCALLTYPE Release(Context* self) { return --self->refs; }
    static HRESULT STDMETHODCALLTYPE Selection(Context* self,TfEditCookie cookie,ULONG,ULONG count,TF_SELECTION* out,ULONG* fetched) {
        assert(cookie == 7 && count == 1); auto* range = self->selected;
        range->start = range->doc->selectionStart; range->end = range->doc->selectionEnd; ++range->doc->selections;
        Range::Add(range); out->range = reinterpret_cast<ITfRange*>(range); *fetched = 1; return S_OK;
    }
    static HRESULT STDMETHODCALLTYPE Query(Context* s,REFIID iid,void** out) {
        *out=nullptr;if (iid!=IID_ITfContextComposition || !s->manager) return E_NOINTERFACE;
        Manager::Add(s->manager);*out=s->manager;return S_OK;
    }
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
    rangeMethods[18]=reinterpret_cast<void*>(Range::CompareStart);rangeMethods[19]=reinterpret_cast<void*>(Range::CompareEnd);rangeMethods[24]=reinterpret_cast<void*>(Range::GetContext);
    contextMethods[0]=reinterpret_cast<void*>(Context::Query);contextMethods[1]=reinterpret_cast<void*>(Context::Add);contextMethods[2]=reinterpret_cast<void*>(Context::Release);
    contextMethods[3]=reinterpret_cast<void*>(Context::Edit);contextMethods[5]=reinterpret_cast<void*>(Context::Selection);contextMethods[6]=reinterpret_cast<void*>(Context::SetSelection);
    contextMethods[12]=reinterpret_cast<void*>(Context::Property);contextMethods[13]=reinterpret_cast<void*>(Context::Property);
    compositionMethods[1]=reinterpret_cast<void*>(Composition::Add);compositionMethods[2]=reinterpret_cast<void*>(Composition::Release);
    compositionMethods[3]=reinterpret_cast<void*>(Composition::GetRange);compositionMethods[6]=reinterpret_cast<void*>(Composition::End);
    managerMethods[1]=reinterpret_cast<void*>(Manager::Add);managerMethods[2]=reinterpret_cast<void*>(Manager::Release);managerMethods[3]=reinterpret_cast<void*>(Manager::Start);
    Document doc={L"前😀",3,3};Range selected(&doc,3,3,false);Context context={contextMethods,&selected};
    Composition composition;Manager manager={managerMethods,1,&composition};context.manager=&manager;
    doc.context=reinterpret_cast<ITfContext*>(&context);
    auto* service=new TextService();BOOL eaten=FALSE;
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(reinterpret_cast<ITfContext*>(&context),VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(doc.text==L"前😀　" && doc.writes==1 && doc.selectionStart==4 && doc.selectionEnd==4);
    assert(service->OnTestKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && eaten);
    assert(service->OnTestKeyUp(nullptr,VK_SPACE,0,&eaten)==S_OK && !eaten);
    BYTE plain[256]={};assert(SetKeyboardState(plain));
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),'A',0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(reinterpret_cast<ITfContext*>(&context),'A',0,&eaten)==S_OK && eaten);
    assert(doc.text==L"前😀　あ" && composition.refs==1 && context.refs==2);
    const int endedBeforeShortcut=composition.ended;
    BYTE control[256]={};control[VK_CONTROL]=control[VK_LCONTROL]=0x80;assert(SetKeyboardState(control));
    // A non-composition shortcut commits before the original key passes through.
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),'C',0,&eaten)==S_OK && !eaten);
    assert(service->OnKeyDown(reinterpret_cast<ITfContext*>(&context),'C',0,&eaten)==S_OK && !eaten);
    assert(doc.text==L"前😀　あ" && composition.refs==0 && composition.ended==endedBeforeShortcut+1 && manager.refs==1);
    assert(doc.selectionStart==5 && doc.selectionEnd==5);
    assert(SetKeyboardState(plain));
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),'A',0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(reinterpret_cast<ITfContext*>(&context),'A',0,&eaten)==S_OK && eaten);
    assert(doc.text==L"前😀　ああ" && composition.refs==1);
    doc.selectionStart=doc.selectionEnd=0;
    const int endedBeforePacket=composition.ended;
    // Unicode packets pass through after committing, preserving the moved caret.
    assert(service->OnTestKeyDown(reinterpret_cast<ITfContext*>(&context),VK_PACKET,0,&eaten)==S_OK && !eaten);
    assert(doc.text==L"前😀　ああ" && composition.refs==0 && composition.ended==endedBeforePacket+1);
    assert(doc.selectionStart==0 && doc.selectionEnd==0);
    const auto beforeTermination=doc.text;
    doc.selectionStart=doc.selectionEnd=static_cast<LONG>(doc.text.size());
    assert(service->OnTestKeyDown(doc.context,'A',0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(doc.context,'A',0,&eaten)==S_OK && eaten);
    assert(doc.text==beforeTermination+L"あ" && composition.refs==1);
    // The application's own composition reference survives its notification.
    Composition::Add(&composition);
    assert(service->OnCompositionTerminated(7,reinterpret_cast<ITfComposition*>(&composition))==S_OK);
    assert(composition.refs==1 && doc.text==beforeTermination+L"あ");
    Composition::Release(&composition);
    assert(service->OnTestKeyDown(doc.context,'A',0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(doc.context,'A',0,&eaten)==S_OK && eaten);
    assert(doc.text==beforeTermination+L"ああ" && composition.refs==1);
    assert(SetKeyboardState(control));
    assert(service->OnTestKeyDown(doc.context,'C',0,&eaten)==S_OK && !eaten);
    assert(doc.ranges==1 && selected.refs==1 && context.refs==1);
    service->Release();assert(g_locks==0);assert(SetKeyboardState(saved));
    puts("Actual key edit/commit/termination: public TSF entry, packaged core, UTF-16 caret, kana preedit, shortcut/packet commit, moved caret and host-terminated preedit continuation passed");
}
