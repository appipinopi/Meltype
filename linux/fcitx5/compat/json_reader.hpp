// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
#pragma once
// Legacy NativeAOT compatibility/reference only; not in the default Rust adapter.
#include "../json_value.hpp"
#include <cstdlib>
#include <cstdint>
namespace meltype_fcitx {
class JsonReader {
public:
    explicit JsonReader(const std::string &text) : s_(text) {}

    bool read(Json &out) {
        bool ok = value(out);
        space();
        return ok && i_ == s_.size();
    }

private:
    const std::string &s_;
    size_t i_ = 0;

    void space() {
        while (i_ < s_.size() && (s_[i_] == ' ' || s_[i_] == '\n' || s_[i_] == '\r' || s_[i_] == '\t')) i_++;
    }
    bool literal(const char *word) {
        size_t n = std::char_traits<char>::length(word);
        if (s_.compare(i_, n, word) != 0) return false;
        i_ += n;
        return true;
    }
    static void appendUtf8(std::string &out, uint32_t cp) {
        if (cp < 0x80) out += static_cast<char>(cp);
        else if (cp < 0x800) { out += static_cast<char>(0xC0 | (cp >> 6)); out += static_cast<char>(0x80 | (cp & 0x3F)); }
        else if (cp < 0x10000) {
            out += static_cast<char>(0xE0 | (cp >> 12));
            out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
            out += static_cast<char>(0x80 | (cp & 0x3F));
        } else {
            out += static_cast<char>(0xF0 | (cp >> 18));
            out += static_cast<char>(0x80 | ((cp >> 12) & 0x3F));
            out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
            out += static_cast<char>(0x80 | (cp & 0x3F));
        }
    }
    bool hex4(uint32_t &cp) {
        if (i_ + 4 > s_.size()) return false;
        cp = static_cast<uint32_t>(std::strtoul(s_.substr(i_, 4).c_str(), nullptr, 16));
        i_ += 4;
        return true;
    }
    bool stringValue(std::string &out) {
        if (s_[i_] != '"') return false;
        i_++;
        while (i_ < s_.size() && s_[i_] != '"') {
            char c = s_[i_++];
            if (c != '\\') { out += c; continue; }
            if (i_ >= s_.size()) return false;
            char e = s_[i_++];
            switch (e) {
            case 'n': out += '\n'; break;
            case 'r': out += '\r'; break;
            case 't': out += '\t'; break;
            case 'b': out += '\b'; break;
            case 'f': out += '\f'; break;
            case 'u': {
                uint32_t cp;
                if (!hex4(cp)) return false;
                // サロゲートペア (絵文字)
                if (cp >= 0xD800 && cp <= 0xDBFF && s_.compare(i_, 2, "\\u") == 0) {
                    i_ += 2;
                    uint32_t low;
                    if (!hex4(low)) return false;
                    cp = 0x10000 + ((cp - 0xD800) << 10) + (low - 0xDC00);
                }
                appendUtf8(out, cp);
                break;
            }
            default: out += e; break;
            }
        }
        if (i_ >= s_.size()) return false;
        i_++;
        return true;
    }
    bool value(Json &out) {
        space();
        if (i_ >= s_.size()) return false;
        char c = s_[i_];
        if (c == '{') {
            out.kind = Json::Kind::Object;
            i_++;
            space();
            if (i_ < s_.size() && s_[i_] == '}') { i_++; return true; }
            while (true) {
                space();
                std::string key;
                if (i_ >= s_.size() || !stringValue(key)) return false;
                space();
                if (i_ >= s_.size() || s_[i_++] != ':') return false;
                Json member;
                if (!value(member)) return false;
                out.members.emplace_back(std::move(key), std::move(member));
                space();
                if (i_ < s_.size() && s_[i_] == ',') { i_++; continue; }
                if (i_ < s_.size() && s_[i_] == '}') { i_++; return true; }
                return false;
            }
        }
        if (c == '[') {
            out.kind = Json::Kind::Array;
            i_++;
            space();
            if (i_ < s_.size() && s_[i_] == ']') { i_++; return true; }
            while (true) {
                Json item;
                if (!value(item)) return false;
                out.items.push_back(std::move(item));
                space();
                if (i_ < s_.size() && s_[i_] == ',') { i_++; continue; }
                if (i_ < s_.size() && s_[i_] == ']') { i_++; return true; }
                return false;
            }
        }
        if (c == '"') { out.kind = Json::Kind::String; return stringValue(out.string); }
        if (literal("true")) { out.kind = Json::Kind::Bool; out.boolean = true; return true; }
        if (literal("false")) { out.kind = Json::Kind::Bool; out.boolean = false; return true; }
        if (literal("null")) { out.kind = Json::Kind::Null; return true; }
        char *end = nullptr;
        out.number = std::strtod(s_.c_str() + i_, &end);
        if (end == s_.c_str() + i_) return false;
        out.kind = Json::Kind::Number;
        i_ = static_cast<size_t>(end - s_.c_str());
        return true;
    }
};
}
