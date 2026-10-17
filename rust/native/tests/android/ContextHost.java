// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
public final class ContextHost {
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-context-");
        long session=Native.create(root.resolve("native").toString());
        long reference=Native.create(root.resolve("reference").toString());
        check(session!=0&&reference!=0,"sessions");
        try {
            for(boolean hardware:new boolean[]{false,true}) {
                Native.startInput(session,1,0);Native.startInput(reference,1,0);
                Native.key(session,0,'k',0);Native.key(reference,0,'k',0);
                EditorHost.Editor editor=new EditorHost.Editor();editor.before="前😀k";editor.pending="k";editor.after="後";
                String actual=Native.editorKey(session,editor,hardware?29:0x41,'a',0,hardware,false,"k");
                Native.canDelete(reference,true);
                String expected=Native.keyContext(reference,0x41,'a',0,"前😀","後");
                check(actual.equals(expected),actual+"\n"+expected);
                check(editor.calls.equals(List.of("before:0","after:4000","before:4000")),editor.calls);
            }
            EditorHost.Editor editor=new EditorHost.Editor();editor.before="秘密";
            Native.startInput(session,0x81,0);
            Native.editorKey(session,editor,29,'a',0,true,true,"");
            Native.editorUndo(session,editor,true,"");
            check(editor.calls.isEmpty(),"private editor must never be queried: "+editor.calls);
            Native.editorKey(session,editor,59,0,1,true,false,"");
            check(editor.calls.isEmpty(),"modifier-only event must not query context");
            Native.startInput(session,1,0);editor=new EditorHost.Editor();editor.throwsRead=true;
            Native.editorKey(session,editor,29,'a',0,true,false,"");
            check(editor.calls.equals(List.of("before:0")),editor.calls);
            // A subsequent JNI call proves the thrown getter was cleared.
            Native.startInput(session,1,0);
            Native.editorKey(session,null,29,'a',0,true,false,"");
            editor=new EditorHost.Editor();
            Native.startInput(session,1,0);
            for(int scalar:"konnichiha".codePoints().toArray()) {
                String reply=Native.editorKey(session,editor,0,scalar,0,false,false,editor.pending);
                Native.applyEditor(session,editor,reply,editor.pending);
            }
            Native.applyEditor(session,editor,Native.commit(session),editor.pending);
            check(editor.before.equals("こんにちは"),editor.before);
            String undo=Native.editorUndo(session,editor,false,"");
            check(undo.contains("\"deleteBefore\":5")&&undo.contains("こんにちは"),undo);
            Native.applyEditor(session,editor,undo,"");
            check(!editor.pending.isEmpty(),"undo restored composition");
            editor=new EditorHost.Editor();editor.unsupported=true;
            check(Native.editorUndo(session,editor,false,"").contains("\"consumed\":false"),"unsupported undo");
            editor=new EditorHost.Editor();
            check(Native.softwareFallback(editor,0,0x1f600,0),"scalar fallback");
            check(editor.before.equals("😀"),editor.before);
            check(Native.softwareFallback(editor,8,0,0)&&editor.before.isEmpty(),"codepoint delete");
            editor.calls.clear();editor.reject=true;
            check(!Native.softwareFallback(editor,0x25,0,0),"refused key pair");
            check(editor.calls.equals(List.of("event:0:21","event:1:21")),editor.calls);
            editor.calls.clear();Native.softwareFallback(editor,0x27,0,0);
            check(editor.calls.equals(List.of("event:0:22","event:1:22")),editor.calls);
            editor=new EditorHost.Editor();Native.softwareFallback(editor,13,0,6);
            check(editor.calls.equals(List.of("action:6")),editor.calls);
            for(int options:new int[]{0,1,0x40000006}) {
                editor=new EditorHost.Editor();Native.softwareFallback(editor,13,0,options);
                check(editor.before.equals("\n"),editor.calls);
            }
            editor=new EditorHost.Editor();
            check(!Native.softwareFallback(editor,0,0xd800,0)&&editor.calls.isEmpty(),"invalid scalar");
            check(!Native.softwareFallback(null,8,0,0),"missing editor");
            System.out.println("Android JNI context/fallback: privacy, composition stripping, read failure, hardware/software context, real-core undo, Unicode fallback/delete, arrows and editor actions passed");
        } finally {
            Native.destroy(session);Native.destroy(reference);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
