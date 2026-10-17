// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro
//
// Meltype の fcitx5 のアドオン (issue #99)。IBus のエンジン (linux/ibus-engine-meltype) と同じく、キーを Meltype の本体
// (libMeltypeNative.so、標準構成はRust本体) に渡し、返ってきた結果 (確定する文字・変換中の文字・候補)
// を入力欄に出す。本体は dlopen で読むので、ビルドに本体は要らない。
// 旧NativeAOT用のJSON互換解析は -DMELTYPE_RUST_CORE=OFF で明示的に選ぶ。

#include <dlfcn.h>
#include "json_value.hpp"
#ifdef MELTYPE_RUST_CORE
#include "rust_json.hpp"
#else
#include "compat/json_reader.hpp"
#include "compat/key_mapping.hpp"
#endif

#include <cstdint>
#include <cstdlib>
#include <memory>
#include <string>
#include <vector>

#include <fcitx-utils/capabilityflags.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/textformatflags.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/event.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextmanager.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/text.h>
#include <fcitx/userinterface.h>

#ifndef MELTYPE_DIR
#define MELTYPE_DIR "/opt/meltype"
#endif

namespace {
using meltype_fcitx::Json;

// ---- 本体 (libMeltypeNative.so) ----

struct Native {
    void *handle = nullptr;
#ifdef MELTYPE_RUST_CORE
    meltype_fcitx::JsonVisit visitJson = nullptr;
    uint64_t (*mapKey)(uint32_t,uint32_t) = nullptr;
    int (*mapModifiers)(uint32_t) = nullptr;
    uint32_t (*modePlan)(uint32_t,int) = nullptr;
    int (*frontendState)(void *) = nullptr;
    void (*setPreedit)(void *,int) = nullptr;
#endif
    int (*initMozc)(const char *, const char *) = nullptr;
    void *(*create)() = nullptr;
    void (*destroy)(void *) = nullptr;
    char *(*handleKey)(void *, int, int, int, const char *, const char *) = nullptr;
    char *(*commit)(void *) = nullptr;
    char *(*resetSession)(void *) = nullptr;
    char *(*selectCandidate)(void *, int) = nullptr;
    void (*setDirect)(void *, int) = nullptr;
    void (*setCanDelete)(void *, int) = nullptr; // 古い本体には無い
    char *(*roleKey)(void *, int) = nullptr;     // 古い本体には無い (入力中のキーの役割: #199)
    void (*free)(void *) = nullptr;

