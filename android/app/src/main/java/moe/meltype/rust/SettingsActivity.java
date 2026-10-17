// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Intent;
import android.os.Bundle;
import android.widget.*;
import org.json.*;

/** Generic Android widgets/document bridge; page definitions and actions live in Rust. */
public final class SettingsActivity extends Activity {
    private long session;
    private LinearLayout body;
    private TextView status;
    private String documentOperation;
    private SettingsPage form;
    private static final int DOCUMENT = 1;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (state != null) documentOperation = state.getString("documentOperation");
        session = Native.create(getFilesDir().getAbsolutePath());
        ScrollView scroll = new ScrollView(this);
        LinearLayout root = new LinearLayout(this); root.setOrientation(LinearLayout.VERTICAL);
        int padding = (int)(16 * getResources().getDisplayMetrics().density);
        root.setPadding(padding,padding,padding,padding);
        scroll.addView(root); setContentView(scroll);
        TextView title = new TextView(this); title.setText("Meltype Rust 設定"); title.setTextSize(24); root.addView(title);
        status = new TextView(this); root.addView(status);
        Spinner navigation = new Spinner(this);
        navigation.setAdapter(new ArrayAdapter<>(this,android.R.layout.simple_spinner_dropdown_item,
                new String[]{"入力設定","ユーザー辞書","定型文","学習","プロファイル","バックアップ・診断"}));
        root.addView(navigation);
        body = new LinearLayout(this); body.setOrientation(LinearLayout.VERTICAL); root.addView(body);
        form = new SettingsPage(this,body,operation -> action(() -> execute(operation,form.values(operation))));
        navigation.setOnItemSelectedListener(new android.widget.AdapterView.OnItemSelectedListener() {
            public void onNothingSelected(AdapterView<?> parent) {}
            public void onItemSelected(AdapterView<?> parent,android.view.View view,int position,long id) { show(position); }
        });
    }
    @Override protected void onSaveInstanceState(Bundle state) { state.putString("documentOperation",documentOperation); super.onSaveInstanceState(state); }
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
        JSONObject result = checked(Native.settingsAction(session,operation.toString(),values.toString()));
        if (result.has("edit")) form.edit(result.getJSONObject("edit"));
        if (result.has("pick")) {
            JSONObject pick = result.getJSONObject("pick"); pick(pick.getString("operation"),pick.getBoolean("output"));
        }
        if (result.has("confirm")) {
            JSONObject confirm = result.getJSONObject("confirm");
            new AlertDialog.Builder(this).setMessage(confirm.getString("message"))
                .setNegativeButton("キャンセル",null).setPositiveButton(confirm.getString("positive"),
                    (dialog,which) -> action(() -> execute(confirm.getJSONObject("action"),form.values()))).show();
        }
        if (result.has("page")) show(result.getInt("page"));
    }
    private void show(int page) {
        form.clear(); status.setText("");
        action(() -> form.render(checked(Native.settingsPage(session,page))));
    }
    private void pick(String operation,boolean output) {
        documentOperation = operation;
        Intent intent = new Intent(output ? Intent.ACTION_CREATE_DOCUMENT : Intent.ACTION_OPEN_DOCUMENT);
        intent.setType("*/*").addCategory(Intent.CATEGORY_OPENABLE);
        if (output) intent.putExtra(Intent.EXTRA_TITLE,operation.equals("dictionary_export") ? "meltype-dictionary.txt" : "meltype-backup.json");
        startActivityForResult(intent,DOCUMENT);
    }
    @Override protected void onActivityResult(int code,int result,Intent data) {
        super.onActivityResult(code,result,data);
        if (code != DOCUMENT || result != RESULT_OK || data == null || data.getData() == null || documentOperation == null) return;
        String operation = documentOperation; documentOperation = null;
        action(() -> {
            JSONObject reply = checked(Native.settingsDocument(session,this,data.getData(),operation));
            if (reply.has("confirm")) {
                JSONObject confirm = reply.getJSONObject("confirm"); long token = confirm.getLong("token");
                try {
                    new AlertDialog.Builder(this).setTitle(confirm.getString("title")).setMessage(confirm.getString("message"))
                        .setNegativeButton("キャンセル",(dialog,which) -> Native.settingsRestore(session,token,false))
                        .setOnCancelListener(dialog -> Native.settingsRestore(session,token,false))
                        .setPositiveButton("復元",(dialog,which) -> action(() -> checked(Native.settingsRestore(session,token,true)))).show();
                } catch (Exception error) { Native.settingsRestore(session,token,false); throw error; }
            }
            if (reply.has("page")) show(reply.getInt("page"));
        });
    }
}
