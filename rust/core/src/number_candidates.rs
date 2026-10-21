// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Number/era candidates from Meltype 1.3.0, independent of OS dictionaries.
pub fn half_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap(),
            '．' => '.',
            _ => c,
        })
        .collect()
}
pub fn to_era(year: u32) -> Option<String> {
    if year > 2100 {
        return None;
    }
    for (name, start) in [
        ("令和", 2019),
        ("平成", 1989),
        ("昭和", 1926),
        ("大正", 1912),
        ("明治", 1868),
    ] {
        if year >= start {
            let n = year - start + 1;
            return Some(format!(
                "{name}{}年",
                if n == 1 { "元".into() } else { n.to_string() }
            ));
        }
    }
    None
}
pub fn candidates(text: &str) -> Vec<String> {
    let half = half_digits(&text.replace(',', ""));
    if half.is_empty() || half.len() > 16 || !half.bytes().all(|b| b.is_ascii_digit()) {
        return vec![];
    }
    const DIGITS: [char; 10] = ['〇', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    const FORMAL: [char; 10] = ['零', '壱', '弐', '参', '四', '五', '六', '七', '八', '九'];
    let mut result = vec![half.bytes().map(|b| DIGITS[(b - b'0') as usize]).collect()];
    if half.len() > 1 && half.starts_with('0') {
        return result;
    }
    let value: u64 = half.parse().unwrap();
    fn render(mut v: u64, digits: &[char; 10], small: [&str; 4], omit_one: bool) -> String {
        let mut groups = vec![];
        while v > 0 {
            groups.push(v % 10000);
            v /= 10000;
        }
        let mut result = String::new();
        for i in (0..groups.len()).rev() {
            if groups[i] == 0 {
                continue;
            }
            for place in (0..4u32).rev() {
                let n = (groups[i] / 10u64.pow(place)) % 10;
                if n == 0 {
                    continue;
                }
                if !(n == 1 && place > 0 && omit_one) {
                    result.push(digits[n as usize]);
                }
                result.push_str(small[place as usize]);
            }
            result.push_str(["", "万", "億", "兆"][i]);
        }
        result
    }
    if value > 0 {
        result.push(render(value, &DIGITS, ["", "十", "百", "千"], true));
        result.push(render(value, &FORMAL, ["", "拾", "百", "千"], false));
    }
    if half.len() == 4 {
        if let Some(era) = to_era(value as u32) {
            result.push(era);
        }
    }
    let mut seen = std::collections::HashSet::new();
    result.retain(|s| seen.insert(s.clone()));
    result
}
pub fn era_to_year(reading: &str) -> Option<Vec<String>> {
    let text = half_digits(
        reading
            .strip_suffix('\n')
            .unwrap_or(reading)
            .strip_suffix("ねん")?,
    );
    for (era, start, last) in [
        ("れいわ", 2019, 2100),
        ("へいせい", 1989, 2019),
        ("しょうわ", 1926, 1989),
        ("たいしょう", 1912, 1926),
        ("めいじ", 1868, 1912),
    ] {
        if let Some(n) = text.strip_prefix(era) {
            let n = if n == "がん" {
                1
            } else {
                if n.is_empty() || n.len() > 2 || !n.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                n.parse::<u32>().ok()?
            };
            let year = start + n.saturating_sub(1);
            return (n > 0 && year <= last).then(|| vec![format!("{year}年"), year.to_string()]);
        }
    }
    None
}
pub fn traditional_units(reading: &str) -> Option<Vec<String>> {
    let text = half_digits(reading.strip_suffix('\n').unwrap_or(reading));
    for (reading, unit, size, metric) in [
        ("しゃく", "尺", 100. / 3.3, "cm"),
        ("すん", "寸", 10. / 3.3, "cm"),
        ("けん", "間", 200. / 110., "m"),
        ("じょう", "丈", 100. / 33., "m"),
        ("つぼ", "坪", 400. / 121., "㎡"),
        ("ごう", "合", 180.39, "mL"),
        ("しょう", "升", 1.8039, "L"),
        ("と", "斗", 18.039, "L"),
        ("かん", "貫", 3.75, "kg"),
        ("もんめ", "匁", 3.75, "g"),
        ("きん", "斤", 600., "g"),
        ("りょう", "両", 37.5, "g"),
    ] {
        if let Some(n) = text.strip_suffix(reading) {
            let (whole, fraction) = n.split_once('.').map_or((n, None), |(a, b)| (a, Some(b)));
            if whole.is_empty()
                || whole.len() > 6
                || !whole.bytes().all(|b| b.is_ascii_digit())
                || fraction.is_some_and(|f| {
                    f.is_empty() || f.len() > 3 || !f.bytes().all(|b| b.is_ascii_digit())
                })
            {
                return None;
            }
            let value = n.parse::<f64>().ok()? * size;
            let value = if value >= 100. {
                format!("{:.0}", value.round_ties_even())
            } else {
                format!("{:.2}", (value * 100.).round_ties_even() / 100.)
                    .trim_end_matches('0')
                    .trim_end_matches('.')
                    .to_owned()
            };
            return Some(vec![format!("{n}{unit}"), format!("約{value}{metric}")]);
        }
    }
    None
}
pub fn seasonal(month: u32) -> Vec<String> {
    const TERMS: [[&str; 4]; 12] = [
        ["新春", "初春", "厳寒", "大寒"],
        ["余寒", "立春", "春寒", "梅花"],
        ["早春", "春分", "春暖", "浅春"],
        ["桜花", "陽春", "春暖", "晩春"],
        ["新緑", "薫風", "若葉", "立夏"],
        ["入梅", "梅雨", "向暑", "初夏"],
        ["盛夏", "猛暑", "大暑", "酷暑"],
        ["晩夏", "残暑", "立秋", "暮夏"],
        ["初秋", "新秋", "秋晴", "野分"],
        ["秋冷", "紅葉", "秋涼", "寒露"],
        ["晩秋", "霜降", "向寒", "初霜"],
        ["初冬", "師走", "寒冷", "歳晩"],
    ];
    if !(1..=12).contains(&month) {
        return vec![];
    }
    let mut result = vec![];
    for m in [month, (month + 10) % 12 + 1, month % 12 + 1] {
        for term in TERMS[(m - 1) as usize] {
            let word = format!("{term}の候");
            if !result.contains(&word) {
                result.push(word);
            }
        }
    }
    result.push(format!("拝啓　{}、", result[0]));
    result
}
pub fn clipboard_line(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty() && text.encode_utf16().count() <= 200 && !text.chars().any(char::is_control))
        .then(|| text.to_owned())
}
