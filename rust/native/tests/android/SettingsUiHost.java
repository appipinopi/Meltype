// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.app.AlertDialog;
import android.content.*;
import android.view.View;
import android.widget.*;
import android.net.Uri;
import java.io.*;
import java.nio.file.*;
public final class SettingsUiHost extends Context {
    long session;SettingsPage form;LinearLayout body;TextView status;String last;Intent launched;
    final DocumentHost documents;
    SettingsUiHost(Path cache){documents=new DocumentHost(cache);}
    public File getCacheDir(){return documents.getCacheDir();}
    public ContentResolver getContentResolver(){return documents.getContentResolver();}
    private void settingsFormReady(){form=new SettingsPage(this);}
    private AdapterView.OnItemSelectedListener settingsNavigationCallback(){return new AdapterView.OnItemSelectedListener(){
        public void onNothingSelected(AdapterView parent){}
        public void onItemSelected(AdapterView parent,View view,int position,long id){last=Native.settingsUiShow(session,SettingsUiHost.this,position);}
    };}
    public void setContentView(View view){((Spinner)((LinearLayout)((ScrollView)view).children.get(0)).children.get(2)).choose(0);}
    public void startActivityForResult(Intent intent,int code){check(code==1,"picker request");launched=intent;}
    private DialogInterface.OnClickListener settingsConfirmationClick(String payload,boolean accept){return (dialog,which)->last=Native.settingsUiConfirmation(session,this,payload,accept);}
    private DialogInterface.OnCancelListener settingsConfirmationCancel(String payload){return dialog->last=Native.settingsUiConfirmation(session,this,payload,false);}
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    void success(){check("{}".equals(last),last);check(status.text.equals("完了"),status.text);}
    void show(int page){last=Native.settingsUiShow(session,this,page);success();}
    EditText field(String hint){for(View view:body.children)if(view instanceof EditText edit&&edit.hint.equals(hint))return edit;throw new AssertionError(hint);}
    void press(String text){for(View view:body.children)if(view instanceof Button button&&button.text.equals(text)){button.performClick();success();return;}throw new AssertionError(text);}
    void result(){Intent data=new Intent("result");data.data=new Uri();last=Native.settingsUiResult(session,this,1,-1,data);success();}
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-settings-ui-");Path profile=Files.createDirectory(root.resolve("profile"));
        SettingsUiHost host=new SettingsUiHost(Files.createDirectory(root.resolve("cache")));host.session=Native.create(profile.toString());
        try {
            check(Native.createSettingsActivity(host).equals("{}"),"native bootstrap");host.success();
            for(View view:host.body.children)if(view instanceof Switch toggle&&toggle.text.equals("予測候補")){toggle.setChecked(true);toggle.performClick();host.success();}
            check(Native.control(host.session,"{\"op\":\"settings_get\"}").contains("\"PredictiveCandidates\":false"),"typed toggle persisted through native UI");
            host.show(2);host.field("呼び出し名").setText("mail");host.field("展開する文字列").setText("一行目\n二行目😀");host.press("保存");
            check(host.field("呼び出し名").text.isEmpty(),"save redraws page");host.press("編集");check(host.field("展開する文字列").text.equals("一行目\n二行目😀"),"native edit reply");
            host.show(1);int count=host.body.children.size();host.last=Native.settingsUiAction(host.session,host,"{\"action\":\"dictionary_add\"}");
            check(host.last.contains("\"error\":"),"invalid dictionary fails");check(host.status.text.startsWith("エラー: ")&&host.body.children.size()==count,"error status with retained form");
            host.field("読み（ひらがな）").setText("めるたいぷ");host.field("単語").setText("Meltype😀");host.press("登録");
            host.press("辞書を書き出す");check(host.launched.action.equals("android.intent.action.CREATE_DOCUMENT"),"native picker reply");
            DocumentHost.Output dictionary=new DocumentHost.Output();host.documents.resolver.output=dictionary;host.result();check(dictionary.closed==1,"UI export closes");
            host.press("この単語を削除");host.press("辞書を取り込む");host.documents.resolver.input=new DocumentHost.Input(dictionary.toByteArray());host.result();
            check(Native.control(host.session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"UI import persists and redraws");
            host.show(3);host.press("すべての学習を削除");check(AlertDialog.last.message.equals("学習した内容をすべて削除しますか？"),"native confirmation reply");
            AlertDialog.last.cancel.onCancel(null);host.success();host.press("すべての学習を削除");AlertDialog.last.yes.onClick(null,-1);host.success();check(host.body.children.size()>0,"learning confirmation redraw");
            host.show(5);host.press("バックアップを保存");DocumentHost.Output backup=new DocumentHost.Output();host.documents.resolver.output=backup;host.result();
            host.show(1);host.press("この単語を削除");host.show(5);host.press("バックアップを検査・復元");host.documents.resolver.input=new DocumentHost.Input(backup.toByteArray());host.result();
            check(AlertDialog.last.title.equals("バックアップを復元"),"picker result stages dialog");AlertDialog.last.yes.onClick(null,-1);host.success();
            check(Native.control(host.session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"native UI confirmed restore");host.documents.empty();
            count=host.body.children.size();host.last=Native.settingsUiShow(host.session,host,99);check(host.last.contains("\"error\":"),"invalid page error");check(host.body.children.size()==count,"invalid page keeps previous contents");
            host.last=Native.settingsUiAction(host.session,host,"broken");check(host.last.contains("\"error\":")&&host.status.text.startsWith("エラー: "),"malformed operation displayed");
            host.show(0);Native.destroy(host.session);host.last=Native.settingsUiAction(host.session,host,"{\"action\":\"mark\"}");check(host.last.contains("\"error\":"),"destroyed UI cannot mutate");
            System.out.println("Android JNI settings UI: bootstrap/navigation, native field/action/reply/status orchestration, toggle/snippet/dictionary paths, picker transfers and confirmation restore, errors/retained form and destroyed session passed");
        } finally {
            Native.destroy(host.session);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
