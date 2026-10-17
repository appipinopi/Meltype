// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.app.Activity;
import android.content.DialogInterface;
import android.content.Intent;
import android.os.Bundle;
import android.widget.*;
import org.json.*;

/** Generic Android widgets/document bridge; page definitions and actions live in Rust. */
public final class SettingsActivity extends Activity {
    private long session;
    private LinearLayout body;
    private TextView status;
    private SettingsPage form;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        session = Native.create(getFilesDir().getAbsolutePath());
        if (state != null) Native.restoreDocumentPicker(session,state.getString("documentOperation"));
        try { checked(Native.createSettingsActivity(this)); }
        catch (JSONException error) { throw new IllegalStateException(error); }
    }
    private void settingsFormReady() {
        form = new SettingsPage(this,body,operation -> action(() -> execute(operation,form.values(operation))));
    }
    private android.widget.AdapterView.OnItemSelectedListener settingsNavigationCallback() {
        return new android.widget.AdapterView.OnItemSelectedListener() {
            public void onNothingSelected(AdapterView<?> parent) {}
            public void onItemSelected(AdapterView<?> parent,android.view.View view,int position,long id) { show(position); }
        };
    }
    @Override protected void onSaveInstanceState(Bundle state) { state.putString("documentOperation",Native.pendingDocument(session)); super.onSaveInstanceState(state); }
    @Override public void onDestroy() { Native.destroy(session); super.onDestroy(); }
    private void action(RunnableChecked action) {
        try { action.run(); status.setText("完了"); }
        catch (Exception error) { status.setText("エラー: " + error.getMessage()); }
    }
    private interface RunnableChecked { void run() throws Exception; }
    private static JSONObject checked(String json) throws JSONException {
        JSONObject result = new JSONObject(json);
        if (result.has("error")) throw new JSONException(result.getString("error"));
        return result;
    }
    private void execute(JSONObject operation,JSONObject values) throws JSONException {
        applyReply(checked(Native.settingsAction(session,operation.toString(),values.toString())));
    }
    private void applyReply(JSONObject result) throws JSONException {
        if (result.has("edit")) form.edit(result.getJSONObject("edit"));
        if (result.has("pick")) {
            JSONObject pick = result.getJSONObject("pick"); checked(Native.settingsPick(session,this,pick.getString("operation")));
        }
        if (result.has("confirm")) {
            checked(Native.showSettingsConfirmation(session,this,result.getJSONObject("confirm").toString()));
        }
        if (result.has("page")) show(result.getInt("page"));
    }
    private void show(int page) {
        form.clear(); status.setText("");
        action(() -> form.render(checked(Native.settingsPage(session,page))));
    }
    @Override protected void onActivityResult(int code,int result,Intent data) {
        super.onActivityResult(code,result,data);
        action(() -> applyReply(checked(Native.settingsPickerResult(session,this,code,result,data))));
    }
    private DialogInterface.OnClickListener settingsConfirmationClick(String payload,boolean accept) {
        return (dialog,which) -> action(() -> applyReply(checked(Native.completeSettingsConfirmation(session,payload,accept))));
    }
    private DialogInterface.OnCancelListener settingsConfirmationCancel(String payload) {
        return dialog -> action(() -> applyReply(checked(Native.completeSettingsConfirmation(session,payload,false))));
    }
}
