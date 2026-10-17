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
unsigned int RustKeyFlags(UINT,LPARAM,bool);
bool RustKeyDown(int);
wchar_t RustKeyChar(UINT,LPARAM);
void* RustKeyStateCreate();
void RustKeyStateReset(void*);
void RustKeyStateDestroy(void*);
void RustKeyCacheClear(void*);
bool RustIsReinjected(void*,UINT);
int RustReinject(void*,UINT,IUnknown*,DWORD&,void* = nullptr,unsigned int (__cdecl *)(void*,const INPUT*,unsigned int) = nullptr);
struct RustKeyStale {unsigned int tries,key;unsigned long long time;};
struct RustKeyPassed {int key,control;};
void RustKeyForeground(void*,bool);
void RustKeyStaleRead(void*,RustKeyStale&);
void RustKeyStaleWrite(void*,unsigned int,unsigned int,unsigned long long);
void RustKeyStaleClear(void*);
bool RustKeyPassedRead(void*,bool,bool,RustKeyPassed&);
using RustJsonEmit=int (__cdecl *)(void*,unsigned int,double,const unsigned short*,unsigned int);
bool RustJsonVisit(const std::string&,void*,RustJsonEmit);
std::string RustJsonQuote(const std::wstring&);
struct RustCoreStateCallbacks {
    void* owner;IUnknown* lifetime;const unsigned long long* epoch;
    unsigned int (__cdecl *read)(void*,unsigned long long*);
    void (__cdecl *update)(void*,unsigned int,unsigned long long);
    void (__cdecl *hello)(void*);
    int (__cdecl *request)(void*,const char*,unsigned int*);
    unsigned long long (__cdecl *clock)(void*);
};
bool RustCoreReady(const RustCoreStateCallbacks&);
std::wstring RustProcessName();
void RustCoreHello(const std::wstring&,const RustCoreStateCallbacks&);
struct RustServiceCall {
    unsigned int op;void* a;void* b;void* c;size_t n1;intptr_t n2;
};
void* RustCreateServiceCom(void*,HRESULT (__cdecl *)(void*,const RustServiceCall*),void (__cdecl *)(void*));
void* RustServiceComInterface(void*,REFIID);
struct RustServiceState;
RustServiceState* RustCreateServiceState();
void RustDestroyServiceState(RustServiceState*);
unsigned int RustServiceStateLiveCount();
void RustServiceSetSid(RustServiceState*,const char*);
void RustServiceInitialize(RustServiceState*,unsigned int,int,int,const char*);
void RustServiceClearResume(RustServiceState*);
bool RustServiceExchange(RustServiceState*,IUnknown*,const std::string&,DWORD,bool,void*,int (__cdecl *)(void*,const unsigned char*,unsigned int),void (__cdecl *)(void*));
HRESULT RustServiceFocusNative(RustServiceState*,IUnknown*,ITfCompositionSink*,ITfTextEditSink*,ITfDocumentMgr*,int);
bool RustServiceEditedNative(RustServiceState*,IUnknown*,ITfCompositionSink*,ITfContext*,ITfEditRecord*,TfEditCookie);
HRESULT RustServiceOpenedNative(RustServiceState*,IUnknown*,ITfCompositionSink*,REFGUID);
HRESULT RustServiceShutdown(RustServiceState*,IUnknown*,ITfCompositionSink*);
HRESULT RustServiceActivate(RustServiceState*,IUnknown*,ITfThreadMgr*,TfClientId,DWORD,void* const*,void*,void (__cdecl *)(void*,int,int),int (__cdecl *)(void*),void (__cdecl *)(void*));
int RustServiceApplyComposition(RustServiceState*,IUnknown*,ITfCompositionSink*,TfEditCookie,ITfContext*,const JsonValue&);
int RustServiceWriteCommit(RustServiceState*,IUnknown*,ITfCompositionSink*,TfEditCookie,ITfContext*,ITfRange*,const std::wstring&);
int RustServiceEndComposition(RustServiceState*,IUnknown*,TfEditCookie);
HRESULT RustServiceTerminated(RustServiceState*,IUnknown*,TfEditCookie,ITfComposition*);
int RustServiceResume(RustServiceState*,IUnknown*,ITfCompositionSink*,ITfContext*);
int RustServiceAbandon(RustServiceState*,IUnknown*);
bool RustServiceInputScope(RustServiceState*,IUnknown*,TfEditCookie,ITfContext*);
using RustServiceApply=int (__cdecl *)(void*,unsigned int,void*,const unsigned char*,unsigned int);
int RustServiceApplyReplyBytes(RustServiceState*,IUnknown*,ITfCompositionSink*,TfEditCookie,ITfContext*,const unsigned char*,unsigned int);
bool RustServiceDispatchKey(RustServiceState*,IUnknown*,ITfCompositionSink*,unsigned int,ITfContext*,UINT,LPARAM);
HRESULT RustServiceCommit(RustServiceState*,IUnknown*,unsigned int,ITfContext*,TfEditCookie,bool,void*,RustServiceApply);
HRESULT RustServiceSelectCandidate(RustServiceState*,IUnknown*,int,bool,void*,RustServiceApply);
bool RustServiceModeAction(RustServiceState*,IUnknown*,ITfContext*,TfEditCookie,unsigned int,int);
bool RustServiceCore(RustServiceState*,IUnknown*,bool);
unsigned int RustServiceMode(const RustServiceState*);
void RustServiceWriteOpen(RustServiceState*,bool);
void RustServiceApplyMode(RustServiceState*,unsigned int,IUnknown*,void*,void (__cdecl *)(void*,int));
HRESULT RustServiceEditSink(RustServiceState*,ITfDocumentMgr*,IUnknown*,ITfTextEditSink*);
std::string RustServiceSid(const RustServiceState*);
void RustServiceClose(RustServiceState*);
ITfLangBarItem* RustServiceCreateBar(RustServiceState*,void*,int (__cdecl *)(void*),void (__cdecl *)(void*));
void RustServiceUpdateBar(RustServiceState*);
void RustServiceRemoveBar(RustServiceState*,IUnknown*);
void* RustServiceCreateCandidates(RustServiceState*,void*,void (__cdecl *)(void*,int,int));
struct JsonValue;
void RustServiceShowCandidates(RustServiceState*,TfEditCookie,ITfContext*,ITfComposition*,const JsonValue&);
void RustServiceHideCandidates(RustServiceState*);
void RustServiceDropCandidates(RustServiceState*);
#endif

// 文字コードの変換
std::string ToUtf8(const std::wstring& text);
std::wstring FromUtf8(const std::string& text);

// デバッグ用のログ (%LOCALAPPDATA%\Meltype\tip.log、MELTYPE_TIP_LOG=1 のときだけ)
void TipLog(const wchar_t* format, ...);

}  // namespace meltype
