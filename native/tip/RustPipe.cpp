// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Replaces Pipe.cpp for Rust preview builds. The TSF shell calls Rust inside
// the client process; no C# daemon or named pipe is required.
#include "Pipe.h"
#include "DisplayAttributes.h"
#include "LangBar.h"
#include "TextService.h"
#include "CandidateWindow.h"
#include <new>

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
    void* (__cdecl *candidateCreate)(void*, void (__cdecl *)(void*,int,int)) = nullptr;
    int (__cdecl *candidateShow)(void*,const char*,const RECT*) = nullptr;
    void (__cdecl *candidateHide)(void*) = nullptr;
    int (__cdecl *candidateVisible)(void*) = nullptr;
    void (__cdecl *candidateDestroy)(void*) = nullptr;
    HRESULT (__cdecl *classObject)(const GUID*,const GUID*,void**,HRESULT (__cdecl *)(const GUID*,void**),void (__cdecl *)(int)) = nullptr;
    HRESULT (__cdecl *registerServer)(const wchar_t*,unsigned int) = nullptr;
    HRESULT (__cdecl *unregisterServer)() = nullptr;
    HRESULT (__cdecl *editSession)(void*,unsigned int,unsigned int,void*,HRESULT (__cdecl *)(void*,unsigned int),void (__cdecl *)(void*)) = nullptr;
    int (__cdecl *surrounding)(void*,unsigned int,void*,void (__cdecl *)(void*,const unsigned short*,unsigned int,const unsigned short*,unsigned int)) = nullptr;
    int (__cdecl *extendPrevious)(void*,unsigned int,int,const unsigned short*,unsigned int) = nullptr;
    int (__cdecl *writeRange)(void*,void*,unsigned int,const unsigned short*,unsigned int,int) = nullptr;
    void* (__cdecl *startComposition)(void*,void*,void*,unsigned int) = nullptr;
    int (__cdecl *updateComposition)(void*,void*,unsigned int,const char*,unsigned int,unsigned int,unsigned int) = nullptr;
    int (__cdecl *endComposition)(void*,void*,unsigned int) = nullptr;
    void (__cdecl *setAttribute)(void*,void*,unsigned int,unsigned int) = nullptr;
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
        candidateCreate = reinterpret_cast<decltype(candidateCreate)>(GetProcAddress(module, "meltype_tsf_candidate_create"));
        candidateShow = reinterpret_cast<decltype(candidateShow)>(GetProcAddress(module, "meltype_tsf_candidate_show"));
        candidateHide = reinterpret_cast<decltype(candidateHide)>(GetProcAddress(module, "meltype_tsf_candidate_hide"));
        candidateVisible = reinterpret_cast<decltype(candidateVisible)>(GetProcAddress(module, "meltype_tsf_candidate_visible"));
        candidateDestroy = reinterpret_cast<decltype(candidateDestroy)>(GetProcAddress(module, "meltype_tsf_candidate_destroy"));
        classObject = reinterpret_cast<decltype(classObject)>(GetProcAddress(module, "meltype_tsf_get_class_object"));
        registerServer = reinterpret_cast<decltype(registerServer)>(GetProcAddress(module, "meltype_tsf_register_server"));
        unregisterServer = reinterpret_cast<decltype(unregisterServer)>(GetProcAddress(module, "meltype_tsf_unregister_server"));
        editSession = reinterpret_cast<decltype(editSession)>(GetProcAddress(module, "meltype_tsf_request_edit_session"));
        surrounding = reinterpret_cast<decltype(surrounding)>(GetProcAddress(module, "meltype_tsf_read_surrounding"));
        extendPrevious = reinterpret_cast<decltype(extendPrevious)>(GetProcAddress(module, "meltype_tsf_extend_previous"));
        writeRange = reinterpret_cast<decltype(writeRange)>(GetProcAddress(module, "meltype_tsf_write_range"));
        startComposition = reinterpret_cast<decltype(startComposition)>(GetProcAddress(module, "meltype_tsf_start_composition"));
        updateComposition = reinterpret_cast<decltype(updateComposition)>(GetProcAddress(module, "meltype_tsf_update_composition"));
        endComposition = reinterpret_cast<decltype(endComposition)>(GetProcAddress(module, "meltype_tsf_end_composition"));
        setAttribute = reinterpret_cast<decltype(setAttribute)>(GetProcAddress(module, "meltype_tsf_set_attribute"));
    }
    bool ready() const { return create && destroy && request && freeText; }
    // The module stays loaded for the process lifetime. Do not unload Rust
    // thread state under the Windows loader lock during TSF DLL teardown.
};
RustApi& api() { static RustApi value; return value; }
HRESULT __cdecl CreateTextService(const GUID* iid, void** out) {
    auto* service = new (std::nothrow) TextService();
    if (!service) return E_OUTOFMEMORY;
    HRESULT result = service->QueryInterface(*iid, out);
    service->Release();
    return result;
}
void __cdecl LockShell(int lock) { if (lock) DllAddRef(); else DllRelease(); }
}
HRESULT RustGetClassObject(REFCLSID clsid, REFIID iid, void** out) {
    if (!out) return E_INVALIDARG;
    *out = nullptr;
    if (!api().classObject) return CLASS_E_CLASSNOTAVAILABLE;
    return api().classObject(&clsid,&iid,out,CreateTextService,LockShell);
}
HRESULT RustRegisterServer() {
    if (!api().registerServer) return E_FAIL;
    wchar_t path[32768];
    DWORD length = GetModuleFileNameW(g_module,path,ARRAYSIZE(path));
    if (!length || length >= ARRAYSIZE(path)) return E_FAIL;
    return api().registerServer(path,length);
}
HRESULT RustUnregisterServer() { return api().unregisterServer ? api().unregisterServer() : E_FAIL; }
namespace {
struct EditAction {
    std::function<HRESULT(TfEditCookie)> action;
    explicit EditAction(std::function<HRESULT(TfEditCookie)> value) : action(std::move(value)) { DllAddRef(); }
    ~EditAction() {
        // Captures may own C++ COM objects with destructors in this shell.
        // Destroy those captures before dropping the DLL's final pin.
        action = {};
        DllRelease();
    }
    static HRESULT __cdecl Invoke(void* context, unsigned int cookie) {
        try { return static_cast<EditAction*>(context)->action(cookie); }
        catch (...) { return E_FAIL; }
    }
    static void __cdecl Destroy(void* context) { delete static_cast<EditAction*>(context); }
};
}
HRESULT RunEditSession(ITfContext* context, TfClientId client, DWORD flags, std::function<HRESULT(TfEditCookie)> action) {
    if (!context) return E_INVALIDARG;
    if (!api().editSession) return E_FAIL;
    auto* owned = new (std::nothrow) EditAction(std::move(action));
    if (!owned) return E_OUTOFMEMORY;
    // Rust consumes owned on success, immediate failure and queued execution.
    return api().editSession(context,client,flags,owned,EditAction::Invoke,EditAction::Destroy);
}
namespace {
struct SurroundingReceiver { std::wstring* before; std::wstring* after; };
void __cdecl ReceiveSurrounding(void* value,const unsigned short* before,unsigned int beforeLength,const unsigned short* after,unsigned int afterLength) {
    auto* receiver = static_cast<SurroundingReceiver*>(value);
    receiver->before->assign(reinterpret_cast<const wchar_t*>(before),beforeLength);
    receiver->after->assign(reinterpret_cast<const wchar_t*>(after),afterLength);
}
}
void RustSurroundingText(TfEditCookie cookie,ITfContext* context,std::wstring& before,std::wstring& after) {
    SurroundingReceiver receiver = {&before,&after};
    if (api().surrounding) api().surrounding(context,cookie,&receiver,ReceiveSurrounding);
}
bool RustExtendPrevious(TfEditCookie cookie,ITfRange* range,LONG count,const std::wstring& expected) {
    return api().extendPrevious && api().extendPrevious(range,cookie,count,reinterpret_cast<const unsigned short*>(expected.data()),static_cast<unsigned int>(expected.size())) != 0;
}
bool RustWriteRange(TfEditCookie cookie,ITfContext* context,ITfRange* range,const std::wstring& text,bool keepCaret) {
    return api().writeRange && api().writeRange(context,range,cookie,reinterpret_cast<const unsigned short*>(text.data()),static_cast<unsigned int>(text.size()),keepCaret ? 1 : 0) != 0;
}
ITfComposition* RustStartComposition(TfEditCookie cookie, ITfContext* context, ITfCompositionSink* sink, ITfRange* range) {
    return api().startComposition ? static_cast<ITfComposition*>(api().startComposition(context,sink,range,cookie)) : nullptr;
}
bool RustUpdateComposition(TfEditCookie cookie,ITfContext* context,ITfComposition* composition,const JsonValue& view,TfGuidAtom input,TfGuidAtom converted,TfGuidAtom target) {
    if (!api().updateComposition) return false;
    std::string clauses = "[";
    for (size_t i = 0; i < view[L"clauses"].array.size(); ++i) {
        if (i) clauses += ',';
        clauses += JsonString(view[L"clauses"].array[i].Str());
    }
    std::string json = "{\"text\":" + JsonString(view[L"text"].Str()) + ",\"converting\":" + (view[L"converting"].Bool() ? "true" : "false") +
        ",\"clauses\":" + clauses + "],\"selectedClause\":" + std::to_string(view[L"selectedClause"].Int()) + "}";
    return api().updateComposition(context,composition,cookie,json.c_str(),input,converted,target) != 0;
}
void RustEndComposition(TfEditCookie cookie,ITfContext* context,ITfComposition* composition) {
    if (api().endComposition) api().endComposition(context,composition,cookie);
}
void RustSetAttribute(TfEditCookie cookie,ITfContext* context,ITfRange* range,TfGuidAtom atom) {
    if (api().setAttribute) api().setAttribute(context,range,cookie,atom);
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
struct CandidateWindow::Impl {
    SelectHandler callback;
    void* handle = nullptr;
    static void __cdecl Select(void* context, int index, int prediction) {
        auto* self = static_cast<Impl*>(context);
        // A selection can destroy the owner. Copy the callable before entering it.
        SelectHandler callback = self->callback;
        if (callback) callback(index, prediction != 0);
    }
};
CandidateWindow::CandidateWindow(SelectHandler callback) : impl_(new Impl()) {
    impl_->callback = std::move(callback);
    if (api().candidateCreate && api().candidateShow && api().candidateHide && api().candidateVisible && api().candidateDestroy) {
        impl_->handle = api().candidateCreate(impl_, Impl::Select);
    }
}
CandidateWindow::~CandidateWindow() {
    if (impl_->handle) api().candidateDestroy(impl_->handle);
    delete impl_;
}
void CandidateWindow::Show(const CandidateView& view, const RECT& anchor) {
    if (!impl_->handle) return;
    auto list = [](const std::vector<std::wstring>& values) {
        std::string json = "[";
        for (size_t i = 0; i < values.size(); ++i) { if (i) json += ','; json += JsonString(values[i]); }
        return json + ']';
    };
    std::string json = "{\"candidates\":" + list(view.candidates) + ",\"notes\":" + list(view.notes) +
        ",\"selected\":" + std::to_string(view.selected) + ",\"suggestion\":" + JsonString(view.suggestion) +
        ",\"meaning\":" + JsonString(view.meaning) + ",\"prediction\":" + (view.prediction ? "true" : "false") +
        ",\"typed\":" + JsonString(view.typed) + "}";
    api().candidateShow(impl_->handle, json.c_str(), &anchor);
}
void CandidateWindow::Hide() { if (impl_->handle) api().candidateHide(impl_->handle); }
bool CandidateWindow::Visible() const { return impl_->handle && api().candidateVisible(impl_->handle) != 0; }
// Rust owns its window classes and per-window drawing resources. The Rust DLL
// stays loaded, so none of its window procedures point into this TSF shell.
void CandidateWindow::RegisterClasses() {}
void CandidateWindow::UnregisterClasses() {}
void CandidateWindow::ReleaseFactories() {}
} // namespace meltype
