// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
public final class LifecycleHost {
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    static void type(long session,EditorHost.Editor editor,String text) {
        for(int scalar:text.codePoints().toArray()) {
            String reply=Native.editorKey(session,editor,0,scalar,0,false,false,editor.pending);
            Native.applyEditor(session,editor,reply,editor.pending);
        }
    }
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-lifecycle-");long session=Native.create(root.toString());
        check(session!=0,"session");
        try {
            Native.startInput(session,1,0);EditorHost.Editor editor=new EditorHost.Editor();
            type(session,editor,"konnnichiha");editor.calls.clear();
            String result=Native.finishInput(session,editor,true,editor.pending);
            check(editor.before.equals("こんにちは")&&editor.pending.isEmpty(),editor.before);
            check(result.contains("\"composing\":false")&&result.contains("\"view\":null"),result);
            check(editor.calls.equals(List.of("begin","commit:こんにちは","finish","end")),editor.calls);
            editor.calls.clear();Native.finishInput(session,editor,false,"");
            check(editor.calls.isEmpty()&&editor.before.equals("こんにちは"),editor.calls);
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"nihongo");
            editor.calls.clear();String before=editor.before;
            check(!Native.selectionChanged(session,editor,true,5,5,5),"caret stayed at composing end");
            check(!Native.selectionChanged(session,editor,false,0,1,5),"not composing");
            check(editor.calls.isEmpty(),editor.calls);
            check(Native.selectionChanged(session,editor,true,0,1,5),"selection changed");
            check(editor.before.equals(before)&&editor.calls.equals(List.of("finish")),editor.calls);
            check(!Native.commit(session).contains("にほんご"),"selection reset must not insert again");
            editor=new EditorHost.Editor();editor.throwsFinish=true;
            check(Native.selectionChanged(session,editor,true,0,0,1),"finish exception handled");
            check(Native.selectionChanged(session,null,true,0,0,1),"missing editor still resets");
            editor=new EditorHost.Editor();final EditorHost.Editor reentrant=editor;
            editor.onFinish=()->{for(int i=0;i<2;i++)check(!Native.selectionChanged(session,reentrant,true,0,0,1),"reentrant selection must not repeat finish");};
            check(Native.selectionChanged(session,editor,true,0,0,1),"outer selection");
            check(editor.calls.equals(List.of("finish")),editor.calls);
            editor.calls.clear();editor.onFinish=()->check(Native.finishInput(session,reentrant,true,"").contains("\"consumed\":false"),"nested close");
            Native.finishInput(session,editor,true,"");
            check(editor.calls.equals(List.of("begin","finish","end")),editor.calls);
            Native.startInput(session,1,0);editor=new EditorHost.Editor();type(session,editor,"nihongo");
            editor.reject=true;String unchanged=editor.before;editor.calls.clear();
            result=Native.finishInput(session,editor,true,editor.pending);
            check(editor.before.equals(unchanged)&&result.contains("\"composing\":false"),result);
            check(editor.calls.get(editor.calls.size()-1).equals("end"),editor.calls);
            check(!Native.commit(session).contains("にほんご"),"rejected final write still retires core");
            Native.startInput(session,1,0);type(session,new EditorHost.Editor(),"nihongo");
            Native.finishInput(session,null,true,"");
            check(!Native.commit(session).contains("にほんご"),"missing final editor retires core");
            Native.startInput(session,1,0);editor=new EditorHost.Editor();
            check(Native.editorReconvert(session,editor,true,false)==null&&editor.calls.isEmpty(),"private reconvert");
            check(Native.editorReconvert(session,editor,false,true)==null&&editor.calls.isEmpty(),"composing reconvert");
            check(Native.editorReconvert(session,editor,false,false)==null,"empty selection");
            editor.selected="\ud800";check(Native.editorReconvert(session,editor,false,false)==null,"unpaired selection");
            editor.selected="橋";result=Native.editorReconvert(session,editor,false,false);
            check(result.contains("\"selectionExpect\":\"橋\"")&&result.contains("\"consumed\":true"),result);
            Native.reset(session);editor.selected="未登録の漢字列";
            result=Native.editorReconvert(session,editor,false,false);
            check(result!=null&&result.contains("\"consumed\":false"),result);
            result=Native.applyEditor(session,editor,null,"");
            check(result.contains("\"consumed\":false"),"null reply exception cleared");
            System.out.println("Android JNI lifecycle: final commit/reset, repeated close, caret/selection, reentrant callbacks, rejection/missing editor, reconversion guards and JNI exception cleanup passed");
        } finally {
            Native.destroy(session);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
