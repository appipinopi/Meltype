// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
use super::*;
use crate::android_editor::{self, Editor, EditorError};
type Result<T> = std::result::Result<T, EditorError>;
struct Connection<'a, 'b, 'c> {
    env: &'a mut JNIEnv<'b>,
    object: &'a JObject<'c>,
}
impl Connection<'_, '_, '_> {
    // Read charAt rather than converting a Java string through UTF-8: a lone
    // surrogate must not compare equal to a replacement character.
    fn units(&mut self, text: JObject<'_>) -> Result<Option<Vec<u16>>> {
        if text.is_null() {
            return Ok(None);
        }
        let result = self
            .env
            .with_local_frame(8, |env| {
                let size = env.call_method(&text, "length", "()I", &[])?.i()?;
                let mut units = Vec::with_capacity(size.max(0) as usize);
                for index in 0..size {
                    units.push(
                        env.call_method(&text, "charAt", "(I)C", &[JValue::Int(index)])?
                            .c()?,
                    );
                }
                Ok::<_, jni::errors::Error>(Some(units))
            })
            .map_err(|_| EditorError);
        let _ = self.env.delete_local_ref(text);
        result
    }
    fn boolean(&mut self, name: &str, signature: &str, args: &[JValue<'_, '_>]) -> Result<bool> {
        self.env
            .call_method(self.object, name, signature, args)
            .and_then(|v| v.z())
            .map_err(|_| EditorError)
    }
    fn text(&mut self, name: &str, text: &str) -> Result<bool> {
        let string = self.env.new_string(text).map_err(|_| EditorError)?;
        let result = self.boolean(
            name,
            "(Ljava/lang/CharSequence;I)Z",
            &[JValue::Object(&string), JValue::Int(1)],
        );
        let _ = self.env.delete_local_ref(string);
        result
    }
    fn clear_exception(&mut self) {
        if self.env.exception_check().unwrap_or(false) {
            let _ = self.env.exception_clear();
        }
    }
}
impl Editor for Connection<'_, '_, '_> {
    fn selected(&mut self) -> Result<Option<Vec<u16>>> {
        let text = self
            .env
            .call_method(
                self.object,
                "getSelectedText",
                "(I)Ljava/lang/CharSequence;",
                &[JValue::Int(0)],
            )
            .and_then(|v| v.l())
            .map_err(|_| EditorError)?;
        self.units(text)
    }
    fn before(&mut self, count: i32) -> Result<Option<Vec<u16>>> {
        let text = self
            .env
            .call_method(
                self.object,
                "getTextBeforeCursor",
                "(II)Ljava/lang/CharSequence;",
                &[JValue::Int(count), JValue::Int(0)],
            )
            .and_then(|v| v.l())
            .map_err(|_| EditorError)?;
        self.units(text)
    }
    fn begin(&mut self) -> Result<()> {
        // Android permits false for a valid non-nesting implementation.
        self.boolean("beginBatchEdit", "()Z", &[]).map(|_| ())
    }
    fn end(&mut self) -> Result<()> {
        // An editor exception must not prevent balancing the batch.
        self.clear_exception();
        let result = self.boolean("endBatchEdit", "()Z", &[]).map(|_| ());
        self.clear_exception();
        result
    }
    fn composing(&mut self, text: &str) -> Result<bool> {
        self.text("setComposingText", text)
    }
    fn delete(&mut self, count: i32) -> Result<bool> {
        self.boolean(
            "deleteSurroundingText",
            "(II)Z",
            &[JValue::Int(count), JValue::Int(0)],
        )
    }
    fn commit(&mut self, text: &str) -> Result<bool> {
        self.text("commitText", text)
    }
    fn finish(&mut self) -> Result<bool> {
        self.clear_exception();
        self.boolean("finishComposingText", "()Z", &[])
    }
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_applyEditor(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    connection: JObject,
    json: JString,
    pending: JString,
) -> jstring {
    let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
        if connection.is_null() {
            return android_editor::Outcome::failure(false);
        }
        let Ok(json) = env.get_string(&json).map(String::from) else {
            return android_editor::Outcome::failure(false);
        };
        let mut editor = Connection {
            env: &mut env,
            object: &connection,
        };
        let pending = match editor.units(pending.into()) {
            Ok(Some(value)) => value,
            _ => {
                editor.clear_exception();
                return android_editor::Outcome::failure(false);
            }
        };
        let outcome = android_editor::apply(&mut editor, &json, &pending);
        editor.clear_exception();
        outcome
    }))
    .unwrap_or_else(|_| android_editor::Outcome::failure(false));
    if outcome.reset {
        unsafe {
            meltype_free(meltype_reset(handle as *mut c_void).cast());
        }
    }
    let text = serde_json::to_string(&outcome).unwrap_or_default();
    env.new_string(text)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
