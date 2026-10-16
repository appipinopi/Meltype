// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai
//
// タスクバーの入力モードの表示 (「あ」= 日本語 /「A」= 英数)。クリックで切り替える。

#pragma once

#include "Globals.h"

namespace meltype {

class TextService;

#ifdef MELTYPE_RUST_CORE
// The COM interfaces, notifications and icon belong to Rust. This temporary
// owner only connects the remaining C++ text service's mode callbacks.
class LangBarButton final {
public:
    explicit LangBarButton(TextService* service);
    ITfLangBarItem* Item() const { return item_; }
    void Update();
    void Detach();
    ULONG Release();
private:
    ~LangBarButton();
    ITfLangBarItemButton* item_ = nullptr;
};
#else
class LangBarButton final : public ITfLangBarItemButton, public ITfSource {
public:
    explicit LangBarButton(TextService* service);
    ITfLangBarItem* Item() { return this; }

    // IUnknown
    STDMETHODIMP QueryInterface(REFIID riid, void** ppv) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;

    // ITfLangBarItem
    STDMETHODIMP GetInfo(TF_LANGBARITEMINFO* info) override;
    STDMETHODIMP GetStatus(DWORD* status) override;
    STDMETHODIMP Show(BOOL) override { return E_NOTIMPL; }
    STDMETHODIMP GetTooltipString(BSTR* tooltip) override;

    // ITfLangBarItemButton
    STDMETHODIMP OnClick(TfLBIClick click, POINT point, const RECT* area) override;
    STDMETHODIMP InitMenu(ITfMenu*) override { return S_OK; }
    STDMETHODIMP OnMenuSelect(UINT) override { return S_OK; }
    STDMETHODIMP GetIcon(HICON* icon) override;
    STDMETHODIMP GetText(BSTR* text) override;

    // ITfSource
    STDMETHODIMP AdviseSink(REFIID riid, IUnknown* sink, DWORD* cookie) override;
    STDMETHODIMP UnadviseSink(DWORD cookie) override;

    // 入力モードが変わったら呼ぶ
    void Update();
    // TextService が終わるとき。タスクバーがまだこのボタンを持っていても、消えた TextService を触らないように
    void Detach() { service_ = nullptr; }

private:
    ~LangBarButton();

    LONG refs_ = 1;
    TextService* service_;  // Deactivate で Detach して nullptr にする
    ITfLangBarItemSink* sink_ = nullptr;
};
#endif

}  // namespace meltype
