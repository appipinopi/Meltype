// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.content.Context;
import android.content.Intent;
import android.view.View;
import android.widget.*;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
public final class DispatchHost extends Context {
    private long session;
    private int options,nextIme;
    private boolean nextOnlyCurrent=true;
    private TextView preview;
    private LinearLayout candidates;
    private EditorHost.Editor editor=new EditorHost.Editor();
    private final List<Intent> intents=new ArrayList<>();
    private final Manager manager=new Manager();
    public static final class Manager {int picked;public void showInputMethodPicker(){picked++;}}
    public Object getSystemService(String name){check(name.equals("input_method"),name);return manager;}
    public boolean switchToNextInputMethod(boolean current){nextIme++;nextOnlyCurrent=current;return true;}
    public void startActivity(Intent intent){intents.add(intent);}
    private View.OnClickListener keyboardCallback(int action,int value){return view->Native.keyboardAction(session,this,editor,action,value,options);}
    private LinearLayout root;
    private void press(String label) {
        for(int group=2;group<9;group++)for(View view:((LinearLayout)root.children.get(group)).children) {
            Button button=(Button)view;if(button.text.equals(label)){check(button.performClick(),label);return;}
        }
        throw new AssertionError("missing keyboard button: "+label);
    }
    private void type(String text){for(char c:text.toCharArray())press(String.valueOf(c));}
    private void fresh(){Native.startInput(session,1,0);editor=new EditorHost.Editor();}
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    public static void main(String[] args)throws Exception {
        Path profile=Files.createTempDirectory("meltype-jni-actions-");
        Files.writeString(profile.resolve("config.json"),"{\"DetectionLevel\":\"Manual\",\"PredictiveCandidates\":true}");
        DispatchHost service=new DispatchHost();service.session=Native.create(profile.toString());check(service.session!=0,"session");
        try {
            service.root=(LinearLayout)Native.createInputView(service);check(service.root!=null,"view");
            service.fresh();Native.direct(service.session,true);service.press("⇧");service.press("a");service.press("a");
            check(service.editor.before.equals("Aa")&&(Native.editorFlags(service.session)&8)==0,service.editor.before);
            service.press("あ/A");check((Native.editorFlags(service.session)&1)==0,"mode toggle routed");
            // Composition spelling consumes nn as one unit; the next ni needs another n.
            service.fresh();service.type("konnnichiha");check(!service.editor.pending.isEmpty(),"software characters compose");
            service.options=6;service.press("確定/↵");check(service.editor.before.equals("こんにちは"),service.editor.before);
            check(!service.editor.calls.contains("action:6"),"consumed commit must not trigger editor action");
            service.press("確定/↵");check(service.editor.calls.contains("action:6"),"unconsumed Enter fallback");
            service.fresh();Native.direct(service.session,true);service.editor.before="前😀";
            service.press("⌫");check(service.editor.before.equals("前"),"delete codepoint");
            service.press("←");service.press("→");
            check(service.editor.calls.contains("event:0:21")&&service.editor.calls.contains("event:1:22"),service.editor.calls);
            service.fresh();service.type("nihongo");service.press("確定/↵");service.press("戻す");
            check(!service.editor.pending.isEmpty(),"undo action");service.press("確定/↵");check(service.editor.before.equals("にほんご"),service.editor.before);
            service.fresh();service.type("hashi");service.press("変換/空白");
            Button chosen=null;for(View view:service.candidates.children)if(((Button)view).text.equals("箸"))chosen=(Button)view;
            check(chosen!=null,"native candidates routed");chosen.performClick();check(service.editor.before.equals("箸"),service.editor.before);
            check(Native.control(service.session,"{\"op\":\"dictionary_set\",\"reading\":\"めるたいぷ\",\"word\":\"Meltype\"}").contains("saved"),"dictionary seed");
            service.fresh();service.type("meru");chosen=null;
            for(View view:service.candidates.children)if(((Button)view).text.equals("Meltype"))chosen=(Button)view;
            check(chosen!=null,"prediction button");chosen.performClick();check(service.editor.before.equals("Meltype"),service.editor.before);
            service.fresh();service.editor.selected="未登録の漢字列";service.press("再変換");
            check(service.preview.text.equals("読みが見つかりません"),service.preview.text);
            service.editor.selected="";service.press("再変換");check(service.preview.text.equals("読みが見つかりません"),"empty selection must be no-op");
            service.fresh();service.type("konnnichiha");service.press("設定");
            check(service.editor.before.equals("こんにちは")&&service.intents.size()==1,"settings commits first");
            Intent intent=service.intents.get(0);check(intent.target==SettingsActivity.class&&intent.flags==0x10000000,"settings target/flags");
            android.os.Build.VERSION.SDK_INT=35;service.press("🌐");check(service.nextIme==1&&!service.nextOnlyCurrent,"modern IME API");
            android.os.Build.VERSION.SDK_INT=27;service.press("🌐");check(service.manager.picked==1&&service.nextIme==1,"legacy picker");
            android.os.Build.VERSION.SDK_INT=35;
            service.fresh();service.editor.onRead=()->Native.startInput(service.session,0x81,0);service.press("a");
            check(service.editor.before.isEmpty()&&Native.editorFlags(service.session)==3,"reentrant read must not apply/replay into new field");
            check(service.editor.calls.equals(java.util.Collections.singletonList("before:0")),"context getters stop at a newer private field: "+service.editor.calls);
            service.fresh();service.editor.selected="橋";service.editor.onSelected=()->Native.startInput(service.session,0x81,0);service.press("再変換");
            check(Native.editorFlags(service.session)==3,"reentrant selected-text guard");
            Native.destroy(service.session);int count=service.intents.size();service.press("設定");service.press("a");
            check(service.intents.size()==count&&service.editor.before.isEmpty(),"destroyed service actions are no-ops");
            System.out.println("Android JNI actions: real button-to-core dispatch, Shift/mode, commit/Enter fallback, Unicode delete/arrows, undo/candidates/predictions, reconversion, captured settings/IME APIs and reentrant reads passed");
        } finally {
            Native.destroy(service.session);
            try(var paths=Files.walk(profile)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
