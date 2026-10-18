// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Header-only ABI verification for the Rust declarations; never calls Carbon.
#include <Carbon/Carbon.h>
#include <stdint.h>
#define SIGNATURE(name, type) _Static_assert(__builtin_types_compatible_p(__typeof__(&name), type), #name " SDK signature changed")
_Static_assert(sizeof(OSStatus) == 4 && (OSStatus)-1 < 0, "OSStatus must be signed 32 bit");
_Static_assert(sizeof(Boolean) == 1, "TIS include-all argument must be one byte");
_Static_assert(sizeof(CFIndex) == sizeof(intptr_t), "CFIndex must be pointer sized");
SIGNATURE(TISRegisterInputSource, OSStatus (*)(CFURLRef));
SIGNATURE(TISCreateInputSourceList, CFArrayRef (*)(CFDictionaryRef, Boolean));
SIGNATURE(TISGetInputSourceProperty, void *(*)(TISInputSourceRef, CFStringRef));
SIGNATURE(TISEnableInputSource, OSStatus (*)(TISInputSourceRef));
SIGNATURE(CFRelease, void (*)(CFTypeRef));
SIGNATURE(CFArrayGetCount, CFIndex (*)(CFArrayRef));
SIGNATURE(CFArrayGetValueAtIndex, const void *(*)(CFArrayRef, CFIndex));