    bool load() {
        const char *dir = std::getenv("MELTYPE_DIR");
        std::string base = dir && *dir ? dir : MELTYPE_DIR;
        handle = dlopen((base + "/libMeltypeNative.so").c_str(), RTLD_NOW | RTLD_LOCAL);
        if (!handle) {
            FCITX_ERROR() << "Meltype: libMeltypeNative.so を読めません: " << dlerror();
            return false;
        }
        auto sym = [this](const char *name) { return dlsym(handle, name); };
        initMozc = reinterpret_cast<decltype(initMozc)>(sym("meltype_init_mozc"));
        create = reinterpret_cast<decltype(create)>(sym("meltype_create"));
        destroy = reinterpret_cast<decltype(destroy)>(sym("meltype_destroy"));
        handleKey = reinterpret_cast<decltype(handleKey)>(sym("meltype_handle_key"));
        commit = reinterpret_cast<decltype(commit)>(sym("meltype_commit"));
        resetSession = reinterpret_cast<decltype(resetSession)>(sym("meltype_reset"));
        selectCandidate = reinterpret_cast<decltype(selectCandidate)>(sym("meltype_select_candidate"));
        setDirect = reinterpret_cast<decltype(setDirect)>(sym("meltype_set_direct"));
        setCanDelete = reinterpret_cast<decltype(setCanDelete)>(sym("meltype_set_can_delete"));
        roleKey = reinterpret_cast<decltype(roleKey)>(sym("meltype_role_key"));
        free = reinterpret_cast<decltype(free)>(sym("meltype_free"));
#ifdef MELTYPE_RUST_CORE
        visitJson = reinterpret_cast<decltype(visitJson)>(sym("meltype_json_visit_utf8"));
        mapKey = reinterpret_cast<decltype(mapKey)>(sym("meltype_fcitx_key"));
        mapModifiers = reinterpret_cast<decltype(mapModifiers)>(sym("meltype_fcitx_modifiers"));
        modePlan = reinterpret_cast<decltype(modePlan)>(sym("meltype_fcitx_mode"));
        create = reinterpret_cast<decltype(create)>(sym("meltype_fcitx_create"));
        destroy = reinterpret_cast<decltype(destroy)>(sym("meltype_fcitx_destroy"));
        setDirect = reinterpret_cast<decltype(setDirect)>(sym("meltype_fcitx_set_direct"));
        frontendState = reinterpret_cast<decltype(frontendState)>(sym("meltype_fcitx_state"));
        setPreedit = reinterpret_cast<decltype(setPreedit)>(sym("meltype_fcitx_set_preedit"));
        if (!visitJson || !mapKey || !mapModifiers || !modePlan || !frontendState || !setPreedit) {
            FCITX_ERROR() << "Meltype: Rust fcitx JSON/key ABI is missing";
            return false;
        }
#endif
        if (!create || !destroy || !handleKey || !commit || !selectCandidate || !setDirect || !free) {
            FCITX_ERROR() << "Meltype: libMeltypeNative.so の関数が足りません";
            return false;
        }
        if (initMozc && !initMozc((base + "/mozc/meltype_mozc_helper").c_str(), nullptr)) {
            FCITX_WARN() << "Meltype: Mozc を使えません (漢字変換はひらがなのままになります)";
        }
        return true;
    }

    bool readJson(const std::string &text, Json &value) const {
#ifdef MELTYPE_RUST_CORE
        return meltype_fcitx::readRustJson(text, value, visitJson);
#else
        return meltype_fcitx::JsonReader(text).read(value);
#endif
    }

    // 本体が返した JSON を受け取って解放する。NULL なら空 (キーはアプリに渡す)。
    std::string take(char *pointer) const {
        if (!pointer) return {};
        std::string text(pointer);
        free(pointer);
        return text;
    }
};

// ---- キー: fcitx5 のキー → Meltype の本体が使う Windows の仮想キーコード (ibus-engine-meltype と同じ) ----

// UTF-8 の文字列の、文字単位の位置 → バイトの位置
size_t byteOffset(const std::string &text, size_t chars) {
    size_t i = 0;
    while (i < text.size() && chars > 0) {
        unsigned char c = static_cast<unsigned char>(text[i]);
        i += c < 0x80 ? 1 : c < 0xE0 ? 2 : c < 0xF0 ? 3 : 4;
        chars--;
    }
    return std::min(i, text.size());
}

size_t charLength(const std::string &text) {
    size_t n = 0;
    for (unsigned char c : text)
        if ((c & 0xC0) != 0x80) n++;
    return n;
}

size_t utf16Length(const std::string &text) {
    size_t units = 0;
    for (unsigned char c : text)
        if ((c & 0xC0) != 0x80) units += c >= 0xF0 ? 2 : 1;
    return units;
}

class MeltypeEngine;

// 入力欄ごとの本体のセッション
class MeltypeState : public fcitx::InputContextProperty {
public:
    MeltypeState(const Native &native) : native_(native) {
        if (native_.create) session_ = native_.create();
    }
    ~MeltypeState() override {
        if (session_ && native_.destroy) native_.destroy(session_);
    }
    void *session() const { return session_; }
    bool direct() const {
#ifdef MELTYPE_RUST_CORE
        const int flags=native_.frontendState(session_); return flags>=0 && (flags&1)!=0;
#else
        return direct_;
#endif
    }
    void setDirect(bool value) {
#ifndef MELTYPE_RUST_CORE
        direct_=value;
#endif
        native_.setDirect(session_,value?1:0);
    }
    bool preeditVisible() const {
#ifdef MELTYPE_RUST_CORE
        const int flags=native_.frontendState(session_); return flags>=0 && (flags&2)!=0;
#else
        return preeditVisible_;
#endif
    }
    void setPreeditVisible(bool value) {
#ifdef MELTYPE_RUST_CORE
        native_.setPreedit(session_,value?1:0);
#else
        preeditVisible_=value;
#endif
    }

private:
    const Native &native_;
    void *session_ = nullptr;
#ifndef MELTYPE_RUST_CORE
    bool direct_=false,preeditVisible_=false;
#endif
};

class MeltypeCandidate : public fcitx::CandidateWord {
public:
    MeltypeCandidate(MeltypeEngine *engine, std::string text, int index)
        : fcitx::CandidateWord(fcitx::Text(std::move(text))), engine_(engine), index_(index) {}
    void select(fcitx::InputContext *ic) const override;

private:
    MeltypeEngine *engine_;
    int index_;
};

class MeltypeEngine : public fcitx::InputMethodEngineV2 {
public:
    explicit MeltypeEngine(fcitx::Instance *instance)
        : instance_(instance), factory_([this](fcitx::InputContext &) { return new MeltypeState(native_); }) {
        loaded_ = native_.load();
        instance_->inputContextManager().registerProperty("meltypeState", &factory_);
    }

