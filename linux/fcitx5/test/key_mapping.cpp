// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Compare Rust ABI against the original mapping using the installed fcitx SDK.
#include "../compat/key_mapping.hpp"
#include <dlfcn.h>
#include <iostream>
#include <stdexcept>
int main(int argc,char **argv) {
    try {
        if(argc!=2)throw std::runtime_error("Rust library required");
        void *library=dlopen(argv[1],RTLD_NOW|RTLD_LOCAL);
        if(!library)throw std::runtime_error("Rust library load failed");
        auto key=reinterpret_cast<uint64_t (*)(uint32_t,uint32_t)>(dlsym(library,"meltype_fcitx_key"));
        auto modifiers=reinterpret_cast<int (*)(uint32_t)>(dlsym(library,"meltype_fcitx_modifiers"));
        auto mode=reinterpret_cast<uint32_t (*)(uint32_t,int)>(dlsym(library,"meltype_fcitx_mode"));
        if(!key||!modifiers||!mode)throw std::runtime_error("Rust key/mode ABI missing");
        size_t count=0;
        auto compare=[&](uint32_t symbol) {
            const auto expected=meltype_fcitx::virtualKey(static_cast<fcitx::KeySym>(symbol));
            const auto actual=key(symbol,fcitx::Key::keySymToUnicode(static_cast<fcitx::KeySym>(symbol)));
            if(static_cast<int>(actual&0xffffffffu)!=expected.first || static_cast<uint32_t>(actual>>32)!=expected.second)
                throw std::runtime_error("key mapping mismatch: "+std::to_string(symbol));
            for(int direct=0;direct<2;direct++) {
                if(mode(symbol,direct)!=meltype_fcitx::modePlan(static_cast<fcitx::KeySym>(symbol),direct!=0))
                    throw std::runtime_error("mode/role mapping mismatch: "+std::to_string(symbol));
            }
            count++;
        };
        for(uint32_t symbol=0;symbol<0x20000;symbol++)compare(symbol);
        for(uint32_t scalar:{0x304bU,0x130U,0x1f600U,0x10ffffU})compare(0x01000000U|scalar);
        for(uint32_t mask=0;mask<256;mask++) {
            fcitx::KeyStates states(mask);
            const int expected=(states.test(fcitx::KeyState::Shift)?1:0)|(states.test(fcitx::KeyState::Ctrl)?2:0)
                |(states.test(fcitx::KeyState::Alt)?4:0)|(states.test(fcitx::KeyState::Super)?8:0);
            if(modifiers(states.toInteger())!=expected || modifiers(states.toInteger()|(1U<<26)|(1U<<31))!=expected)
                throw std::runtime_error("modifier mapping mismatch");
        }
        dlclose(library);
        std::cout<<"fcitx Rust keys: "<<count<<" SDK/reference symbols, "<<(count*2)<<" mode/role states and 512 modifier cases passed\n";
    } catch(const std::exception &error){std::cerr<<error.what()<<'\n';return 1;}
}
