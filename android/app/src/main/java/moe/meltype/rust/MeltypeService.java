// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Intent;
import android.inputmethodservice.InputMethodService;
import android.os.Build;
import android.text.InputType;
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
    private boolean direct;
    private boolean shifted;
    private boolean composing;
    private boolean privateEditor;
    private String composingText = "";
    private LinearLayout candidates;
    private TextView preview;

    @Override public void onCreate() { super.onCreate(); session = Native.create(getFilesDir().getAbsolutePath()); }
    @Override public void onDestroy() { Native.destroy(session); session = 0; super.onDestroy(); }
    @Override public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        Native.reset(session); composing = false; shifted = false;
        int kind = info.inputType & InputType.TYPE_MASK_CLASS;
        int variation = info.inputType & InputType.TYPE_MASK_VARIATION;
        boolean password = kind == InputType.TYPE_CLASS_TEXT && (
            variation == InputType.TYPE_TEXT_VARIATION_PASSWORD || variation == InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD ||
            variation == InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD) ||
            kind == InputType.TYPE_CLASS_NUMBER && variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD;
        boolean privateInput = password || (info.imeOptions & EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING) != 0;
        privateEditor = privateInput;
        composingText = "";
        Native.privateInput(session, privateInput);
        direct = password || kind == InputType.TYPE_CLASS_NUMBER || kind == InputType.TYPE_CLASS_PHONE || kind == InputType.TYPE_CLASS_DATETIME;
        Native.direct(session, direct);
        if (preview != null) preview.setText("");
        if (candidates != null) candidates.removeAllViews();
    }
    @Override public void onFinishInput() {
        if (composing) apply(Native.commit(session));
        Native.reset(session); composing = false;
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
                int scalar = shifted ? Character.toUpperCase(letter) : letter;
                softwareKey(0, scalar); shifted = false;
            });
            root.addView(row);
        }
        LinearLayout actions = new LinearLayout(this);
        add(actions, "⇧", () -> { shifted = !shifted; });
        add(actions, "あ/A", () -> { apply(Native.commit(session)); direct = !direct; Native.direct(session, direct); });
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
        add(tools, "戻す", () -> { if (!privateEditor) apply(Native.undo(session,beforeCursor())); });
        add(tools, "再変換", () -> {
            InputConnection connection = getCurrentInputConnection();
            if (connection == null || privateEditor || composing) return;
            CharSequence selected = connection.getSelectedText(0);
            if (selected != null && !selected.toString().isEmpty()) {
                if (!apply(Native.reconvert(session,selected.toString())) && preview != null) preview.setText("読みが見つかりません");
            }
        });
        add(tools, "設定", () -> { apply(Native.commit(session)); startActivity(new Intent(this,SettingsActivity.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)); });
        root.addView(tools); return root;
    }
    private void add(LinearLayout row, String label, Runnable action) {
        Button button = new Button(this); button.setText(label); button.setMinWidth(0); button.setMinimumWidth(0); button.setPadding(1,1,1,1);
        button.setOnClickListener(view -> action.run());
        row.addView(button, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT,1));
    }
    private void softwareKey(int vk, int scalar) {
        if (apply(key(vk,scalar,0))) return;
        InputConnection connection = getCurrentInputConnection(); if (connection == null) return;
        if (scalar > 0) connection.commitText(new String(Character.toChars(scalar)),1);
        else if (vk == 8) connection.deleteSurroundingTextInCodePoints(1,0);
        else if (vk == 0x25 || vk == 0x27) {
            int key = vk == 0x25 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
            connection.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_DOWN,key));
            connection.sendKeyEvent(new KeyEvent(KeyEvent.ACTION_UP,key));
        }
        else if (vk == 13) {
            EditorInfo info = getCurrentInputEditorInfo();
            int action = info.imeOptions & EditorInfo.IME_MASK_ACTION;
            if (action != EditorInfo.IME_ACTION_NONE && action != EditorInfo.IME_ACTION_UNSPECIFIED && (info.imeOptions & EditorInfo.IME_FLAG_NO_ENTER_ACTION) == 0) connection.performEditorAction(action);
            else connection.commitText("\n",1);
        }
    }
    @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
        int vk = switch (keyCode) {
            case KeyEvent.KEYCODE_DEL -> 8; case KeyEvent.KEYCODE_ENTER -> 13;
            case KeyEvent.KEYCODE_SPACE -> 32; case KeyEvent.KEYCODE_ESCAPE -> 27;
            case KeyEvent.KEYCODE_DPAD_UP -> 38; case KeyEvent.KEYCODE_DPAD_DOWN -> 40;
            case KeyEvent.KEYCODE_DPAD_LEFT -> 37; case KeyEvent.KEYCODE_DPAD_RIGHT -> 39;
            case KeyEvent.KEYCODE_TAB -> 9;
            case KeyEvent.KEYCODE_F6 -> 0x75; case KeyEvent.KEYCODE_F7 -> 0x76;
            case KeyEvent.KEYCODE_F8 -> 0x77; case KeyEvent.KEYCODE_F9 -> 0x78; case KeyEvent.KEYCODE_F10 -> 0x79;
            default -> 0;
        };
        if (KeyEvent.isModifierKey(keyCode)) return super.onKeyDown(keyCode,event);
        int mods = (event.isShiftPressed()?1:0) | (event.isCtrlPressed()?2:0) | (event.isAltPressed()?4:0) | (event.isMetaPressed()?8:0);
        return apply(key(vk,event.getUnicodeChar(),mods)) || super.onKeyDown(keyCode,event);
    }
    private String beforeCursor() {
        InputConnection connection = getCurrentInputConnection();
        if (connection == null || privateEditor) return "";
        CharSequence value = connection.getTextBeforeCursor(4000,0);
        String text = value == null ? "" : value.toString();
        if (composing && text.endsWith(composingText)) return text.substring(0,text.length()-composingText.length());
        return text;
    }
    private String key(int vk, int scalar, int modifiers) {
        InputConnection connection = getCurrentInputConnection();
        CharSequence after = connection == null || privateEditor ? "" : connection.getTextAfterCursor(4000,0);
        return Native.keyContext(session,vk,scalar,modifiers,beforeCursor(),after == null ? "" : after.toString());
    }
    @Override public void onUpdateSelection(int oldStart,int oldEnd,int newStart,int newEnd,int candidatesStart,int candidatesEnd) {
        super.onUpdateSelection(oldStart,oldEnd,newStart,newEnd,candidatesStart,candidatesEnd);
        if (composing && (newStart != newEnd || newEnd != candidatesEnd)) {
            Native.reset(session); composing = false; composingText = "";
            InputConnection connection = getCurrentInputConnection();
            if (connection != null) connection.finishComposingText();
            if (preview != null) preview.setText("");
            if (candidates != null) candidates.removeAllViews();
        }
    }
    private boolean apply(String json) {
        InputConnection connection = getCurrentInputConnection(); if (connection == null) return false;
        try {
            JSONObject result = new JSONObject(json);
            if (result.has("selectionExpect")) {
                CharSequence selected = connection.getSelectedText(0);
                if (selected == null || !result.getString("selectionExpect").contentEquals(selected)) {
                    Native.reset(session); return false;
                }
            }
            connection.beginBatchEdit();
            try {
                JSONArray commits = result.getJSONArray("commits");
                for (int i=0;i<commits.length();i++) {
                    JSONObject edit = commits.getJSONObject(i);
                    int count = edit.optInt("deleteBefore");
                    if (count > 0) {
                        String expect = edit.optString("expect", "");
                        CharSequence actual = connection.getTextBeforeCursor(count,0);
                        if (expect.length() != count || actual == null || !expect.contentEquals(actual)
                                || !connection.deleteSurroundingText(count,0)) {
                            Native.reset(session); composing = false; composingText = "";
                            return false;
                        }
                    }
                    if (!edit.getString("text").isEmpty()) connection.commitText(edit.getString("text"),1);
                }
                JSONObject view = result.optJSONObject("view");
                composing = view != null;
                composingText = view == null ? "" : view.getString("text");
                if (view == null) { connection.finishComposingText(); if (preview != null) preview.setText(""); }
                else {
                    String text = view.getString("text"); connection.setComposingText(text,1);
                    if (preview != null) preview.setText(text);
                }
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
            } finally { connection.endBatchEdit(); }
            return result.getBoolean("consumed");
        } catch (JSONException error) { return false; }
    }
}
