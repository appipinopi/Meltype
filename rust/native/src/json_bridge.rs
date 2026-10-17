// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 lnkiai, appipinopi
//! TSF JSON compatibility, including raw UTF-16 escapes and the upstream depth
//! limit/number conversion. The C++ owner receives values, never parses bytes.
use std::{
    collections::BTreeMap,
    ffi::{c_char, c_void, CString},
    panic::{catch_unwind, AssertUnwindSafe},
    slice,
};
#[derive(Debug, PartialEq)]
enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(Vec<u16>),
    Array(Vec<Value>),
    Object(BTreeMap<Vec<u16>, Value>),
}
fn decode_utf8(raw: &[u8]) -> Vec<u16> {
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::{
            MultiByteToWideChar, CP_UTF8, MULTI_BYTE_TO_WIDE_CHAR_FLAGS,
        };
        if raw.is_empty() || raw.len() > i32::MAX as usize {
            return Vec::new();
        }
        // Match the original Windows replacement of malformed UTF8 sequences;
        // Rust's lossy decoder can emit a different number of replacement units.
        unsafe {
            let count = MultiByteToWideChar(CP_UTF8, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), raw, None);
            if count <= 0 {
                return Vec::new();
            }
            let mut result = vec![0; count as usize];
            MultiByteToWideChar(
                CP_UTF8,
                MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0),
                raw,
                Some(&mut result),
            );
            result
        }
    }
    #[cfg(not(windows))]
    {
        String::from_utf8_lossy(raw).encode_utf16().collect()
    }
}
struct Parser<'a> {
    input: &'a [u8],
    index: usize,
}
impl Parser<'_> {
    fn skip(&mut self) {
        while self
            .input
            .get(self.index)
            .is_some_and(|c| b" \t\r\n".contains(c))
        {
            self.index += 1;
        }
    }
    fn take(&mut self, word: &[u8]) -> bool {
        if self.input[self.index..].starts_with(word) {
            self.index += word.len();
            true
        } else {
            false
        }
    }
    fn value(&mut self, depth: usize) -> Option<Value> {
        if depth > 32 {
            return None;
        }
        self.skip();
        match *self.input.get(self.index)? {
            b'"' => Some(Value::String(self.string()?)),
            b'[' => {
                self.index += 1;
                self.skip();
                let mut values = Vec::new();
                if self.take(b"]") {
                    return Some(Value::Array(values));
                }
                loop {
                    values.push(self.value(depth + 1)?);
                    self.skip();
                    if self.take(b"]") {
                        return Some(Value::Array(values));
                    }
                    if !self.take(b",") {
                        return None;
                    }
                }
            }
            b'{' => {
                self.index += 1;
                self.skip();
                let mut values = BTreeMap::new();
                if self.take(b"}") {
                    return Some(Value::Object(values));
                }
                loop {
                    self.skip();
                    let key = self.string()?;
                    self.skip();
                    if !self.take(b":") {
                        return None;
                    }
                    values.insert(key, self.value(depth + 1)?);
                    self.skip();
                    if self.take(b"}") {
                        return Some(Value::Object(values));
                    }
                    if !self.take(b",") {
                        return None;
                    }
                }
            }
            _ => {
                if self.take(b"true") {
                    return Some(Value::Bool(true));
                }
                if self.take(b"false") {
                    return Some(Value::Bool(false));
                }
                if self.take(b"null") {
                    return Some(Value::Null);
                }
                let start = self.index;
                while self
                    .input
                    .get(self.index)
                    .is_some_and(|c| *c == 0 || b"+-0123456789.eE".contains(c))
                {
                    self.index += 1;
                }
                if start == self.index {
                    return None;
                }
                // strchr also matched its terminating NUL in the old scanner;
                // strtod then read only the prefix before that NUL.
                let prefix = self.input[start..self.index]
                    .split(|byte| *byte == 0)
                    .next()?;
                let number = CString::new(prefix).ok()?;
                // Preserve the original TSF parser's strtod conversion, including
                // its permissive numeric tokens. Core protocol producers emit
                // ordinary JSON numbers; this boundary also serves old shells.
                Some(Value::Number(unsafe {
                    libc::strtod(number.as_ptr(), std::ptr::null_mut())
                }))
            }
        }
    }
    fn string(&mut self) -> Option<Vec<u16>> {
        if !self.take(b"\"") {
            return None;
        }
        let mut out = Vec::new();
        let mut raw = Vec::new();
        loop {
            let byte = *self.input.get(self.index)?;
            self.index += 1;
            match byte {
                b'"' => {
                    out.extend(decode_utf8(&raw));
                    return Some(out);
                }
                b'\\' => {
                    let escape = *self.input.get(self.index)?;
                    self.index += 1;
                    match escape {
                        b'"' | b'\\' | b'/' => raw.push(escape),
                        b'b' => raw.push(8),
                        b'f' => raw.push(12),
                        b'n' => raw.push(10),
                        b'r' => raw.push(13),
                        b't' => raw.push(9),
                        b'u' => {
                            let mut code = 0u16;
                            for _ in 0..4 {
                                let byte = *self.input.get(self.index)?;
                                self.index += 1;
                                let digit = (byte as char).to_digit(16)?;
                                code = code * 16 + digit as u16;
                            }
                            out.extend(decode_utf8(&raw));
                            raw.clear();
                            out.push(code);
                        }
                        _ => return None,
                    }
                }
                _ => raw.push(byte),
            }
        }
    }
}
fn parse(input: &[u8]) -> Option<Value> {
    let mut parser = Parser { input, index: 0 };
    let result = parser.value(1)?;
    parser.skip();
    (parser.index == input.len()).then_some(result)
}
pub type Emit = unsafe extern "C" fn(*mut c_void, u32, f64, *const u16, u32) -> i32;
unsafe fn emit(value: &Value, owner: *mut c_void, callback: Emit) -> bool {
    let empty = std::ptr::null();
    match value {
        Value::Null => callback(owner, 0, 0.0, empty, 0) != 0,
        Value::Bool(value) => callback(owner, 1, if *value { 1.0 } else { 0.0 }, empty, 0) != 0,
        Value::Number(value) => callback(owner, 2, *value, empty, 0) != 0,
        Value::String(value) => callback(owner, 3, 0.0, value.as_ptr(), value.len() as u32) != 0,
        Value::Array(values) => {
            callback(owner, 4, 0.0, empty, 0) != 0
                && values.iter().all(|value| emit(value, owner, callback))
                && callback(owner, 5, 0.0, empty, 0) != 0
        }
        Value::Object(values) => {
            callback(owner, 6, 0.0, empty, 0) != 0
                && values.iter().all(|(key, value)| {
                    callback(owner, 7, 0.0, key.as_ptr(), key.len() as u32) != 0
                        && emit(value, owner, callback)
                })
                && callback(owner, 8, 0.0, empty, 0) != 0
        }
    }
}
/// # Safety
/// input is readable for length bytes. callback/code/owner remain valid for this
/// synchronous call and never throw. Events: 0 null,1 bool(number),2 number,
/// 3 UTF16 string,4/5 array begin/end,6/7/8 object begin/key/end. Text is borrowed
/// for the callback only. Zero callback result stops traversal. Invalid JSON
/// emits no events. Returns 1 success,0 parse failure/cancel/invalid arguments.
#[no_mangle]
pub unsafe extern "C" fn meltype_json_visit(
    input: *const u8,
    length: u32,
    owner: *mut c_void,
    callback: Option<Emit>,
) -> i32 {
    catch_unwind(AssertUnwindSafe(|| {
        if input.is_null() || callback.is_none() {
            return 0;
        }
        let Some(value) = parse(slice::from_raw_parts(input, length as usize)) else {
            return 0;
        };
        i32::from(emit(&value, owner, callback.unwrap()))
    }))
    .unwrap_or(0)
}
fn quote(text: &[u16]) -> String {
    let mut out = String::from("\"");
    for ch in String::from_utf16_lossy(text).chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => {
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}
/// # Safety
/// text is readable for length UTF16 units (null permitted only for zero length).
/// Returns UTF8 quoted JSON allocated for meltype_free, or null on invalid input.
#[no_mangle]
pub unsafe extern "C" fn meltype_json_quote(text: *const u16, length: u32) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        if text.is_null() && length != 0 {
            return std::ptr::null_mut();
        }
        let text = if length == 0 {
            &[]
        } else {
            slice::from_raw_parts(text, length as usize)
        };
        crate::allocate(&quote(text))
    }))
    .unwrap_or(std::ptr::null_mut())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn utf16_escapes_keep_unpaired_units_and_nuls() {
        assert_eq!(
            parse(br#""a\ud800\u0000\udc00\ud83d\ude00""#),
            Some(Value::String(vec![97, 0xd800, 0, 0xdc00, 0xd83d, 0xde00]))
        );
    }
    #[test]
    fn depth_limit_matches_empty_and_scalar_boundaries() {
        let root = format!("{}0{}", "[".repeat(31), "]".repeat(31));
        assert!(parse(root.as_bytes()).is_some());
        let deep = format!("{}0{}", "[".repeat(32), "]".repeat(32));
        assert!(parse(deep.as_bytes()).is_none());
        assert!(parse(format!("{}{}", "[".repeat(32), "]".repeat(32)).as_bytes()).is_some());
    }
    #[test]
    fn duplicate_keys_replace_and_invalid_tail_is_rejected() {
        assert_eq!(parse(b"1\0e5"), Some(Value::Number(1.0)));
        assert_eq!(
            parse(br#"{"a":true,"a":false}"#),
            Some(Value::Object(BTreeMap::from([(
                vec![97],
                Value::Bool(false)
            )])))
        );
        assert!(parse(b"{} trailing").is_none());
        assert!(parse(br#"[1,]"#).is_none());
    }
    #[test]
    fn quote_preserves_upstream_control_escapes() {
        assert_eq!(
            quote(&[0, 8, 12, 9, 10, 13, 34, 92, 0xd800]),
            "\"\\u0000\\u0008\\u000c\\t\\n\\r\\\"\\\\�\""
        );
    }
}
