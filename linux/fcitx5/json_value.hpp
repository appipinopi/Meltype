// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
#pragma once
#include <string>
#include <vector>
#include <utility>
namespace meltype_fcitx {
struct Json {
    enum class Kind { Null, Bool, Number, String, Array, Object } kind = Kind::Null;
    bool boolean = false;
    double number = 0;
    std::string string;
    std::vector<Json> items;
    std::vector<std::pair<std::string, Json>> members;

    const Json *get(const std::string &name) const {
        for (const auto &[key, value] : members)
            if (key == name) return &value;
        return nullptr;
    }
    std::string str(const std::string &name) const {
        auto *v = get(name);
        return v && v->kind == Kind::String ? v->string : std::string();
    }
    int integer(const std::string &name, int fallback = 0) const {
        auto *v = get(name);
        return v && v->kind == Kind::Number ? static_cast<int>(v->number) : fallback;
    }
    bool flag(const std::string &name) const {
        auto *v = get(name);
        return v && v->kind == Kind::Bool && v->boolean;
    }
    std::vector<std::string> strings(const std::string &name) const {
        std::vector<std::string> result;
        if (auto *v = get(name); v && v->kind == Kind::Array)
            for (const auto &item : v->items)
                if (item.kind == Kind::String) result.push_back(item.string);
        return result;
    }
};
}
