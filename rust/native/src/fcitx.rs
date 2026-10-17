// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! fcitx5 key policy, preserving the former adapter's mapping independently of
//! the SDK. The framework still supplies keySymToUnicode at the ABI boundary.
fn translate(symbol: u32, scalar: u32) -> (u32, u32) {
    let special = match symbol {
        0xff0d | 0xff8d => 13,
        0xff09 => 9,
        0x20 => 32,
        0xff08 => 8,
        0xffff => 0x2e,
        0xff1b => 27,
        0xff51 => 0x25,
        0xff52 => 0x26,
        0xff53 => 0x27,
        0xff54 => 0x28,
        0xff50 => 0x24,
        0xff57 => 0x23,
        0xff55 => 0x21,
        0xff56 => 0x22,
        0xffc3..=0xffc7 => 0x75 + symbol - 0xffc3,
        _ => 0,
    };
    if special != 0 {
        return (special, if special == 32 { 32 } else { 0 });
    }
    let Some(character) = char::from_u32(scalar).filter(|c| *c >= ' ') else {
        return (0, 0);
    };
    let key = match character {
        'a'..='z' => scalar - 32,
        'A'..='Z' | '0'..='9' => scalar,
        ',' => 0xbc,
        '.' => 0xbe,
        '-' => 0xbd,
        '/' => 0xbf,
        '[' => 0xdb,
        ']' => 0xdd,
        _ => 7,
    };
    (key, scalar)
}
/// Low 32 bits: virtual key; high 32 bits: Unicode scalar. Zero means unhandled.
#[no_mangle]
pub extern "C" fn meltype_fcitx_key(symbol: u32, scalar: u32) -> u64 {
    let (key, scalar) = translate(symbol, scalar);
    u64::from(key) | (u64::from(scalar) << 32)
}
/// fcitx KeyState bits: Shift=1, Ctrl=4, Alt=8, Super=64. Preserve the old
/// adapter's Super-only policy (Super2 and lock/repeat/internal bits are ignored).
#[no_mangle]
pub extern "C" fn meltype_fcitx_modifiers(state: u32) -> i32 {
    i32::from(state & 1 != 0)
        | (i32::from(state & 4 != 0) << 1)
        | (i32::from(state & 8 != 0) << 2)
        | (i32::from(state & 64 != 0) << 3)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_preserve_special_scalar_and_punctuation_contracts() {
        for (symbol, key) in [
            (0xff0d, 13),
            (0xff8d, 13),
            (0xff09, 9),
            (0x20, 32),
            (0xff08, 8),
            (0xffff, 46),
            (0xff1b, 27),
            (0xff51, 37),
            (0xff52, 38),
            (0xff53, 39),
            (0xff54, 40),
            (0xff50, 36),
            (0xff57, 35),
            (0xff55, 33),
            (0xff56, 34),
            (0xffc3, 117),
            (0xffc7, 121),
        ] {
            assert_eq!(
                translate(symbol, 'x' as u32),
                (key, if key == 32 { 32 } else { 0 })
            );
        }
        for character in [
            'a', 'Z', '0', ',', '.', '-', '/', '[', ']', '!', 'İ', 'は', '😀', '\u{7f}',
        ] {
            let packed = meltype_fcitx_key(0, character as u32);
            assert_ne!(packed as u32, 0);
            assert_eq!((packed >> 32) as u32, character as u32);
        }
        assert_eq!(translate(0, 'a' as u32), (65, 97));
        assert_eq!(translate(0, 'İ' as u32), (7, 'İ' as u32));
        for invalid in [0, 31, 0xd800, 0x110000] {
            assert_eq!(meltype_fcitx_key(0, invalid), 0);
        }
        assert_eq!(meltype_fcitx_key(0xfe20, 0), 0); // former fcitx policy, unlike IBus
    }
    #[test]
    fn modifier_bits_preserve_all_combinations_without_locks_or_super2() {
        for mask in 0..16u32 {
            let source = (mask & 1) | ((mask & 2) << 1) | ((mask & 4) << 1) | ((mask & 8) << 3);
            assert_eq!(meltype_fcitx_modifiers(source), mask as i32);
            assert_eq!(
                meltype_fcitx_modifiers(source | 2 | 16 | (1 << 26) | (1 << 31)),
                mask as i32
            );
        }
    }
}
