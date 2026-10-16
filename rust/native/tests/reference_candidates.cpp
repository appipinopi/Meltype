// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
// Test-only reference: compile the upstream renderer directly. This DLL is
// never part of the Rust product or registered as an input method.
#include "../../../native/tip/CandidateWindow.cpp"
#include "../../../native/tip/Json.cpp"
namespace meltype {
HINSTANCE g_module = nullptr;
LONG g_locks = 0;
std::string ToUtf8(const std::wstring& value) {
    int size = WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0, nullptr, nullptr);
    std::string out(size, '\0');
    WideCharToMultiByte(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), out.data(), size, nullptr, nullptr);
    return out;
}
std::wstring FromUtf8(const std::string& value) {
    int size = MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), nullptr, 0);
    std::wstring out(size, '\0');
    MultiByteToWideChar(CP_UTF8, 0, value.data(), static_cast<int>(value.size()), out.data(), size);
    return out;
}
}
BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) meltype::g_module = module;
    return TRUE;
}
extern "C" __declspec(dllexport) void* __cdecl meltype_tsf_candidate_create(void* context, void (__cdecl *select)(void*,int,int)) {
    return new meltype::CandidateWindow([=](int i, bool prediction) { if (select) select(context,i,prediction ? 1 : 0); });
}
extern "C" __declspec(dllexport) int __cdecl meltype_tsf_candidate_show(void* window, const char* json, const RECT* anchor) {
    meltype::JsonValue value;
    if (!window || !json || !anchor || !meltype::ParseJson(json,value)) return 0;
    meltype::CandidateView view;
    for (const auto& text : value[L"candidates"].array) view.candidates.push_back(text.Str());
    for (const auto& text : value[L"notes"].array) view.notes.push_back(text.Str());
    view.selected = value[L"selected"].Int(); view.typed = value[L"typed"].Str();
    view.suggestion = value[L"suggestion"].Str(); view.meaning = value[L"meaning"].Str(); view.prediction = value[L"prediction"].Bool();
    static_cast<meltype::CandidateWindow*>(window)->Show(view,*anchor);
    return static_cast<meltype::CandidateWindow*>(window)->Visible() ? 1 : 0;
}
extern "C" __declspec(dllexport) void __cdecl meltype_tsf_candidate_hide(void* window) { static_cast<meltype::CandidateWindow*>(window)->Hide(); }
extern "C" __declspec(dllexport) int __cdecl meltype_tsf_candidate_visible(void* window) { return static_cast<meltype::CandidateWindow*>(window)->Visible() ? 1 : 0; }
extern "C" __declspec(dllexport) void __cdecl meltype_tsf_candidate_destroy(void* window) { delete static_cast<meltype::CandidateWindow*>(window); }
