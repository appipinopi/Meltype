// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Context;
import android.view.View;
import android.widget.CompoundButton;
import android.widget.LinearLayout;

/** Listener bridge; Rust creates widgets and reads/edits their fields through JNI. */
final class SettingsPage {
    interface Actions { void perform(String operation); }
    private final Context context;
    private final LinearLayout body;
    private final Actions actions;
    SettingsPage(Context context,LinearLayout body,Actions actions) {
        this.context=context;this.body=body;this.actions=actions;
    }
    void clear(){body.removeAllViews();}
    String values(){return Native.settingsValues(this,"null");}
    String values(String operation){return Native.settingsValues(this,operation);}
    String edit(String edits){return Native.editSettingsFields(this,edits);}
    String render(String page){return Native.renderSettingsPage(this,page);}
    private void dispatch(String operation){actions.perform(operation);}
    private View.OnClickListener settingsButtonCallback(String operation){return view->dispatch(operation);}
    private CompoundButton.OnCheckedChangeListener settingsToggleCallback(String operation){return (view,checked)->dispatch(operation);}
}
