// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Context;
import android.view.View;
import android.widget.CompoundButton;
import android.widget.LinearLayout;
import org.json.*;

/** Listener bridge; Rust creates widgets and reads/edits their fields through JNI. */
final class SettingsPage {
    interface Actions { void perform(JSONObject operation); }
    private final Context context;
    private final LinearLayout body;
    private final Actions actions;
    SettingsPage(Context context,LinearLayout body,Actions actions) {
        this.context=context;this.body=body;this.actions=actions;
    }
    private static JSONObject checked(String json) throws JSONException {
        JSONObject value=new JSONObject(json);
        if(value.has("error"))throw new JSONException(value.getString("error"));
        return value;
    }
    void clear(){body.removeAllViews();}
    JSONObject values() throws JSONException {return checked(Native.settingsValues(this,"null"));}
    JSONObject values(JSONObject operation) throws JSONException {return checked(Native.settingsValues(this,operation.toString()));}
    void edit(JSONObject edits) throws JSONException {checked(Native.editSettingsFields(this,edits.toString()));}
    void render(JSONObject page) throws JSONException {checked(Native.renderSettingsPage(this,page.toString()));}
    private void dispatch(String operation) {
        try{actions.perform(new JSONObject(operation));}catch(JSONException error){throw new IllegalStateException(error);}
    }
    private View.OnClickListener settingsButtonCallback(String operation){return view->dispatch(operation);}
    private CompoundButton.OnCheckedChangeListener settingsToggleCallback(String operation){return (view,checked)->dispatch(operation);}
}
