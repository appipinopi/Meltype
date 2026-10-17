// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Context;
import android.widget.*;
import org.json.*;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.Map;

/** Reusable framework renderer for Rust-owned settings page definitions. */
final class SettingsPage {
    interface Actions { void perform(JSONObject operation); }
    private final Context context;
    private final LinearLayout body;
    private final Actions actions;
    private final Map<String,EditText> fields = new LinkedHashMap<>();
    SettingsPage(Context context,LinearLayout body,Actions actions) {
        this.context=context;this.body=body;this.actions=actions;
    }
    void clear() {body.removeAllViews();fields.clear();}
    JSONObject values() throws JSONException {
        JSONObject result=new JSONObject();
        for(Map.Entry<String,EditText> field:fields.entrySet()) result.put(field.getKey(),field.getValue().getText().toString());
        return result;
    }
    void edit(JSONObject edits) throws JSONException {
        Iterator<String> keys=edits.keys();
        while(keys.hasNext()) {
            String key=keys.next();EditText field=fields.get(key);
            if(field==null)throw new JSONException("入力項目が見つかりません: "+key);
            field.setText(edits.getString(key));
        }
    }
    void render(JSONObject page) throws JSONException {
        clear();JSONArray nodes=page.getJSONArray("nodes");
        for(int i=0;i<nodes.length();i++) {
            JSONObject node=nodes.getJSONObject(i);
            switch(node.getString("type")) {
                case "label" -> {
                    TextView text=new TextView(context);text.setText(node.getString("text"));
                    text.setTextIsSelectable(true);body.addView(text);
                }
                case "field" -> {
                    EditText edit=new EditText(context);edit.setHint(node.getString("hint"));
                    edit.setText(node.getString("value"));edit.setMinLines(node.getInt("lines"));
                    fields.put(node.getString("id"),edit);body.addView(edit);
                }
                case "toggle" -> {
                    Switch toggle=new Switch(context);toggle.setText(node.getString("text"));toggle.setChecked(node.getBoolean("checked"));
                    JSONObject operation=node.getJSONObject("action");
                    toggle.setOnCheckedChangeListener((button,enabled)->actions.perform(operation));
                    // The clicked toggle is read at dispatch time, after the framework updates it.
                    toggle.setTag(operation);body.addView(toggle);
                }
                case "button" -> {
                    Button button=new Button(context);button.setText(node.getString("text"));
                    JSONObject operation=node.getJSONObject("action");button.setOnClickListener(v->actions.perform(operation));body.addView(button);
                }
                default -> throw new JSONException("画面項目が見つかりません");
            }
        }
    }
    JSONObject values(JSONObject operation) throws JSONException {
        JSONObject result=values();
        for(int i=0;i<body.getChildCount();i++) {
            if(body.getChildAt(i) instanceof Switch toggle && toggle.getTag()==operation) {
                result.put("checked",toggle.isChecked());break;
            }
        }
        return result;
    }
}
