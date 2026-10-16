// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Production TSF key entry points and callback ABI, no real input injection.
#include "../../../native/tip/TextService.h"
#include <cassert>
#include <cstdio>
using namespace meltype;
namespace {
void* methods[17] = {};
struct Context {
    void** table=methods;
    ULONG refs=1;
    static HRESULT STDMETHODCALLTYPE Query(Context*,REFIID,void** out) {*out=nullptr;return E_NOINTERFACE;}
    static ULONG STDMETHODCALLTYPE Add(Context* self) {return ++self->refs;}
    static ULONG STDMETHODCALLTYPE Release(Context* self) {return --self->refs;}
};
struct Keyboard {
    BYTE saved[256] = {};
    Keyboard() {assert(GetKeyboardState(saved));BYTE clear[256]={};assert(SetKeyboardState(clear));}
    ~Keyboard() {SetKeyboardState(saved);}
};
struct State {TextService* service;RustKeySnapshot snapshot={};bool checked=false;int queries=0,passes=0;};
}
int wmain() {
    MSG message={};PeekMessageW(&message,nullptr,0,0,PM_NOREMOVE);
    Keyboard keyboard;
    methods[0]=reinterpret_cast<void*>(Context::Query);
    methods[1]=reinterpret_cast<void*>(Context::Add);
    methods[2]=reinterpret_cast<void*>(Context::Release);
    Context document;
    auto* context=reinterpret_cast<ITfContext*>(&document);
    auto* service=new TextService();
    BOOL eaten=TRUE;
    assert(service->OnTestKeyDown(context,VK_SHIFT,0,&eaten)==S_OK && !eaten);
    assert(service->OnTestKeyDown(context,VK_KANJI,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyDown(context,VK_KANJI,0,&eaten)==S_OK && eaten);
    assert(service->OnTestKeyUp(context,VK_KANJI,0,&eaten)==S_OK && eaten);
    assert(service->OnKeyUp(context,VK_KANJI,0,&eaten)==S_OK && eaten);
    assert(service->OnTestKeyUp(context,VK_KANJI,0,&eaten)==S_OK && !eaten);
    assert(service->OnKeyDown(context,VK_KANJI,0,&eaten)==S_OK && eaten);
    assert(service->OnSetFocus(FALSE)==S_OK);
    assert(service->OnKeyUp(context,VK_KANJI,0,&eaten)==S_OK && !eaten);
    assert(service->OnTestKeyUp(context,static_cast<WPARAM>(-1),0,&eaten)==S_OK && !eaten);
    assert(RustKeyFlags(VK_SHIFT,0,false)&1);
    assert(RustKeyFlags(VK_KANJI,0,false)&2);
    BYTE shifted[256]={};shifted[VK_SHIFT]=shifted[VK_LSHIFT]=0x80;
    assert(SetKeyboardState(shifted));
    assert(RustKeyDown(VK_SHIFT) && RustKeyChar(VK_SPACE,0)==L' ');
    void* keyState=RustKeyStateCreate();
    assert(keyState);
    RustKeyStale stale={};
    RustKeyStaleWrite(keyState,3,65,0x123456789abcdef0ULL);
    RustKeyStaleRead(keyState,stale);
    assert(stale.tries==3 && stale.key==65 && stale.time==0x123456789abcdef0ULL);
    RustKeyStaleClear(keyState);
    RustKeyStaleRead(keyState,stale);
    assert(stale.tries==0 && stale.key==0 && stale.time==0x123456789abcdef0ULL);
    int reads=0,privateField=0;
    auto reader=[](void* p,void*,int* field)->int {++*static_cast<int*>(p);*field=1;return -1;};
    assert(RustKeySecret(keyState,context,65,true,&reads,static_cast<ITfTextInputProcessorEx*>(service),reader,privateField)==-1);
    assert(privateField==1 && reads==1 && document.refs==2);
    assert(RustKeySecret(keyState,context,65,false,&reads,static_cast<ITfTextInputProcessorEx*>(service),reader,privateField)==-1);
    assert(privateField==1 && reads==1 && document.refs==1);
    DWORD error=0;
    auto sender=[](void*,const INPUT* inputs,unsigned int count)->unsigned int {
        assert(count==2);
        assert(inputs[0].type==INPUT_KEYBOARD && inputs[0].ki.wVk==65 && inputs[0].ki.dwFlags==0);
        assert(inputs[1].ki.dwFlags==KEYEVENTF_KEYUP && inputs[0].ki.dwExtraInfo==0x4d4c5450);
        return 2;  // Controlled transport: no key is injected.
    };
    assert(RustReinject(keyState,65,static_cast<ITfTextInputProcessorEx*>(service),error,nullptr,sender)==1 && error==0);
    LPARAM extra=SetMessageExtraInfo(0x4d4c5450);
    assert(RustIsReinjected(keyState,65) && !RustIsReinjected(keyState,65));
    SetMessageExtraInfo(extra);
    auto failure=[](void*,const INPUT*,unsigned int)->unsigned int {SetLastError(ERROR_ACCESS_DENIED);return 0;};
    assert(RustReinject(keyState,66,static_cast<ITfTextInputProcessorEx*>(service),error,nullptr,failure)==0 && error==ERROR_ACCESS_DENIED);
    RustKeyStateReset(keyState);RustKeyStateDestroy(keyState);
    State state{service};state.snapshot.open=1;
    RustKeyCallbacks callbacks={
        &state,static_cast<ITfTextInputProcessorEx*>(service),
        [](void* p,RustKeySnapshot* out) {*out=static_cast<State*>(p)->snapshot;},
        [](void* p,unsigned int op,unsigned long long a,unsigned long long) {
            auto* s=static_cast<State*>(p);
            if (op==0) s->checked=false;
            else if (op==4) s->checked=a!=0;
        },
        [](void*,void*) {},
        [](void*,int)->int {assert(false);return 0;},
        [](void*,int) {assert(false);},
        [](void*) {assert(false);},
        [](void* p)->int {
            auto* s=static_cast<State*>(p);++s->queries;
            s->service->Release();s->service=nullptr;
            assert(g_locks==1);return 1;
        },
        [](void* p,unsigned int key,void*,int test)->int {assert(!static_cast<State*>(p)->service && key==VK_SPACE && test && g_locks==1);return 0;},
        [](void* p,unsigned int reason) {assert(reason==0 && g_locks==1);++static_cast<State*>(p)->passes;},
    };
    wchar_t ch=0;
    assert(RustWouldEat(context,VK_SPACE,0,true,callbacks,ch));
    assert(ch==L' ' && state.checked && state.queries==1 && state.passes==1);
    assert(document.refs==1 && g_locks==0);
    puts("Actual TSF key adapters: production test/down/up, keyboard/callback ABI, cached private metadata, controlled replay/marker, Shift+Space and owner release passed");
}
