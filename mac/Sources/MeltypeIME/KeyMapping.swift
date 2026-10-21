// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
import AppKit

enum KeyMapping {
    static func virtualKey(for event: NSEvent) -> Int32? {
#if MELTYPE_RUST_CORE
        return NativeCore.shared.keyPolicy?.virtualKey(for: event)
#else
        return LegacyKeyMapping.virtualKey(for: event)
#endif
    }
    static func modifiers(for flags: NSEvent.ModifierFlags) -> Int32 {
#if MELTYPE_RUST_CORE
        return NativeCore.shared.keyPolicy?.modifiers(for: flags) ?? 0
#else
        var modifiers: Int32 = 0
        if flags.contains(.shift) { modifiers |= 1 }
        if flags.contains(.control) { modifiers |= 2 }
        if flags.contains(.option) { modifiers |= 4 }
        if flags.contains(.command) { modifiers |= 8 }
        return modifiers
#endif
    }
}