    void keyEvent(const fcitx::InputMethodEntry &, fcitx::KeyEvent &event) override {
        if (!loaded_ || event.isRelease()) return;
        auto *ic = event.inputContext();
        auto *state = ic->propertyFor(&factory_);
        if (!state->session()) return;
        syncCanDelete(ic, state->session());
        const auto key = event.key();
        const auto sym = key.sym();
#ifdef MELTYPE_RUST_CORE
        const auto modePlan = native_.modePlan(sym,state->direct()?1:0);
#else
        const auto modePlan = meltype_fcitx::modePlan(sym,state->direct());
#endif
        // 入力中の 変換・無変換・ひらがな/カタカナ: 設定で役割を分けていれば、本体が変換ボックスで処理する (#199)
        const int roleKey = static_cast<int>((modePlan>>8)&0xffu);
        if (native_.roleKey && roleKey != 0) {
            const int vk = roleKey;
            auto handled = native_.take(native_.roleKey(state->session(), vk));
            Json result;
            if (!handled.empty() && native_.readJson(handled, result)) {
                apply(ic, result);
                event.filterAndAccept();
                return;
            }
        }
        // 半角/全角: 英数 (直接入力) ⇔ 日本語。英数・ひらがなのキーでも切り替える (JIS キーボード)。
        if (modePlan & 1u) {
            const bool toDirect = (modePlan & 2u) != 0;
            apply(ic, native_.take(native_.commit(state->session())));
            state->setDirect(toDirect);
            // 切り替えたモードをカーソル位置に出す (Mozc と同じ入力パネルの枠)
            ic->inputPanel().setAuxUp(fcitx::Text(toDirect ? "A" : "あ"));
            ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
            event.filterAndAccept();
            return;
        }
#ifdef MELTYPE_RUST_CORE
        const auto packed = native_.mapKey(sym, fcitx::Key::keySymToUnicode(sym));
        const int vk = static_cast<int>(packed & 0xffffffffu);
        const uint32_t character = static_cast<uint32_t>(packed >> 32);
#else
        auto [vk, character] = meltype_fcitx::virtualKey(sym);
#endif
        if (vk == 0) return;
        const auto states = key.states();
#ifdef MELTYPE_RUST_CORE
        const int modifiers = native_.mapModifiers(states.toInteger());
#else
        int modifiers = 0;
        if (states.test(fcitx::KeyState::Shift)) modifiers |= 1;
        if (states.test(fcitx::KeyState::Ctrl)) modifiers |= 2;
        if (states.test(fcitx::KeyState::Alt)) modifiers |= 4;
        if (states.test(fcitx::KeyState::Super)) modifiers |= 8;
#endif
        std::string before, after;
        bool surrounding = !state->preeditVisible() && readSurrounding(ic, before, after);
        auto text = native_.take(native_.handleKey(state->session(), vk, static_cast<int>(character), modifiers,
                                                   surrounding ? before.c_str() : nullptr, surrounding ? after.c_str() : nullptr));
        if (text.empty()) return;
        Json result;
        if (!native_.readJson(text, result)) return;
        apply(ic, result);
        if (result.flag("consumed")) event.filterAndAccept();
    }

