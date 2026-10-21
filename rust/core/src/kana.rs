// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
pub fn key(vk: i32, shift: bool) -> Option<char> {
    if shift {
        match vk {
            0x33 => return Some('ぁ'),
            0x34 => return Some('ぅ'),
            0x35 => return Some('ぇ'),
            0x36 => return Some('ぉ'),
            0x37 => return Some('ゃ'),
            0x38 => return Some('ゅ'),
            0x39 => return Some('ょ'),
            0x30 => return Some('を'),
            0x45 => return Some('ぃ'),
            0x5A => return Some('っ'),
            0xDB => return Some('「'),
            0xDD => return Some('」'),
            0xBC => return Some('、'),
            0xBE => return Some('。'),
            0xBF => return Some('・'),
            _ => {}
        }
    }
    match vk {
        0x31 => Some('ぬ'),
        0x32 => Some('ふ'),
        0x33 => Some('あ'),
        0x34 => Some('う'),
        0x35 => Some('え'),
        0x36 => Some('お'),
        0x37 => Some('や'),
        0x38 => Some('ゆ'),
        0x39 => Some('よ'),
        0x30 => Some('わ'),
        0xBD => Some('ほ'),
        0xDE => Some('へ'),
        0xDC => Some('ー'),
        0x51 => Some('た'),
        0x57 => Some('て'),
        0x45 => Some('い'),
        0x52 => Some('す'),
        0x54 => Some('か'),
        0x59 => Some('ん'),
        0x55 => Some('な'),
        0x49 => Some('に'),
        0x4F => Some('ら'),
        0x50 => Some('せ'),
        0xC0 => Some('゛'),
        0xDB => Some('゜'),
        0x41 => Some('ち'),
        0x53 => Some('と'),
        0x44 => Some('し'),
        0x46 => Some('は'),
        0x47 => Some('き'),
        0x48 => Some('く'),
        0x4A => Some('ま'),
        0x4B => Some('の'),
        0x4C => Some('り'),
        0xBB => Some('れ'),
        0xBA => Some('け'),
        0xDD => Some('む'),
        0x5A => Some('つ'),
        0x58 => Some('さ'),
        0x43 => Some('そ'),
        0x56 => Some('ひ'),
        0x42 => Some('こ'),
        0x4E => Some('み'),
        0x4D => Some('も'),
        0xBC => Some('ね'),
        0xBE => Some('る'),
        0xBF => Some('め'),
        0xE2 => Some('ろ'),
        _ => None,
    }
}
pub fn combine(previous: char, mark: char) -> Option<char> {
    let table = match mark {
        '゛' => {
            "かがきぎくぐけげこごさざしじすずせぜそぞただちぢつづてでとどはばひびふぶへべほぼうゔ"
        }
        '゜' => "はぱひぴふぷへぺほぽ",
        _ => return None,
    };
    table
        .chars()
        .collect::<Vec<_>>()
        .as_chunks::<2>()
        .0
        .iter()
        .find(|p| p[0] == previous)
        .map(|p| p[1])
}
pub fn halfwidth(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut out = String::new();
    for c in text.chars() {
        if ('！'..='～').contains(&c) {
            out.push(char::from_u32(c as u32 - 0xfee0).unwrap());
            continue;
        }
        if let Some(half) = (0xff61..=0xff9f)
            .filter_map(char::from_u32)
            .find(|h| h.to_string().nfkc().collect::<String>() == c.to_string())
        {
            out.push(half);
            continue;
        }
        let decomposed: Vec<_> = c.to_string().nfd().collect();
        if decomposed.len() == 2 && ['\u{3099}', '\u{309a}'].contains(&decomposed[1]) {
            out.push_str(&halfwidth(&decomposed[0].to_string()));
            out.push(if decomposed[1] == '\u{3099}' {
                'ﾞ'
            } else {
                'ﾟ'
            });
        } else {
            out.push(c);
        }
    }
    out
}
pub fn fullwidth(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c == ' ' {
                '　'
            } else if c.is_ascii_graphic() {
                char::from_u32(c as u32 + 0xfee0).unwrap()
            } else {
                c
            }
        })
        .collect()
}
