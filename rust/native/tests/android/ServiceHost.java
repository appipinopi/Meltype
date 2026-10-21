// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.content.Context;
import android.view.*;
import android.view.inputmethod.*;
import android.widget.*;
import java.nio.file.*;
public final class ServiceHost extends Context {
    long session;LinearLayout candidates;TextView preview;LinearLayout root;
    EditorHost.Editor editor=new EditorHost.Editor();EditorInfo info=new EditorInfo();
    int connections,infos;Runnable onConnection,onInfo;boolean failConnection;
    public InputConnection getCurrentInputConnection(){connections++;if(failConnection)throw new IllegalStateException("connection failed");if(onConnection!=null){Runnable next=onConnection;onConnection=null;next.run();}return editor;}
    public EditorInfo getCurrentInputEditorInfo(){infos++;if(onInfo!=null){Runnable next=onInfo;onInfo=null;next.run();}return info;}
    private View.OnClickListener keyboardCallback(int action,int value){return view->Native.serviceClick(session,this,action,value);}
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    void fresh(int inputType){editor=new EditorHost.Editor();info=new EditorInfo();info.inputType=inputType;Native.serviceStart(session,this,info);}
    boolean key(char scalar){KeyEvent event=new KeyEvent(0,29+scalar-'a');event.scalar=scalar;return Native.serviceKey(session,this,event.keyCode,event);}
    void type(String text){for(char scalar:text.toCharArray())check(key(scalar),"consumed: "+scalar);}
    void press(String label){for(int i=2;i<9;i++)for(View view:((LinearLayout)root.children.get(i)).children)if(((Button)view).text.equals(label)){view.performClick();return;}throw new AssertionError(label);}
    public static void main(String[] args)throws Exception {
        Path profile=Files.createTempDirectory("meltype-jni-service-");Files.writeString(profile.resolve("config.json"),"{\"DetectionLevel\":\"Manual\"}");
        ServiceHost host=new ServiceHost();host.session=Native.create(profile.toString());
        try {
            host.fresh(1);host.root=(LinearLayout)Native.serviceView(host.session,host);check(host.root!=null&&host.preview!=null,"service view initialized");
            int count=host.connections;host.type("nihongo");check(host.connections-count==7,"one connection snapshot per hardware event");check(!host.editor.pending.isEmpty(),"hardware composition");
            count=host.connections;check(!Native.serviceKey(host.session,host,59,new KeyEvent(0,59)),"modifier goes to superclass");check(host.connections==count,"modifier has no editor reads");
            Native.serviceFinish(host.session,host);check(host.editor.before.equals("にほんご")&&host.editor.pending.isEmpty(),"finish commits");check(host.preview.text.isEmpty()&&host.candidates.children.isEmpty(),"finish clears native view");
            host.fresh(1);host.type("kana");LinearLayout rebuilt=(LinearLayout)Native.serviceView(host.session,host);check(rebuilt!=null&&host.preview.text.equals("かな"),"UI recreation restores state");host.root=rebuilt;
            Native.serviceSelection(host.session,host,0,0,-1);check((Native.editorFlags(host.session)&4)==0&&host.preview.text.isEmpty(),"selection reset renders");
            host.fresh(1);host.press("n");host.press("i");host.info.imeOptions=6;host.press("確定/↵");check(host.editor.before.equals("に"),"software click uses captured editor");host.press("確定/↵");check(host.editor.calls.contains("action:6"),"click reads IME options");
            host.fresh(0x81);count=host.connections;check(!host.key('a'),"private direct hardware passes to superclass");check(host.connections-count==1,"private connection captured once");check(host.editor.calls.stream().noneMatch(call->call.startsWith("before:")||call.startsWith("after:")),"private field has no context reads");
            host.fresh(1);host.onConnection=()->host.fresh(0x81);check(host.key('a'),"superseded hardware event consumed to avoid replay");check(host.editor.before.isEmpty()&&Native.editorFlags(host.session)==3,"connection getter replacement protected");
            host.fresh(1);host.onInfo=()->host.fresh(0x81);count=host.connections;Native.serviceClick(host.session,host,0,'a');check(host.connections==count&&host.editor.before.isEmpty(),"info getter replacement stops connection read and click");
            host.fresh(1);host.editor.onRead=()->host.fresh(0x81);check(host.key('a'),"read callback replacement consumed");check(host.editor.before.isEmpty(),"no new-field write");
            host.fresh(1);host.type("a");host.onConnection=()->host.fresh(0x81);Native.serviceFinish(host.session,host);check(Native.editorFlags(host.session)==3&&host.editor.before.isEmpty(),"finish getter replacement cannot finish new field");
            host.fresh(1);host.type("a");host.onConnection=()->host.fresh(0x81);Native.serviceSelection(host.session,host,0,0,-1);check(Native.editorFlags(host.session)==3,"selection getter replacement protected");
            host.fresh(1);host.editor=null;check(!host.key('a'),"missing connection passes through");check(Native.composingText(host.session).isEmpty(),"missing connection cannot advance core");
            host.fresh(1);host.failConnection=true;check(!host.key('a'),"getter exception fails safely");host.failConnection=false;check(host.key('a'),"getter exception cleared");
            Native.destroy(host.session);count=host.connections;check(!host.key('a'),"destroyed key no-op");Native.serviceClick(host.session,host,0,'a');check(host.connections==count&&Native.serviceView(host.session,host)==null,"destroyed service has no reads/view");
            System.out.println("Android JNI service events: hardware/soft keys, one editor snapshot, view recreation, finish/selection, privacy, missing/failed getters, reentrant field changes and destruction passed");
        } finally {
            Native.destroy(host.session);
            try(var paths=Files.walk(profile)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