    // 別の入力欄・アプリに移るとき、入力欄が変わったときは、未確定の内容をそのまま確定する。
    void reset(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override { commitPending(event.inputContext()); }
    void deactivate(const fcitx::InputMethodEntry &, fcitx::InputContextEvent &event) override { commitPending(event.inputContext()); }

    void selectCandidate(fcitx::InputContext *ic, int index) {
        auto *state = ic->propertyFor(&factory_);
        if (!state->session()) return;
        syncCanDelete(ic, state->session());
        apply(ic, native_.take(native_.selectCandidate(state->session(), index)));
    }

private:
    fcitx::Instance *instance_;
    Native native_;
    bool loaded_ = false;
    fcitx::FactoryFor<MeltypeState> factory_;

    void commitPending(fcitx::InputContext *ic) {
        auto *state = ic->propertyFor(&factory_);
        if (!state->session()) return;
        syncCanDelete(ic, state->session());
        apply(ic, native_.take(native_.commit(state->session())));
    }

    // 入力欄のキャレットの前後の文字列 (それぞれ 20 文字まで)。入力欄が対応していなければ false。
    static bool readSurrounding(fcitx::InputContext *ic, std::string &before, std::string &after, size_t limit = 20) {
        if (!ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) || !ic->surroundingText().isValid()) return false;
        const auto &text = ic->surroundingText().text();
        if (text.empty()) return false;
        size_t cursor = ic->surroundingText().cursor();
        size_t start = cursor > limit ? cursor - limit : 0;
        size_t from = byteOffset(text, start), at = byteOffset(text, cursor), to = byteOffset(text, cursor + 20);
        before = text.substr(from, at - from);
        after = text.substr(at, to - at);
        return true;
    }

    // 確定し直し (DeleteBefore) は、入力欄が周りの文字の削除に対応しているときだけ本体が行う。
    // 本体を呼ぶ前に毎回伝える (入力欄を移った最初のキーで、前の入力欄の値のまま確定し直さないように)。
    void syncCanDelete(fcitx::InputContext *ic, void *session) {
        if (!native_.setCanDelete || !session) return;
        bool canDelete = ic->capabilityFlags().test(fcitx::CapabilityFlag::SurroundingText) && ic->surroundingText().isValid();
        native_.setCanDelete(session, canDelete ? 1 : 0);
    }

    void apply(fcitx::InputContext *ic, const std::string &text) {
        if (text.empty()) return;
        Json result;
        if (native_.readJson(text, result)) apply(ic, result);
    }

