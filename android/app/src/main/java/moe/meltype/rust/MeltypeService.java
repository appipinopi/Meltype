// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.inputmethodservice.InputMethodService;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.widget.LinearLayout;
import android.widget.TextView;

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
        View root = (View)Native.createInputView(this);
        render(Native.editorView(session));
        return root;
    }
    private View.OnClickListener keyboardCallback(int action, int value) {
        return view -> {
            EditorInfo info = getCurrentInputEditorInfo();
            Native.keyboardAction(session,this,getCurrentInputConnection(),action,value,info == null ? 0 : info.imeOptions);
        };
    }
    @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (Native.isModifierKey(keyCode)) return super.onKeyDown(keyCode,event);
        String reply = Native.editorKey(session,getCurrentInputConnection(),keyCode,event.getUnicodeChar(),
                event.getMetaState(),true,privateEditor(),Native.composingText(session));
        return apply(reply) || super.onKeyDown(keyCode,event);
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
    private boolean render(String json) { return Native.renderInputView(this,json); }
}
