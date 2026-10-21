// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import AppKit

/// OS normalization stays identical to the former event path. Rust owns the
/// virtual-key/modifier policy. Missing Rust ABI fails without a legacy fallback.
struct NativeKeyPolicy {
    private typealias KeyFunction = @convention(c) (UInt32, UInt32) -> Int32
    private typealias ModifiersFunction = @convention(c) (UInt64) -> Int32
    private let key: KeyFunction
    private let modifiers: ModifiersFunction
    init?(lookup: (String) -> UnsafeMutableRawPointer?) {
        guard let key = lookup("meltype_macos_key"),
              let modifiers = lookup("meltype_macos_modifiers") else { return nil }
        self.key = unsafeBitCast(key, to: KeyFunction.self)
        self.modifiers = unsafeBitCast(modifiers, to: ModifiersFunction.self)
    }
    func virtualKey(for event: NSEvent) -> Int32? {
        let scalar = event.charactersIgnoringModifiers?.lowercased().unicodeScalars.first?.value ?? 0
        let value = key(UInt32(event.keyCode), scalar)
        return value == 0 ? nil : value
    }
    func modifiers(for flags: NSEvent.ModifierFlags) -> Int32 {
        modifiers(UInt64(flags.rawValue))
    }
}
