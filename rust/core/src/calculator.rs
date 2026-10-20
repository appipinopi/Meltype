// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Yukishiro, appipinopi
//! Bounded arithmetic parser; input is never evaluated as code.
pub fn evaluate(expression: &str) -> Option<String> {
    let text = expression
        .trim()
        .trim_end_matches('=')
        .replace('×', "*")
        .replace('÷', "/")
        .replace('＋', "+")
        .replace('－', "-")
        .replace(' ', "");
    if text.is_empty()
        || text.len() > 80
        || !text.bytes().any(|b| b"+-*/".contains(&b))
        || !text.bytes().any(|b| b.is_ascii_digit())
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-*/().".contains(&b))
    {
        return None;
    }
    struct Parser<'a> {
        text: &'a [u8],
        i: usize,
    }
    impl Parser<'_> {
        fn sum(&mut self) -> Option<f64> {
            let mut v = self.product()?;
            while let Some(op @ (b'+' | b'-')) = self.text.get(self.i).copied() {
                self.i += 1;
                let r = self.product()?;
                v = if op == b'+' { v + r } else { v - r };
            }
            Some(v)
        }
        fn product(&mut self) -> Option<f64> {
            let mut v = self.unary()?;
            while let Some(op @ (b'*' | b'/')) = self.text.get(self.i).copied() {
                self.i += 1;
                let r = self.unary()?;
                if op == b'/' && r == 0. {
                    return None;
                }
                v = if op == b'*' { v * r } else { v / r };
            }
            Some(v)
        }
        fn unary(&mut self) -> Option<f64> {
            if self.text.get(self.i) == Some(&b'-') {
                self.i += 1;
                return Some(-self.unary()?);
            }
            if self.text.get(self.i) == Some(&b'(') {
                self.i += 1;
                let v = self.sum()?;
                if self.text.get(self.i) != Some(&b')') {
                    return None;
                }
                self.i += 1;
                return Some(v);
            }
            let start = self.i;
            while self
                .text
                .get(self.i)
                .is_some_and(|b| b.is_ascii_digit() || *b == b'.')
            {
                self.i += 1;
            }
            std::str::from_utf8(&self.text[start..self.i])
                .ok()?
                .parse()
                .ok()
        }
    }
    let mut p = Parser {
        text: text.as_bytes(),
        i: 0,
    };
    let value = p.sum()?;
    if p.i != text.len() || !value.is_finite() || value.abs() >= 1e15 {
        return None;
    }
    let rounded = (value * 1e10).round_ties_even() / 1e10;
    let result = format!("{rounded:.10}");
    let result = result.trim_end_matches('0').trim_end_matches('.');
    Some(result.to_owned())
}
