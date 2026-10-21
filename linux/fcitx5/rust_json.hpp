// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
#pragma once
#include "json_value.hpp"
#include <cstdint>
#include <limits>

namespace meltype_fcitx {
using JsonEmit = int (*)(void *, uint32_t, double, const uint8_t *, uint32_t);
using JsonVisit = int (*)(const uint8_t *, uint32_t, void *, JsonEmit);

// This adapter receives already parsed events from Rust. It never interprets
// JSON bytes, escapes or numbers, and never unwinds through the Rust callback.
class RustJsonBuilder {
    struct Frame { Json *value; std::string key; bool hasKey = false; };
    Json root_;
    std::vector<Frame> stack_;
    bool hasRoot_ = false;
    Json *append(Json value) {
        if (stack_.empty()) {
            if (hasRoot_) return nullptr;
            hasRoot_ = true;
            root_ = std::move(value);
            return &root_;
        }
        auto &frame = stack_.back();
        if (frame.value->kind == Json::Kind::Array) {
            frame.value->items.push_back(std::move(value));
            return &frame.value->items.back();
        }
        if (frame.value->kind != Json::Kind::Object || !frame.hasKey) return nullptr;
        frame.value->members.emplace_back(std::move(frame.key), std::move(value));
        frame.hasKey = false;
        return &frame.value->members.back().second;
    }
    bool event(uint32_t kind, double number, const uint8_t *text, uint32_t length) {
        if (kind == 5 || kind == 8) {
            if (stack_.empty() || stack_.back().hasKey) return false;
            const auto expected = kind == 5 ? Json::Kind::Array : Json::Kind::Object;
            if (stack_.back().value->kind != expected) return false;
            stack_.pop_back();
            return true;
        }
        if (kind == 7) {
            if (stack_.empty() || stack_.back().value->kind != Json::Kind::Object || stack_.back().hasKey || (!text && length)) return false;
            stack_.back().key = text ? std::string(reinterpret_cast<const char *>(text), length) : std::string();
            stack_.back().hasKey = true;
            return true;
        }
        Json value;
        switch (kind) {
        case 0: break;
        case 1: value.kind = Json::Kind::Bool; value.boolean = number != 0; break;
        case 2: value.kind = Json::Kind::Number; value.number = number; break;
        case 3:
            if (!text && length) return false;
            value.kind = Json::Kind::String;
            if (text) value.string.assign(reinterpret_cast<const char *>(text), length);
            break;
        case 4: value.kind = Json::Kind::Array; break;
        case 6: value.kind = Json::Kind::Object; break;
        default: return false;
        }
        auto *added = append(std::move(value));
        if (!added) return false;
        if (kind == 4 || kind == 6) stack_.push_back({added, {}, false});
        return true;
    }
public:
    static int emit(void *owner, uint32_t kind, double number, const uint8_t *text, uint32_t length) noexcept {
        try { return static_cast<RustJsonBuilder *>(owner)->event(kind, number, text, length) ? 1 : 0; }
        catch (...) { return 0; }
    }
    bool finish(Json &out) {
        if (!hasRoot_ || !stack_.empty()) return false;
        out = std::move(root_);
        return true;
    }
};
inline bool readRustJson(const std::string &text, Json &out, JsonVisit visit) {
    if (!visit || text.size() > std::numeric_limits<uint32_t>::max()) return false;
    RustJsonBuilder builder;
    return visit(reinterpret_cast<const uint8_t *>(text.data()), static_cast<uint32_t>(text.size()), &builder, RustJsonBuilder::emit) != 0
        && builder.finish(out);
}
}
