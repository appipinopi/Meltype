// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import java.nio.file.Files;
import java.nio.file.Path;
public final class StateHost {
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    static void type(long session,EditorHost.Editor editor,String text) {
        for(int scalar:text.codePoints().toArray()) {
            String reply=Native.editorKey(session,editor,0,scalar,0,false,(Native.editorFlags(session)&2)!=0,Native.composingText(session));
            Native.applyEditor(session,editor,reply,Native.composingText(session));
            check(Native.composingText(session).equals(editor.pending),"native composition matches host");
        }
    }
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-state-");
        long session=Native.create(root.resolve("first").toString()),second=Native.create(root.resolve("second").toString());
        check(session!=0&&second!=0,"sessions");
        try {
            check(Native.editorFlags(session)==0&&Native.composingText(session).isEmpty(),"initial native state");
            Native.toggleShift(session);check((Native.editorFlags(session)&8)!=0,"shift owned natively");
            check(Native.editorFlags(second)==0,"independent shift");
            check(Native.takeShiftedScalar(session,'a')=='A'&&Native.takeShiftedScalar(session,'a')=='a',"one-shot shift");
            Native.toggleShift(session);check(Native.takeShiftedScalar(session,'1')=='1'&&(Native.editorFlags(session)&8)==0,"nonletter consumes shift");
            Native.toggleDirect(session);check((Native.editorFlags(session)&1)==1,"direct enabled");
            check(Native.key(session,0,'k',0).contains("\"consumed\":false"),"core direct mode synchronized");
            Native.toggleDirect(session);check((Native.editorFlags(session)&1)==0,"direct disabled");
            Native.toggleShift(session);Native.startInput(session,0x81,0);
            check(Native.editorFlags(session)==3,"field start resets shift and sets privacy/direct");
            Native.startInput(session,1,0);EditorHost.Editor editor=new EditorHost.Editor();type(session,editor,"nihongo");
            check((Native.editorFlags(session)&4)!=0,"native composing flag");
            editor.onFinish=()->check((Native.editorFlags(session)&4)==0&&Native.composingText(session).isEmpty(),"state detached before callback");
            Native.selectionChanged(session,editor,true,0,0,3);
            check(Native.composingText(session).isEmpty(),"selection cleared state");
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"konnichiha");
            editor.onFinish=()->{Native.startInput(session,1,0);type(session,new EditorHost.Editor(),"nihongo");};
            Native.selectionChanged(session,editor,true,0,0,3);
            check((Native.editorFlags(session)&4)!=0&&Native.editorView(session).contains(Native.composingText(session)),"selection renderer must use the newer field view");
            for(boolean fail:new boolean[]{false,true}) {
                Native.startInput(session,1,0);EditorHost.Editor old=new EditorHost.Editor();old.throwsFinish=fail;
                String[] newerText={""};
                old.onCommit=()->{Native.startInput(session,1,0);type(session,new EditorHost.Editor(),"nihongo");newerText[0]=Native.composingText(session);};
                String outcome=Native.applyEditor(session,old,"{\"consumed\":true,\"commits\":[{\"text\":\"old\"},{\"text\":\"extra\"}],\"view\":null}","");
                check((Native.editorFlags(session)&4)!=0&&Native.composingText(session).equals(newerText[0]),"old reply must not overwrite a newer field: fail="+fail+" flags="+Native.editorFlags(session)+" pending="+Native.composingText(session)+" outcome="+outcome);
                check(outcome.contains(newerText[0])&&outcome.contains("\"consumed\":true"),outcome);
                check(old.before.equals("old")&&!old.calls.contains("commit:extra")&&old.calls.get(old.calls.size()-1).equals("end"),old.calls);
                check(Native.commit(session).contains("にほんご"),"old failure must not reset new core");
            }
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"konnichiha");
            String[] newerText={""};
            editor.onFinish=()->{Native.startInput(session,1,0);type(session,new EditorHost.Editor(),"nihongo");newerText[0]=Native.composingText(session);};
            String outcome=Native.finishInput(session,editor,true,Native.composingText(session));
            check(Native.composingText(session).equals(newerText[0])&&outcome.contains(newerText[0]),"old close must not retire new input");
            Native.reset(session);check((Native.editorFlags(session)&4)==0&&Native.composingText(session).isEmpty(),"explicit reset");
            Native.privateInput(session,true);check((Native.editorFlags(session)&2)!=0,"legacy privacy ABI synchronized");
            Native.privateInput(session,false);check((Native.editorFlags(session)&2)==0,"privacy disabled");
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"konnichiha");
            Native.toggleInputMode(session,editor);
            check(editor.before.equals("こんにちは")&&(Native.editorFlags(session)&1)==1&&Native.composingText(session).isEmpty(),"mode toggle commits before changing mode");
            Native.toggleInputMode(session,editor);check((Native.editorFlags(session)&1)==0,"mode restored");
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"nihongo");
            editor.onCommit=()->Native.startInput(session,0x81,0);
            Native.toggleInputMode(session,editor);
            check(Native.editorFlags(session)==3,"old mode action must not toggle the new private field");
            Native.startInput(second,1,0);type(second,new EditorHost.Editor(),"nihongo");
            EditorHost.Editor destroyed=new EditorHost.Editor();destroyed.onCommit=()->Native.destroy(second);
            outcome=Native.applyEditor(second,destroyed,"{\"consumed\":true,\"commits\":[{\"text\":\"x\"}],\"view\":null}","");
            check(Native.editorFlags(second)==0&&Native.composingText(second).isEmpty(),"callback destruction");
            check(outcome.contains("\"consumed\":true"),"completed edit must not replay after destruction: "+outcome);
            System.out.println("Android JNI owned state: mode/shift/privacy, composition, independent sessions, field generations, reentrant replacement/close/failure and destruction passed");
        } finally {
            Native.destroy(session);Native.destroy(second);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
