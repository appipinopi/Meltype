// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Minimal production DLL bridge. Legacy TextService/JSON adapters are test-only.
#include "Globals.h"
namespace meltype {
namespace {
using Notice=void (__cdecl *)(void*,unsigned int,int);
using Lock=void (__cdecl *)(int);
struct Api {
    HMODULE module=nullptr;
    std::string worker;
    HRESULT (__cdecl *object)(const char*,const GUID*,void**,void*,Notice,Notice,Notice,Notice,Lock)=nullptr;
    HRESULT (__cdecl *factory)(const GUID*,const GUID*,void**,HRESULT (__cdecl *)(const GUID*,void**),Lock)=nullptr;
    HRESULT (__cdecl *reg)(const wchar_t*,unsigned int)=nullptr;
    HRESULT (__cdecl *unreg)()=nullptr;
    unsigned int (__cdecl *process)(unsigned short*,unsigned int)=nullptr;
    Api() {
        wchar_t path[32768]={};const DWORD length=GetModuleFileNameW(g_module,path,ARRAYSIZE(path));
        if (!length || length>=ARRAYSIZE(path)) return;
        std::wstring directory(path,length);directory.resize(directory.find_last_of(L"\\/")+1);
        worker=ToUtf8(directory+L"meltype-tip-worker.exe");
        module=LoadLibraryExW((directory+L"meltype_native.dll").c_str(),nullptr,LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR|LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (!module) return;
        object=reinterpret_cast<decltype(object)>(GetProcAddress(module,"meltype_tsf_service_object_create"));
        factory=reinterpret_cast<decltype(factory)>(GetProcAddress(module,"meltype_tsf_get_class_object"));
        reg=reinterpret_cast<decltype(reg)>(GetProcAddress(module,"meltype_tsf_register_server"));
        unreg=reinterpret_cast<decltype(unreg)>(GetProcAddress(module,"meltype_tsf_unregister_server"));
        process=reinterpret_cast<decltype(process)>(GetProcAddress(module,"meltype_tsf_process_name"));
    }
};
Api& api(){static Api value;return value;}
void __cdecl lock(int value){if(value) DllAddRef();else DllRelease();}
void __cdecl mode(void*,unsigned int op,int value) {
    try {switch(op) {
        case 0:TipLog(L"Meltype.exe: %s",value?L"つながりました":L"使えません (動いていない・モードが Meltype IME でない)");break;
        case 1:TipLog(L"応答を読めません");break;
        case 2:TipLog(L"入力モードの表示: %s",value?L"英数 (コードの行)":L"日本語");break;
        case 3:TipLog(L"コードの行: %s",value?L"日本語にしました":L"英数に戻しました");break;
        case 4:TipLog(L"入力欄の種類を読むところ: %s",value==3?L"Windows とアプリ":value==1?L"Windows":value==2?L"アプリ":L"無い");break;
        case 5:TipLog(L"入力欄の種類を読めないので、パスワード欄として扱います (%s)",value==1?L"ITfInputScope":value==2?L"GetInputScopes":value==3?L"VARIANT":L"Rust API");break;
        case 6:{const wchar_t* reasons[]={nullptr,L"英数",L"Ctrl・Alt・Windows キーを押している",L"この入力欄では使わない",L"英語の単語の続き",L"Meltype.exe が使えない",L"パスワードの入力欄"};if(value>0 && value<ARRAYSIZE(reasons)) TipLog(L"キーを通します: %s",reasons[value]);break;}
        case 7:TipLog(L"アプリが確定した打ちかけの文字を、変換中に戻しました");break;
        case 10:TipLog(L"キャレットが変換中の文字の外に出たので確定します");break;
        case 11:TipLog(L"入力モード: %s",value?L"日本語":L"英数");break;
    }}catch(...){}
}
void __cdecl key(void*,unsigned int op,int value) {
    try {switch(op) {
        case 0:TipLog(L"パスワードの入力欄なので何もしません");break;
        case 1:TipLog(L"英数文字を直接入力できませんでした。キーをアプリへ送り直します");break;
        case 2:TipLog(L"編集セッションを始められません: 0x%08X",static_cast<unsigned int>(value));break;
        case 3:TipLog(L"Meltype.exe から応答が来ないので、打った文字をそのまま入れました");break;
        case 4:mode(nullptr,1,0);break;
        case 5:mode(nullptr,2,value);break;
        case 6:mode(nullptr,4,value);break;
        case 7:mode(nullptr,5,value);break;
        case 8:TipLog(L"同じキーの送り直しが続くので、やめます");break;
        case 9:TipLog(L"キーをアプリに送り直せません: %lu",static_cast<DWORD>(value));break;
    }}catch(...){}
}
void __cdecl startup(void*,unsigned int op,int value) {
    try {
        if(op!=0){mode(nullptr,op-1,value);return;}
        std::wstring process;
        if(api().process){const auto n=api().process(nullptr,0);if(n && n<=32768){process.resize(n);if(api().process(reinterpret_cast<unsigned short*>(process.data()),n)==n)process.resize(n-1);else process.clear();}}
        TipLog(L"Activate (%s)%s",process.c_str(),value?L" 無効: セキュリティ保護された画面":L"");
    }catch(...){}
}
void __cdecl shutdown(void*,unsigned int op,int value) {
    try {if(op==0)TipLog(L"手放した変換中の文字を終えられませんでした: 0x%08X",static_cast<unsigned int>(value));else if(op==1)mode(nullptr,1,0);}catch(...){}
}
HRESULT __cdecl create(const GUID* iid,void** out) {
    if(!out)return E_INVALIDARG;*out=nullptr;if(!iid)return E_INVALIDARG;
    try {return api().object?api().object(api().worker.c_str(),iid,out,nullptr,mode,key,startup,shutdown,lock):E_FAIL;}catch(...){return E_FAIL;}
}
}
HRESULT RustGetClassObject(REFCLSID clsid,REFIID iid,void** out) {
    if(!out)return E_INVALIDARG;*out=nullptr;
    try {return api().factory?api().factory(&clsid,&iid,out,create,lock):CLASS_E_CLASSNOTAVAILABLE;}catch(...){return E_FAIL;}
}
HRESULT RustRegisterServer() {
    try {if(!api().reg)return E_FAIL;wchar_t path[32768]={};const auto n=GetModuleFileNameW(g_module,path,ARRAYSIZE(path));return n && n<ARRAYSIZE(path)?api().reg(path,n):E_FAIL;}catch(...){return E_FAIL;}
}
HRESULT RustUnregisterServer(){try{return api().unreg?api().unreg():E_FAIL;}catch(...){return E_FAIL;}}
}
