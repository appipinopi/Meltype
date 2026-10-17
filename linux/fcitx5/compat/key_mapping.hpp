// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
#pragma once
// Legacy NativeAOT/reference mapping, excluded from the default Rust addon.
#include <fcitx-utils/key.h>
#include <fcitx-utils/keysym.h>
#include <utility>
namespace meltype_fcitx {
constexpr int OtherCharacterKey = 0x07;

int specialKey(fcitx::KeySym sym) {
    switch (sym) {
    case FcitxKey_Return: case FcitxKey_KP_Enter: return 0x0D;
    case FcitxKey_Tab: return 0x09;
    case FcitxKey_space: return 0x20;
    case FcitxKey_BackSpace: return 0x08;
    case FcitxKey_Delete: return 0x2E;
    case FcitxKey_Escape: return 0x1B;
    case FcitxKey_Left: return 0x25;
    case FcitxKey_Up: return 0x26;
    case FcitxKey_Right: return 0x27;
    case FcitxKey_Down: return 0x28;
    case FcitxKey_Home: return 0x24;
    case FcitxKey_End: return 0x23;
    case FcitxKey_Page_Up: return 0x21;
    case FcitxKey_Page_Down: return 0x22;
    case FcitxKey_F6: return 0x75;
    case FcitxKey_F7: return 0x76;
    case FcitxKey_F8: return 0x77;
    case FcitxKey_F9: return 0x78;
    case FcitxKey_F10: return 0x79;
    default: return 0;
    }
}

// (仮想キーコード, 入力する文字)。Meltype が扱わないキーなら仮想キーコードは 0。
std::pair<int, uint32_t> virtualKey(fcitx::KeySym sym) {
    if (int vk = specialKey(sym)) return {vk, sym == FcitxKey_space ? 0x20u : 0u};
    uint32_t c = fcitx::Key::keySymToUnicode(sym);
    if (c < 0x20) return {0, 0};
    if (c >= 'a' && c <= 'z') return {static_cast<int>(c - 0x20), c};
    if (c >= 'A' && c <= 'Z') return {static_cast<int>(c), c};
    if (c >= '0' && c <= '9') return {static_cast<int>(c), c};
    switch (c) {
    case ',': return {0xBC, c};
    case '.': return {0xBE, c};
    case '-': return {0xBD, c};
    case '/': return {0xBF, c};
    case '[': return {0xDB, c};
    case ']': return {0xDD, c};
    default: return {OtherCharacterKey, c};
    }
}
}
