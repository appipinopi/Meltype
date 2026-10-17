// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Intent;
import android.inputmethodservice.InputMethodService;
import android.os.Build;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.TextView;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Android lifecycle and editor bridge; conversion and state live in Rust. */
public final class MeltypeService extends InputMethodService {
    private long session;
    private LinearLayout candidates;
    private TextView preview;

    @Override public void onCreate() { super.onCreate(); session = Native.create(getFilesDir().getAbsolutePath()); }
    @Override public void onDestroy() { Native.destroy(session); session = 0; super.onDestroy(); }
    @Override public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        Native.startInput(session, info.inputType, info.imeOptions);
        if (preview != null) preview.setText("");
        if (candidates != null) candidates.removeAllViews();
    }
    @Override public void onFinishInput() {
        render(Native.finishInput(session,getCurrentInputConnection(),composing(),Native.composingText(session)));
        super.onFinishInput();
    }
    @Override public boolean onEvaluateFullscreenMode() { return false; }
    @Override public View onCreateInputView() {
        LinearLayout root = new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL);
        preview = new TextView(this); preview.setTextSize(20); root.addView(preview);
        HorizontalScrollView scroll = new HorizontalScrollView(this);
        candidates = new LinearLayout(this); scroll.addView(candidates); root.addView(scroll);
        for (String letters : new String[]{"qwertyuiop", "asdfghjkl", "zxcvbnm", "1234567890", "-.,!?@/"}) {
            LinearLayout row = new LinearLayout(this);
            for (char letter : letters.toCharArray()) add(row, String.valueOf(letter), () -> {
                softwareKey(0, Native.takeShiftedScalar(session,letter));
            });
            root.addView(row);
        }
        LinearLayout actions = new LinearLayout(this);
        add(actions, "⇧", () -> { Native.toggleShift(session); });
        add(actions, "あ/A", () -> render(Native.toggleInputMode(session,getCurrentInputConnection())));
        add(actions, "⌫", () -> softwareKey(8,0));
        add(actions, "変換/空白", () -> softwareKey(32,32));
        add(actions, "確定/↵", () -> softwareKey(13,0));
        add(actions, "取消", () -> softwareKey(27,0));
        add(actions, "🌐", () -> {
            if (Build.VERSION.SDK_INT >= 28) switchToNextInputMethod(false);
            else ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).showInputMethodPicker();
        });
        root.addView(actions);
        LinearLayout tools = new LinearLayout(this);
        add(tools, "←", () -> softwareKey(0x25,0));
        add(tools, "→", () -> softwareKey(0x27,0));
        add(tools, "縮める", () -> apply(key(0x25,0,1)));
        add(tools, "伸ばす", () -> apply(key(0x27,0,1)));
        add(tools, "戻す", () -> { if (!privateEditor()) apply(Native.editorUndo(session,getCurrentInputConnection(),privateEditor(),Native.composingText(session))); });
        add(tools, "再変換", () -> {
            String reply = Native.editorReconvert(session,getCurrentInputConnection(),privateEditor(),composing());
            if (reply != null && !apply(reply) && preview != null)
                preview.setText("読みが見つかりません");
        });
        add(tools, "設定", () -> { apply(Native.commit(session)); startActivity(new Intent(this,SettingsActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)); });
        root.addView(tools); render(Native.editorView(session)); return root;
    }
    private void add(LinearLayout row, String label, Runnable action) {
        Button button = new Button(this); button.setText(label); button.setMinWidth(0); button.setMinimumWidth(0); button.setPadding(1,1,1,1);
        button.setOnClickListener(view -> action.run());
        row.addView(button, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT,1));
    }
    private void softwareKey(int vk, int scalar) {
        if (apply(key(vk,scalar,0))) return;
        InputConnection connection = getCurrentInputConnection(); if (connection == null) return;
        EditorInfo info = getCurrentInputEditorInfo();
        Native.softwareFallback(connection,vk,scalar,info == null ? 0 : info.imeOptions);
    }
    @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (Native.isModifierKey(keyCode)) return super.onKeyDown(keyCode,event);
        String reply = Native.editorKey(session,getCurrentInputConnection(),keyCode,event.getUnicodeChar(),
                event.getMetaState(),true,privateEditor(),Native.composingText(session));
        return apply(reply) || super.onKeyDown(keyCode,event);
    }
    private String key(int vk, int scalar, int modifiers) {
        return Native.editorKey(session,getCurrentInputConnection(),vk,scalar,modifiers,false,
                privateEditor(),Native.composingText(session));
    }
    @Override public void onUpdateSelection(int oldStart,int oldEnd,int newStart,int newEnd,int candidatesStart,int candidatesEnd) {
        super.onUpdateSelection(oldStart,oldEnd,newStart,newEnd,candidatesStart,candidatesEnd);
        if (Native.selectionChanged(session,getCurrentInputConnection(),composing(),newStart,newEnd,candidatesEnd)) {
            render(Native.editorView(session));
        }
    }
    private boolean composing() { return (Native.editorFlags(session) & 4) != 0; }
    private boolean privateEditor() { return (Native.editorFlags(session) & 2) != 0; }
    private boolean apply(String json) {
        InputConnection connection = getCurrentInputConnection(); if (connection == null) return false;
        return render(Native.applyEditor(session, connection, json, Native.composingText(session)));
    }
    private boolean render(String json) {
        try {
            JSONObject result = new JSONObject(json);
            JSONObject view = result.optJSONObject("view");
            if (preview != null) preview.setText(result.getString("composingText"));
            if (candidates != null) {
                candidates.removeAllViews();
                if (view != null) {
                    JSONArray choices = view.getJSONArray("candidates");
                    for (int i=0;i<choices.length();i++) {
                        final int index=i; Button button=new Button(this); button.setText(choices.getString(i));
                        button.setOnClickListener(v -> apply(Native.select(session,index))); candidates.addView(button);
                    }
                    JSONArray predictions = view.optJSONArray("predictions");
                    if (predictions != null) for (int i=0;i<predictions.length();i++) {
                        final int index=i; Button button=new Button(this); button.setText(predictions.getString(i));
                        button.setOnClickListener(v -> apply(Native.predict(session,index))); candidates.addView(button);
                    }
                }
            }
            return result.getBoolean("consumed");
        } catch (JSONException error) { return false; }
    }
}
