// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.inputmethodservice.InputMethodService;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.EditorInfo;
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
        Native.serviceStart(session,this,info);
    }
    @Override public void onFinishInput() { Native.serviceFinish(session,this); super.onFinishInput(); }
    @Override public boolean onEvaluateFullscreenMode() { return false; }
    @Override public View onCreateInputView() {return (View)Native.serviceView(session,this);}
    private View.OnClickListener keyboardCallback(int action,int value){return view->Native.serviceClick(session,this,action,value);}
    @Override public boolean onKeyDown(int keyCode,KeyEvent event){return Native.serviceKey(session,this,keyCode,event)||super.onKeyDown(keyCode,event);}
    @Override public void onUpdateSelection(int oldStart,int oldEnd,int newStart,int newEnd,int candidatesStart,int candidatesEnd){
        super.onUpdateSelection(oldStart,oldEnd,newStart,newEnd,candidatesStart,candidatesEnd);
        Native.serviceSelection(session,this,newStart,newEnd,candidatesEnd);
    }
}
