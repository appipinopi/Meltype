// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Replaces Pipe.cpp for Rust preview builds. The TSF shell calls Rust inside
// the client process; no C# daemon or named pipe is required.
#include "Pipe.h"
#include "DisplayAttributes.h"
#include "LangBar.h"
#include "TextService.h"

namespace meltype {
namespace {
struct RustApi {
    HMODULE module = nullptr;
    void* (__cdecl *create)() = nullptr;
    void (__cdecl *destroy)(void*) = nullptr;
    char* (__cdecl *request)(void*, const char*) = nullptr;
    void (__cdecl *freeText)(void*) = nullptr;
    void* (__cdecl *displayAttribute)(const GUID*) = nullptr;
    void* (__cdecl *displayAttributeEnum)() = nullptr;
    void* (__cdecl *langbar)(void*, int (__cdecl *)(void*), void (__cdecl *)(void*)) = nullptr;
    void (__cdecl *langbarUpdate)(void*) = nullptr;
    void (__cdecl *langbarDetach)(void*) = nullptr;
    RustApi() {
        wchar_t path[32768];
        const DWORD size = GetModuleFileNameW(g_module, path, ARRAYSIZE(path));
        if (size == 0 || size >= ARRAYSIZE(path)) return;
        std::wstring directory(path, size);
        directory.resize(directory.find_last_of(L"\\/") + 1);
        // Absolute path and restricted dependency search: never load from CWD.
        module = LoadLibraryExW((directory + L"meltype_native.dll").c_str(), nullptr,
                               LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (!module) return;
        create = reinterpret_cast<decltype(create)>(GetProcAddress(module, "meltype_tip_create"));
        destroy = reinterpret_cast<decltype(destroy)>(GetProcAddress(module, "meltype_tip_destroy"));
        request = reinterpret_cast<decltype(request)>(GetProcAddress(module, "meltype_tip_request"));
        freeText = reinterpret_cast<decltype(freeText)>(GetProcAddress(module, "meltype_free"));
        displayAttribute = reinterpret_cast<decltype(displayAttribute)>(GetProcAddress(module, "meltype_tsf_create_display_attribute"));
        displayAttributeEnum = reinterpret_cast<decltype(displayAttributeEnum)>(GetProcAddress(module, "meltype_tsf_create_display_attribute_enum"));
        langbar = reinterpret_cast<decltype(langbar)>(GetProcAddress(module, "meltype_tsf_create_langbar"));
        langbarUpdate = reinterpret_cast<decltype(langbarUpdate)>(GetProcAddress(module, "meltype_tsf_langbar_update"));
        langbarDetach = reinterpret_cast<decltype(langbarDetach)>(GetProcAddress(module, "meltype_tsf_langbar_detach"));
    }
    bool ready() const { return create && destroy && request && freeText; }
    // The module stays loaded for the process lifetime. Do not unload Rust
    // thread state under the Windows loader lock during TSF DLL teardown.
};
RustApi& api() { static RustApi value; return value; }
}
bool PipeClient::Transact(const std::string& request, std::string& response, DWORD, bool) {
    if (!api().ready()) return false;
    if (!rustConnection_) rustConnection_ = api().create();
    if (!rustConnection_) return false;
    char* text = api().request(rustConnection_, request.c_str());
    if (!text) return false;
    response.assign(text); api().freeText(text); return true;
}
void PipeClient::Close() {
    if (rustConnection_ && api().ready()) api().destroy(rustConnection_);
    rustConnection_ = nullptr;
}
ITfDisplayAttributeInfo* CreateDisplayAttributeInfo(REFGUID guid) {
    return api().displayAttribute ? static_cast<ITfDisplayAttributeInfo*>(api().displayAttribute(&guid)) : nullptr;
}
IEnumTfDisplayAttributeInfo* CreateDisplayAttributeEnum() {
    return api().displayAttributeEnum ? static_cast<IEnumTfDisplayAttributeInfo*>(api().displayAttributeEnum()) : nullptr;
}
namespace {
int __cdecl ReadMode(void* service) { return static_cast<TextService*>(service)->IsOpen() ? 1 : 0; }
void __cdecl ToggleMode(void* service) { static_cast<TextService*>(service)->ToggleInputMode(nullptr); }
}
LangBarButton::LangBarButton(TextService* service) {
    DllAddRef();
    if (api().langbar && api().langbarUpdate && api().langbarDetach) {
        item_ = static_cast<ITfLangBarItemButton*>(api().langbar(service, ReadMode, ToggleMode));
    }
}
LangBarButton::~LangBarButton() {
    Detach();
    if (item_) item_->Release();
    DllRelease();
}
void LangBarButton::Update() { if (item_) api().langbarUpdate(item_); }
void LangBarButton::Detach() { if (item_) api().langbarDetach(item_); }
ULONG LangBarButton::Release() { delete this; return 0; }
} // namespace meltype