    void apply(fcitx::InputContext *ic, const Json &result) {
        if (auto *commits = result.get("commits"); commits && commits->kind == Json::Kind::Array) {
            for (const auto &edit : commits->items) {
                int remove = edit.integer("deleteBefore");
                if (remove > 0) {
                    const auto expected = edit.str("expect");
                    std::string before, after;
                    if (expected.empty() || utf16Length(expected) != static_cast<size_t>(remove)
                        || !readSurrounding(ic, before, after, charLength(expected))
                        || before.size() < expected.size()
                        || before.compare(before.size() - expected.size(), expected.size(), expected) != 0) {
                        auto *session = ic->propertyFor(&factory_)->session();
                        if (native_.resetSession) native_.take(native_.resetSession(session));
                        hidePreedit(ic);
                        return;
                    }
                    hidePreedit(ic);
                    const auto count = static_cast<unsigned int>(charLength(expected));
                    ic->deleteSurroundingText(-static_cast<int>(count), count);
                } else {
                    ic->inputPanel().reset();
                    ic->propertyFor(&factory_)->setPreeditVisible(false);
                }
                auto committed = edit.str("text");
                if (!committed.empty()) ic->commitString(committed);
            }
        }
        auto *view = result.get("view");
        if (view && view->kind == Json::Kind::Object) show(ic, *view);
        else hidePreedit(ic);
    }

    void show(fcitx::InputContext *ic, const Json &view) {
        auto &panel = ic->inputPanel();
        panel.reset();
        const auto text = view.str("text");
        const auto clauses = view.strings("clauses");
        const bool converting = view.flag("converting");
        const int selectedClause = view.integer("selectedClause", -1);
        fcitx::Text preedit;
        if (converting && !clauses.empty()) {
            // 変換中: 文節ごとに下線。選んでいる文節は強調する。
            for (size_t i = 0; i < clauses.size(); i++) {
                fcitx::TextFormatFlags format = fcitx::TextFormatFlag::Underline;
                if (static_cast<int>(i) == selectedClause) format |= fcitx::TextFormatFlag::HighLight;
                preedit.append(clauses[i], format);
            }
        } else {
            preedit.append(text, fcitx::TextFormatFlag::Underline);
        }
        preedit.setCursor(static_cast<int>(preedit.toString().size()));
        if (ic->capabilityFlags().test(fcitx::CapabilityFlag::Preedit)) panel.setClientPreedit(preedit);
        else panel.setPreedit(preedit);
        ic->propertyFor(&factory_)->setPreeditVisible(!text.empty());
        // 案内 (Space で変換 など) と もしかして は補助テキストに出す
        auto hint = view.str("hint");
        if (auto suggestion = view.str("suggestion"); !suggestion.empty()) hint = suggestion + "　" + hint;
        if (!hint.empty()) panel.setAuxDown(fcitx::Text(hint));
        const auto candidates = view.strings("candidates");
        if (converting && candidates.size() > 1) {
            auto list = std::make_unique<fcitx::CommonCandidateList>();
            list->setPageSize(9);
            list->setLabels({"1. ", "2. ", "3. ", "4. ", "5. ", "6. ", "7. ", "8. ", "9. "});
            list->setCursorPositionAfterPaging(fcitx::CursorPositionAfterPaging::DonotChange);
            for (size_t i = 0; i < candidates.size(); i++) list->append<MeltypeCandidate>(this, candidates[i], static_cast<int>(i));
            int selected = view.integer("selectedIndex", 0);
            if (selected >= 0 && selected < static_cast<int>(candidates.size())) list->setGlobalCursorIndex(selected);
            panel.setCandidateList(std::move(list));
        }
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
    }

    void hidePreedit(fcitx::InputContext *ic) {
        ic->inputPanel().reset();
        ic->updatePreedit();
        ic->updateUserInterface(fcitx::UserInterfaceComponent::InputPanel);
        ic->propertyFor(&factory_)->setPreeditVisible(false);
    }
};

void MeltypeCandidate::select(fcitx::InputContext *ic) const { engine_->selectCandidate(ic, index_); }

class MeltypeEngineFactory : public fcitx::AddonFactory {
    fcitx::AddonInstance *create(fcitx::AddonManager *manager) override { return new MeltypeEngine(manager->instance()); }
};

} // namespace

FCITX_ADDON_FACTORY(MeltypeEngineFactory);
