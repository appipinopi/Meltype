// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.app.Activity;
import android.app.AlertDialog;
import android.content.Intent;
import android.os.Bundle;
import android.widget.*;
import org.json.*;
import java.io.*;
import java.nio.file.Files;
import java.util.Iterator;

/** Android widgets and document picker; validation and persistence live in Rust. */
public final class SettingsActivity extends Activity {
    private long session;
    private LinearLayout body;
    private TextView status;
    private String documentOperation;
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
        navigation.setOnItemSelectedListener(new android.widget.AdapterView.OnItemSelectedListener() {
            public void onNothingSelected(AdapterView<?> parent) {}
            public void onItemSelected(AdapterView<?> parent,android.view.View view,int position,long id) { show(position); }
        });
    }
    @Override protected void onSaveInstanceState(Bundle state) { state.putString("documentOperation",documentOperation); super.onSaveInstanceState(state); }
    @Override public void onDestroy() { Native.destroy(session); super.onDestroy(); }
    private JSONObject request(String op,String... fields) throws JSONException {
        JSONObject value = new JSONObject().put("op",op);
        for (int i=0;i<fields.length;i+=2) value.put(fields[i],fields[i+1]);
        JSONObject result = new JSONObject(Native.control(session,value.toString()));
        if (result.has("error")) throw new JSONException(result.getString("error"));
        return result;
    }
    private void action(RunnableChecked action) {
        try { action.run(); status.setText("完了"); }
        catch (Exception error) { status.setText("エラー: " + error.getMessage()); }
    }
    private interface RunnableChecked { void run() throws Exception; }
    private void button(String label,RunnableChecked action) {
        Button button = new Button(this); button.setText(label); button.setOnClickListener(v -> action(action)); body.addView(button);
    }
    private void label(String value) { TextView text = new TextView(this); text.setText(value); text.setTextIsSelectable(true); body.addView(text); }
    private EditText field(String hint) { EditText edit = new EditText(this); edit.setHint(hint); body.addView(edit); return edit; }
    private void show(int page) {
        body.removeAllViews(); status.setText("");
        action(() -> {
            switch (page) {
                case 0 -> settings();
                case 1 -> dictionary();
                case 2 -> snippets();
                case 3 -> learning();
                case 4 -> profiles();
                default -> backups();
            }
        });
    }
    private void settings() throws JSONException {
        JSONObject settings = request("settings_get");
        String[][] toggles = {
            {"Enabled","入力を有効にする"}, {"LearningEnabled","入力から学習する"},
            {"PredictiveCandidates","予測候補"}, {"TranslationCandidates","英訳候補"},
            {"ShowCandidateMeanings","候補の意味"}, {"ShowTypedKeys","入力したキーを表示"},
            {"CorrectTypos","書き間違いの提案"}, {"SpaceAroundEnglish","英語の前後に空白"},
            {"SlashAsMiddleDot","スラッシュを中点にする"}, {"TabConversion","Tabで変換"},
            {"ContinueAfterConversion","変換後も入力を続ける"}, {"SeparateKeyRoles","変換キーの役割を分ける"}
        };
        for (String[] option : toggles) {
            Switch toggle = new Switch(this); toggle.setText(option[1]); toggle.setChecked(settings.optBoolean(option[0]));
            toggle.setOnCheckedChangeListener((button,checked) -> action(() -> {
                JSONObject r = new JSONObject().put("op","settings_set").put("name",option[0]).put("value",checked);
                JSONObject result = new JSONObject(Native.control(session,r.toString()));
                if (result.has("error")) throw new JSONException(result.getString("error"));
            })); body.addView(toggle);
        }
        label("ソフトキーボードはローマ字入力です。JISかな入力は外付けキーボードで使用できます。");
        EditText mark = field("定型文の開始記号"); mark.setText(settings.optString("SnippetMark"));
        button("開始記号を保存",() -> request("settings_set","name","SnippetMark","value",mark.getText().toString()));
    }
    private void dictionary() throws JSONException {
        EditText reading = field("読み（ひらがな）"), word = field("単語");
        button("登録",() -> { request("dictionary_set","reading",reading.getText().toString(),"word",word.getText().toString()); show(1); });
        button("辞書を取り込む",() -> pick("dictionary_import",false));
        button("辞書を書き出す",() -> pick("dictionary_export",true));
        JSONObject entries = request("dictionary_list");
        Iterator<String> keys = entries.keys();
        while (keys.hasNext()) {
            String key = keys.next(); JSONArray words = entries.getJSONArray(key);
            for (int i=0;i<words.length();i++) {
                String value = words.getString(i); label(key + " → " + value);
                button("この単語を削除",() -> { request("dictionary_remove","reading",key,"word",value); show(1); });
            }
        }
    }
    private void snippets() throws JSONException {
        EditText name = field("呼び出し名"), text = field("展開する文字列"); text.setMinLines(2);
        button("保存",() -> { request("snippet_set","name",name.getText().toString(),"text",text.getText().toString()); show(2); });
        JSONObject entries = request("snippets_list"); Iterator<String> keys = entries.keys();
        while (keys.hasNext()) {
            String key = keys.next(), value = entries.getString(key); label(key + " → " + value);
            button("編集",() -> { name.setText(key); text.setText(value); });
            button("削除",() -> { request("snippet_remove","name",key); show(2); });
        }
    }
    private void learning() throws JSONException {
        JSONObject learned = request("learning_list");
        JSONObject languages = learned.getJSONObject("languages");
        label("覚えた入力言語");
        Iterator<String> names = languages.keys();
        while (names.hasNext()) {
            String name = names.next(); JSONObject entry = languages.getJSONObject(name);
            label(name + " → " + (entry.optBoolean("english") ? "英語" : "日本語"));
        }
        label("覚えた変換");
        JSONObject conversions = learned.getJSONObject("conversions"); names = conversions.keys();
        while (names.hasNext()) { String name = names.next(); label(name + " → " + conversions.getString(name)); }
        label("覚えたフレーズ");
        JSONArray phrases = learned.getJSONArray("phrases");
        for (int i=0;i<phrases.length();i++) {
            JSONObject phrase = phrases.getJSONObject(i);
            label(phrase.getString("reading") + " → " + phrase.getString("text"));
        }
        EditText key = field("削除する学習キー");
        button("指定した学習を削除",() -> { request("learning_remove","key",key.getText().toString()); show(3); });
        button("すべての学習を削除",() -> new AlertDialog.Builder(this).setMessage("学習した内容をすべて削除しますか？")
                .setNegativeButton("キャンセル",null).setPositiveButton("削除",(dialog,which) -> action(() -> { request("learning_clear"); show(3); })).show());
    }
    private void profiles() throws JSONException {
        label("使用中: " + request("settings_get").optString("ActiveProfile"));
        EditText name = field("プロファイル名");
        button("新規作成",() -> { request("profile_add","name",name.getText().toString()); show(4); });
        button("切り替え",() -> { request("profile_switch","name",name.getText().toString()); show(4); });
        button("削除",() -> { request("profile_delete","name",name.getText().toString()); show(4); });
        JSONArray available = request("settings_get").getJSONArray("Profiles");
        for (int i=0;i<available.length();i++) {
            String profileName = available.getJSONObject(i).getString("Name");
            button(profileName,() -> name.setText(profileName));
        }
    }
    private void backups() throws JSONException {
        button("バックアップを保存",() -> pick("backup_create",true));
        button("バックアップを検査・復元",() -> pick("backup_restore",false));
        label(request("diagnostics").toString(2));
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
            File temporary = File.createTempFile("meltype-document-",".tmp",getCacheDir());
            boolean retained = false;
            try {
                boolean output = operation.equals("dictionary_export") || operation.equals("backup_create");
                if (!output) {
                    try (InputStream in = getContentResolver().openInputStream(data.getData()); OutputStream out = new FileOutputStream(temporary)) {
                        if (in == null) throw new IOException("ファイルを開けません");
                        byte[] buffer = new byte[8192]; int count; long total = 0;
                        while ((count = in.read(buffer)) != -1) {
                            total += count; if (total > 32L*1024*1024) throw new IOException("上限は32 MiBです");
                            out.write(buffer,0,count);
                        }
                    }
                }
                if (operation.equals("backup_restore")) {
                    JSONObject report = request("backup_inspect","path",temporary.getAbsolutePath());
                    new AlertDialog.Builder(this).setTitle("バックアップを復元").setMessage(report.toString(2))
                        .setNegativeButton("キャンセル",(dialog,which) -> temporary.delete())
                        .setOnCancelListener(dialog -> temporary.delete())
                        .setPositiveButton("復元",(dialog,which) -> action(() -> {
                            try { request("backup_restore","path",temporary.getAbsolutePath()); }
                            finally { temporary.delete(); }
                        })).show();
                    retained = true;
                    return;
                }
                request(operation,"path",temporary.getAbsolutePath());
                if (output) try (OutputStream out = getContentResolver().openOutputStream(data.getData(),"wt")) {
                    if (out == null) throw new IOException("保存先を開けません"); Files.copy(temporary.toPath(),out);
                }
                if (operation.equals("dictionary_import")) show(1);
            } finally {
                if (!retained) temporary.delete();
            }
        });
    }
}
