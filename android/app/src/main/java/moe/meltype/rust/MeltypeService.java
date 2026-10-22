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
    private FlickKeyboard keyboard;

    @Override public void onCreate() { super.onCreate(); session = Native.create(getFilesDir().getAbsolutePath()); }
    @Override public void onDestroy() { if(keyboard!=null)keyboard.cancelGestures(); keyboard=null; Native.destroy(session); session = 0; super.onDestroy(); }
    @Override public void onStartInput(EditorInfo info, boolean restarting) {
        super.onStartInput(info, restarting);
        if(keyboard!=null)keyboard.cancelGestures();
        if(restarting)Native.serviceRestart(session,this,info);
        else Native.serviceStart(session,this,info);
    }
    @Override public void onFinishInput() { if(keyboard!=null)keyboard.cancelGestures(); Native.serviceFinish(session,this); super.onFinishInput(); }
    @Override public boolean onEvaluateFullscreenMode() { return false; }
    @Override public void onFinishInputView(boolean finishingInput) { if(keyboard!=null)keyboard.cancelGestures(); super.onFinishInputView(finishingInput); }
    @Override public View onCreateInputView() {
        LinearLayout root=(LinearLayout)Native.serviceView(session,this);
        if(keyboard!=null)keyboard.cancelGestures();
        if(root!=null)keyboard=FlickKeyboard.attach(this,root,new FlickKeyboard.Listener(){
            public void key(int scalar){Native.serviceClick(session,MeltypeService.this,0,scalar);}
            public void variant(int kind){Native.serviceClick(session,MeltypeService.this,12,kind);}
            public void layout(boolean flick){getSharedPreferences("keyboard",MODE_PRIVATE).edit().putBoolean("flick",flick).apply();}
        },getSharedPreferences("keyboard",MODE_PRIVATE).getBoolean("flick",true));
        return root;
    }
    private View.OnClickListener keyboardCallback(int action,int value){return view->Native.serviceClick(session,this,action,value);}
    @Override public boolean onKeyDown(int keyCode,KeyEvent event){return Native.serviceKey(session,this,keyCode,event)||super.onKeyDown(keyCode,event);}
    @Override public void onUpdateSelection(int oldStart,int oldEnd,int newStart,int newEnd,int candidatesStart,int candidatesEnd){
        super.onUpdateSelection(oldStart,oldEnd,newStart,newEnd,candidatesStart,candidatesEnd);
        Native.serviceSelection(session,this,newStart,newEnd,candidatesEnd);
    }
}
