// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Android widget construction through JNI. Preserve the existing keyboard
//! hierarchy/default theme, labels, row order and equally weighted buttons.
use super::*;
type Result<T> = jni::errors::Result<T>;
#[derive(serde::Deserialize)]
struct Render {
    consumed: bool,
    #[serde(rename = "composingText")]
    text: String,
    view: Option<Choices>,
}
#[derive(serde::Deserialize)]
struct Choices {
    candidates: Vec<String>,
    #[serde(default)]
    predictions: Option<Vec<String>>,
}
fn widget<'a>(env: &mut JNIEnv<'a>, class: &str, context: &JObject) -> Result<JObject<'a>> {
    env.new_object(
        class,
        "(Landroid/content/Context;)V",
        &[JValue::Object(context)],
    )
}
fn add(env: &mut JNIEnv, parent: &JObject, child: &JObject) -> Result<()> {
    env.call_method(
        parent,
        "addView",
        "(Landroid/view/View;)V",
        &[JValue::Object(child)],
    )?;
    Ok(())
}
fn button(
    env: &mut JNIEnv,
    context: &JObject,
    row: &JObject,
    label: &str,
    action: i32,
    value: i32,
) -> Result<()> {
    env.with_local_frame(16, |env| {
        let button = widget(env, "android/widget/Button", context)?;
        let label = env.new_string(label)?;
        env.call_method(
            &button,
            "setText",
            "(Ljava/lang/CharSequence;)V",
            &[JValue::Object(&label)],
        )?;
        for name in ["setMinWidth", "setMinimumWidth"] {
            env.call_method(&button, name, "(I)V", &[JValue::Int(0)])?;
        }
        env.call_method(
            &button,
            "setPadding",
            "(IIII)V",
            &[
                JValue::Int(1),
                JValue::Int(1),
                JValue::Int(1),
                JValue::Int(1),
            ],
        )?;
        let callback = env
            .call_method(
                context,
                "keyboardCallback",
                "(II)Landroid/view/View$OnClickListener;",
                &[JValue::Int(action), JValue::Int(value)],
            )?
            .l()?;
        env.call_method(
            &button,
            "setOnClickListener",
            "(Landroid/view/View$OnClickListener;)V",
            &[JValue::Object(&callback)],
        )?;
        let params = env.new_object(
            "android/widget/LinearLayout$LayoutParams",
            "(IIF)V",
            &[JValue::Int(0), JValue::Int(-2), JValue::Float(1.)],
        )?;
        env.call_method(
            row,
            "addView",
            "(Landroid/view/View;Landroid/view/ViewGroup$LayoutParams;)V",
            &[JValue::Object(&button), JValue::Object(&params)],
        )?;
        Ok(())
    })
}
fn build<'a>(env: &mut JNIEnv<'a>, service: &JObject) -> Result<JObject<'a>> {
    let root = widget(env, "android/widget/LinearLayout", service)?;
    env.call_method(&root, "setOrientation", "(I)V", &[JValue::Int(1)])?;
    let preview = widget(env, "android/widget/TextView", service)?;
    env.call_method(&preview, "setTextSize", "(F)V", &[JValue::Float(20.)])?;
    add(env, &root, &preview)?;
    let scroll = widget(env, "android/widget/HorizontalScrollView", service)?;
    let candidates = widget(env, "android/widget/LinearLayout", service)?;
    add(env, &scroll, &candidates)?;
    add(env, &root, &scroll)?;
    for letters in [
        "qwertyuiop",
        "asdfghjkl",
        "zxcvbnm",
        "1234567890",
        "-.,!?@/",
    ] {
        let row = widget(env, "android/widget/LinearLayout", service)?;
        for letter in letters.chars() {
            button(env, service, &row, &letter.to_string(), 0, letter as i32)?;
        }
        add(env, &root, &row)?;
    }
    let actions = widget(env, "android/widget/LinearLayout", service)?;
    for (label, action, value) in [
        ("⇧", 1, 0),
        ("あ/A", 2, 0),
        ("⌫", 3, 8),
        ("変換/空白", 3, 32),
        ("確定/↵", 3, 13),
        ("取消", 3, 27),
        ("🌐", 4, 0),
    ] {
        button(env, service, &actions, label, action, value)?;
    }
    add(env, &root, &actions)?;
    let tools = widget(env, "android/widget/LinearLayout", service)?;
    for (label, action, value) in [
        ("←", 3, 0x25),
        ("→", 3, 0x27),
        ("縮める", 5, 0),
        ("伸ばす", 6, 0),
        ("戻す", 7, 0),
        ("再変換", 8, 0),
        ("設定", 9, 0),
    ] {
        button(env, service, &tools, label, action, value)?;
    }
    add(env, &root, &tools)?;
    // Publish only after the complete view hierarchy has been built.
    env.set_field(
        service,
        "preview",
        "Landroid/widget/TextView;",
        JValue::Object(&preview),
    )?;
    env.set_field(
        service,
        "candidates",
        "Landroid/widget/LinearLayout;",
        JValue::Object(&candidates),
    )?;
    Ok(root)
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_createInputView(
    mut env: JNIEnv,
    _class: JClass,
    service: JObject,
) -> jni::sys::jobject {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        env.with_local_frame_returning_local(64, |env| build(env, &service))
    }));
    match result {
        Ok(Ok(view)) => view.into_raw(),
        _ => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            std::ptr::null_mut()
        }
    }
}
fn render(env: &mut JNIEnv, service: &JObject, reply: &Render) -> Result<()> {
    let preview = env
        .get_field(service, "preview", "Landroid/widget/TextView;")?
        .l()?;
    if !preview.is_null() {
        let text = env.new_string(&reply.text)?;
        env.call_method(
            &preview,
            "setText",
            "(Ljava/lang/CharSequence;)V",
            &[JValue::Object(&text)],
        )?;
    }
    let candidates = env
        .get_field(service, "candidates", "Landroid/widget/LinearLayout;")?
        .l()?;
    if candidates.is_null() {
        return Ok(());
    }
    env.call_method(&candidates, "removeAllViews", "()V", &[])?;
    if let Some(view) = &reply.view {
        for (action, choices) in [
            (10, view.candidates.as_slice()),
            (11, view.predictions.as_deref().unwrap_or_default()),
        ] {
            for (index, label) in choices.iter().enumerate() {
                env.with_local_frame(16, |env| -> Result<()> {
                    let button = widget(env, "android/widget/Button", service)?;
                    let text = env.new_string(label)?;
                    env.call_method(
                        &button,
                        "setText",
                        "(Ljava/lang/CharSequence;)V",
                        &[JValue::Object(&text)],
                    )?;
                    let callback = env
                        .call_method(
                            service,
                            "keyboardCallback",
                            "(II)Landroid/view/View$OnClickListener;",
                            &[JValue::Int(action), JValue::Int(index as i32)],
                        )?
                        .l()?;
                    env.call_method(
                        &button,
                        "setOnClickListener",
                        "(Landroid/view/View$OnClickListener;)V",
                        &[JValue::Object(&callback)],
                    )?;
                    add(env, &candidates, &button)
                })?;
            }
        }
    }
    Ok(())
}
#[no_mangle]
pub extern "system" fn Java_moe_meltype_rust_Native_renderInputView(
    mut env: JNIEnv,
    _class: JClass,
    service: JObject,
    json: JString,
) -> jboolean {
    let reply = env
        .get_string(&json)
        .ok()
        .and_then(|s| serde_json::from_str::<Render>(&String::from(s)).ok());
    let consumed = reply.as_ref().is_some_and(|r| r.consumed);
    if let Some(reply) = reply {
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| {
            env.with_local_frame(32, |env| render(env, &service, &reply))
        }));
    }
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    // A widget failure must not replay a key already applied to the editor.
    u8::from(consumed)
}
