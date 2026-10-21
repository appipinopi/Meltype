// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
#pragma once
#include <string>
#include <algorithm>
namespace meltype_fcitx {
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

}
