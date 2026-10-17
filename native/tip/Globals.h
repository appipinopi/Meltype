// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai
//
// Meltype IME (TSF のテキストサービス) の共通の定義。

#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <msctf.h>
#include <ctffunc.h>
#include <olectl.h>
#include <string>
#include <vector>

namespace meltype {

// テキストサービスの CLSID (COM のクラス)
// {417D801B-A9BD-4C26-BD16-356A825A6998}
inline constexpr CLSID CLSID_TextService = {0x417d801b, 0xa9bd, 0x4c26, {0xbd, 0x16, 0x35, 0x6a, 0x82, 0x5a, 0x69, 0x98}};

// 日本語のキーボードとしてのプロファイル
// {21F643F4-72D5-4946-BF70-0A126AB52D05}
inline constexpr GUID GUID_Profile = {0x21f643f4, 0x72d5, 0x4946, {0xbf, 0x70, 0x0a, 0x12, 0x6a, 0xb5, 0x2d, 0x05}};

// 下線の種類: 打っている途中 / 変換した文節 / 選んでいる文節
// {C8107D36-2806-4D8F-A183-FD0AEB8ED862}
inline constexpr GUID GUID_AttrInput = {0xc8107d36, 0x2806, 0x4d8f, {0xa1, 0x83, 0xfd, 0x0a, 0xeb, 0x8e, 0xd8, 0x62}};
// {E17AA399-C5EB-42E6-BE45-C44F0651F2E2}
inline constexpr GUID GUID_AttrConverted = {0xe17aa399, 0xc5eb, 0x42e6, {0xbe, 0x45, 0xc4, 0x4f, 0x06, 0x51, 0xf2, 0xe2}};
// {CEF94813-C47F-48C1-B4EF-D4582D5D4054}
inline constexpr GUID GUID_AttrTarget = {0xcef94813, 0xc47f, 0x48c1, {0xb4, 0xef, 0xd4, 0x58, 0x2d, 0x5d, 0x40, 0x54}};

inline constexpr LANGID kLangId = MAKELANGID(LANG_JAPANESE, SUBLANG_JAPANESE_JAPAN);
inline constexpr wchar_t kDisplayName[] = L"Meltype";

extern HINSTANCE g_module;
extern LONG g_locks;

inline void DllAddRef() { InterlockedIncrement(&g_locks); }
inline void DllRelease() { InterlockedDecrement(&g_locks); }
#ifdef MELTYPE_RUST_CORE
HRESULT RustGetClassObject(REFCLSID clsid, REFIID iid, void** out);
HRESULT RustRegisterServer();
HRESULT RustUnregisterServer();
void RustSurroundingText(TfEditCookie, ITfContext*, std::wstring&, std::wstring&);
bool RustWriteRange(TfEditCookie, ITfContext*, ITfRange*, const std::wstring&, bool);
struct JsonValue;
ITfComposition* RustStartComposition(TfEditCookie, ITfContext*, ITfCompositionSink*, ITfRange*);
bool RustUpdateComposition(TfEditCookie, ITfContext*, ITfComposition*, const JsonValue&, TfGuidAtom, TfGuidAtom, TfGuidAtom);
void RustEndComposition(TfEditCookie, ITfContext*, ITfComposition*);
void RustSetAttribute(TfEditCookie, ITfContext*, ITfRange*, TfGuidAtom);
bool RustApplyCommits(TfEditCookie,ITfContext*,const JsonValue&,void*,
    void* (__cdecl *)(void*),int (__cdecl *)(void*,void*,const unsigned short*,unsigned int));
void* RustCaptureResume(TfEditCookie,ITfContext*,ITfComposition*);
bool RustResumeMatches(void*,ITfContext*);
ITfComposition* RustTryResume(void*,TfEditCookie,ITfContext*,ITfCompositionSink*);
void RustResumeAttribute(TfEditCookie,ITfContext*,ITfComposition*,TfGuidAtom);
void RustDestroyResume(void*);
void* RustAbandonedCreate();
void RustAbandonedReset(void*);
void RustAbandonedDestroy(void*);
void RustAbandonedAdd(void*,ITfContext*,ITfComposition*,IUnknown*,TfClientId);
bool RustAbandonedForget(void*,ITfComposition*);
HRESULT RustAbandonedEnd(void*,IUnknown*,TfClientId);
void RustClearCompositionAttribute(TfEditCookie,ITfComposition*);
unsigned int RustInputScope(TfEditCookie,ITfContext*);
int RustReadOpen(ITfThreadMgr*);
void RustWriteOpen(ITfThreadMgr*,TfClientId,bool);
bool RustContextDisabled(ITfContext*);
void* RustSubscribe(unsigned int,IUnknown*,TfClientId,IUnknown*,ITfContext** = nullptr);
void RustUnsubscribe(void*);
struct RustEditCallbacks {
    void* owner;
    IUnknown* lifetime;
    int (__cdecl *takeOwn)(void*);
    void* (__cdecl *current)(void*,void**);
    void (__cdecl *query)(void*,unsigned int,void*,int);
    void (__cdecl *moved)(void*,int);
    void (__cdecl *commit)(void*);
};
void RustEditNotification(TfEditCookie,ITfContext*,ITfEditRecord*,const RustEditCallbacks&);
unsigned int RustKeyFlags(UINT,LPARAM,bool);
bool RustKeyDown(int);
wchar_t RustKeyChar(UINT,LPARAM);
struct RustKeySnapshot {
    void* composition; void* context;
    int disabled,open,detection,english;
    unsigned int staleTries,staleVk;
    unsigned long long staleTime;
};
struct RustKeyCallbacks {
    void* owner; IUnknown* lifetime;
    void (__cdecl *read)(void*,RustKeySnapshot*);
    void (__cdecl *update)(void*,unsigned int,unsigned long long,unsigned long long);
    void (__cdecl *resume)(void*,void*);
    int (__cdecl *commitNow)(void*,int);
    void (__cdecl *commitAsync)(void*,int);
    void (__cdecl *abandon)(void*);
    int (__cdecl *ready)(void*);
    int (__cdecl *secret)(void*,unsigned int,void*,int);
    void (__cdecl *pass)(void*,unsigned int);
};
bool RustWouldEat(ITfContext*,UINT,LPARAM,bool,const RustKeyCallbacks&,wchar_t&);
void* RustKeyStateCreate();
void RustKeyStateReset(void*);
void RustKeyStateDestroy(void*);
void RustKeyCacheClear(void*);
bool RustIsReinjected(void*,UINT);
int RustKeySecret(void*,ITfContext*,UINT,bool,void*,IUnknown*,int (__cdecl *)(void*,void*,int*),int&);
int RustReinject(void*,UINT,IUnknown*,DWORD&,void* = nullptr,unsigned int (__cdecl *)(void*,const INPUT*,unsigned int) = nullptr);
struct RustKeyStale {unsigned int tries,key;unsigned long long time;};
struct RustKeyPassed {int key,control;};
struct RustKeyEntry {
    void* owner;IUnknown* lifetime;
    int (__cdecl *would)(void*,void*,unsigned int,unsigned long long,int,wchar_t*);
    void (__cdecl *mark)(void*,unsigned int,int);
    int (__cdecl *composing)(void*);
    void (__cdecl *toggle)(void*,void*);
    void (__cdecl *handle)(void*,void*,unsigned int,wchar_t);
};
void RustKeyForeground(void*,bool);
void RustKeyStaleRead(void*,RustKeyStale&);
void RustKeyStaleWrite(void*,unsigned int,unsigned int,unsigned long long);
void RustKeyStaleClear(void*);
bool RustKeyPassedRead(void*,bool,bool,RustKeyPassed&);
bool RustKeyEvent(void*,unsigned int,ITfContext*,UINT,LPARAM,const RustKeyEntry&);
struct RustModeCallbacks {
    void* owner;IUnknown* lifetime;
    unsigned int (__cdecl *read)(void*);
    int (__cdecl *ready)(void*);
    int (__cdecl *secret)(void*,unsigned int,void*);
    void (__cdecl *setOpen)(void*,int);
    int (__cdecl *request)(void*,const char*);
    void (__cdecl *notice)(void*,int);
};
bool RustToggleMode(ITfContext*,ITfThreadMgr*,void*,TfClientId,const std::string&,const RustModeCallbacks&);
bool RustCodeLine(ITfContext*,void*,TfClientId,const std::string&,const RustModeCallbacks&,bool);
struct RustModeQueryCallbacks {
    void* owner;IUnknown* lifetime;
    unsigned int (__cdecl *read)(void*);
    int (__cdecl *secret)(void*,unsigned int,void*);
    int (__cdecl *request)(void*,const char*,unsigned int,int);
};
struct RustModeApplyCallbacks {
    void* owner;IUnknown* lifetime;
    unsigned int (__cdecl *read)(void*);
    void (__cdecl *write)(void*,unsigned int);
    void (__cdecl *notify)(void*,int);
};
bool RustQueryMode(ITfContext*,TfEditCookie,void*,const std::string&,const RustModeQueryCallbacks&,bool);
bool RustApplyMode(unsigned int,const RustModeApplyCallbacks&);
struct RustKeyEditCallbacks {
    void* owner;IUnknown* lifetime;
    unsigned int (__cdecl *read)(void*);
    void (__cdecl *update)(void*,unsigned int,unsigned int);
    int (__cdecl *edit)(void*,unsigned int,unsigned int,void*);
    int (__cdecl *request)(void*,const char*,unsigned int*);
    int (__cdecl *write)(void*,unsigned int,void*,void*,wchar_t);
    void (__cdecl *log)(void*,unsigned int,int);
};
HRESULT RustHandleKey(ITfContext*,TfClientId,void*,UINT,wchar_t,const std::string&,const RustKeyEditCallbacks&);
struct RustCommitCallbacks {
    void* owner;IUnknown* lifetime;
    void* (__cdecl *current)(void*,void**,void**);
    void (__cdecl *update)(void*,unsigned int,unsigned int,int);
    void (__cdecl *requestApply)(void*,const char*,unsigned int,void*,void*);
};
HRESULT RustCommit(unsigned int,ITfContext*,TfClientId,TfEditCookie,const std::string&,const RustCommitCallbacks&,bool);
HRESULT RustSelectCandidate(TfClientId,int,bool,const std::string&,const RustCommitCallbacks&);
struct RustApplyReplyCallbacks {
    void* owner;IUnknown* lifetime;
    void* (__cdecl *current)(void*);
    int (__cdecl *write)(void*,void*,const unsigned short*,unsigned int);
    void (__cdecl *action)(void*,unsigned int,unsigned int,void*,const char*,int);
};
bool RustApplyReply(TfEditCookie,ITfContext*,const JsonValue&,const std::string&,const RustApplyReplyCallbacks&);
struct RustCompositionSlots {IUnknown* lifetime;void** composition;void** context;ITfCompositionSink* sink;};
int RustApplyOwnedComposition(TfEditCookie,ITfContext*,const JsonValue&,TfGuidAtom,TfGuidAtom,TfGuidAtom,const RustCompositionSlots&);
int RustWriteOwnedCommit(TfEditCookie,ITfContext*,ITfRange*,const std::wstring&,bool,const RustCompositionSlots&);
int RustEndOwnedComposition(TfEditCookie,const RustCompositionSlots&);
struct RustTerminationCallbacks {
    void* owner;IUnknown* lifetime;void** composition;void** context;void** resume;void* abandoned;
    unsigned int (__cdecl *read)(void*);
    void (__cdecl *update)(void*,unsigned int);
    void (__cdecl *request)(void*,const char*);
};
HRESULT RustCompositionTerminated(TfEditCookie,ITfComposition*,const std::string&,const RustTerminationCallbacks&);
struct RustRecoveryCallbacks {
    void* owner;IUnknown* lifetime;void** composition;void** context;void** resume;ITfCompositionSink* sink;
    void* (__cdecl *editContext)(void*);
    void (__cdecl *update)(void*,unsigned int,int);
    void (__cdecl *request)(void*,const char*);
};
int RustResumeOrCommit(ITfContext*,TfClientId,TfGuidAtom,const std::string&,const RustRecoveryCallbacks&);
struct RustAbandonCallbacks {
    void* owner;IUnknown* lifetime;void** composition;void** context;void** abandoned;
    void (__cdecl *update)(void*,unsigned int);
    void (__cdecl *request)(void*,const char*);
};
int RustAbandonComposition(TfClientId,const std::string&,const RustAbandonCallbacks&);
struct RustFocusCallbacks {
    void* owner;IUnknown* lifetime;unsigned long long* epoch;
    unsigned int (__cdecl *read)(void*);
    int (__cdecl *commit)(void*,int);
    void (__cdecl *action)(void*,unsigned int,void*);
};
HRESULT RustDocumentFocus(ITfDocumentMgr*,const RustFocusCallbacks&);
HRESULT RustForegroundFocus(bool,void*,const RustFocusCallbacks&);
#endif

// 文字コードの変換
std::string ToUtf8(const std::wstring& text);
std::wstring FromUtf8(const std::string& text);

// デバッグ用のログ (%LOCALAPPDATA%\Meltype\tip.log、MELTYPE_TIP_LOG=1 のときだけ)
void TipLog(const wchar_t* format, ...);

}  // namespace meltype
