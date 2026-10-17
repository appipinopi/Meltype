// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Carbon keycode policy. The SDK supplies the first OS-normalized scalar.
#[no_mangle]
pub extern "C" fn meltype_macos_key(code: u32, scalar: u32) -> i32 {
    if code > u16::MAX as u32 {
        return 0;
    }
    match code {
        36 | 76 => 0x0d,
        48 => 9,
        49 => 32,
        51 => 8,
        117 => 0x2e,
        53 => 0x1b,
        123 => 0x25,
        126 => 0x26,
        124 => 0x27,
        125 => 0x28,
        115 => 0x24,
        119 => 0x23,
        116 => 0x21,
        121 => 0x22,
        97 => 0x75,
        98 => 0x76,
        100 => 0x77,
        101 => 0x78,
        109 => 0x79,
        _ => match scalar {
            0x61..=0x7a => (scalar - 0x20) as i32,
            0x30..=0x39 => scalar as i32,
            0x2c => 0xbc,
            0x2e => 0xbe,
            0x2d => 0xbd,
            0x2f => 0xbf,
            0x5b => 0xdb,
            0x5d => 0xdd,
            0x20..=0x10ffff if char::from_u32(scalar).is_some() => 7,
            _ => 0,
        },
    }
}
#[no_mangle]
pub extern "C" fn meltype_macos_modifiers(flags: u64) -> i32 {
    ((flags >> 17) & 15) as i32
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_boundaries_and_carbon_priority() {
        assert_eq!(meltype_macos_key(36, 0), 13);
        assert_eq!(meltype_macos_key(0, 'a' as u32), 65);
        assert_eq!(meltype_macos_key(0, '😀' as u32), 7);
        for scalar in [0, 31, 0xd800, 0xdfff, 0x110000] {
            assert_eq!(meltype_macos_key(0, scalar), 0);
        }
        assert_eq!(meltype_macos_key(65536, 97), 0);
        assert_eq!(meltype_macos_modifiers(u64::MAX), 15);
        assert_eq!(
            meltype_macos_modifiers((1 << 16) | (1 << 21) | (1 << 23)),
            0
        );
    }
}
