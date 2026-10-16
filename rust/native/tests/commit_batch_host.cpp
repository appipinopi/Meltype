// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Link the actual shell adapter to check JSON serialization and callback ABI.
#include "../../../native/tip/TextService.h"
#include <algorithm>
#include <cassert>
#include <cstdio>
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
    static HRESULT STDMETHODCALLTYPE Property(Context*,REFGUID,ITfProperty** out) { *out = nullptr; return E_NOINTERFACE; }
};
void* __cdecl Current(void*) { return nullptr; }
int __cdecl Write(void* owner,void* range,const unsigned short* value,unsigned int length) {
    auto* context = static_cast<Context*>(owner);
    bool result = RustWriteRange(7,reinterpret_cast<ITfContext*>(context),static_cast<ITfRange*>(range),
        std::wstring(reinterpret_cast<const wchar_t*>(value),length),true);
    // Simulate the application moving its caret while a deferred edit runs.
    context->selected->doc->selectionStart = context->selected->doc->selectionEnd = 0;
    return result ? 1 : 0;
}
}
int wmain() {
    rangeMethods[1] = reinterpret_cast<void*>(Range::Add); rangeMethods[2] = reinterpret_cast<void*>(Range::Release);
    rangeMethods[3] = reinterpret_cast<void*>(Range::Read); rangeMethods[4] = reinterpret_cast<void*>(Range::Write);
    rangeMethods[8] = reinterpret_cast<void*>(Range::Shift); rangeMethods[14] = reinterpret_cast<void*>(Range::Empty);
    rangeMethods[15] = reinterpret_cast<void*>(Range::Collapse); rangeMethods[23] = reinterpret_cast<void*>(Range::Clone);
    contextMethods[1] = reinterpret_cast<void*>(Context::Add); contextMethods[2] = reinterpret_cast<void*>(Context::Release);
    contextMethods[5] = reinterpret_cast<void*>(Context::Selection); contextMethods[12] = reinterpret_cast<void*>(Context::Property);
    Document doc = {L"old",0,3}; Range selected(&doc,0,3,false); Context context = {contextMethods,&selected};
    JsonValue reply;
    assert(ParseJson(u8"{\"commits\":[{\"text\":\"日\"},{\"text\":\"😀\"},{\"text\":\"本\"}]}",reply));
    assert(RustApplyCommits(7,reinterpret_cast<ITfContext*>(&context),reply,&context,Current,Write));
    assert(doc.text == L"日😀本" && doc.writes == 3 && doc.selections == 1);
    assert(doc.ranges == 1 && selected.refs == 1 && context.refs == 1);
    doc.text = L"old"; doc.selectionStart = doc.selectionEnd = 3; doc.writes = 0;
    assert(ParseJson("{\"commits\":[{\"text\":\"bad\",\"deleteBefore\":3,\"expect\":\"no!\"},{\"text\":\"later\"}]}",reply));
    assert(!RustApplyCommits(7,reinterpret_cast<ITfContext*>(&context),reply,&context,Current,Write));
    assert(doc.text == L"old" && doc.writes == 0 && doc.ranges == 1 && selected.refs == 1 && context.refs == 1);
    assert(g_locks == 0);
    puts("Actual TSF shell commit adapter: JSON/UTF-16 callback ABI, ordered insertion despite caret moves, guarded rejection and COM cleanup passed");
    return 0;
}
