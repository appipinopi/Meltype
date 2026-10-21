// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.content.ContentResolver;
import android.content.Intent;
import android.net.Uri;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.regex.*;
public final class DocumentHost {
    final Path cache;
    final ContentResolver resolver=new ContentResolver();
    Intent launched;int launches;boolean failLaunch;
    public DocumentHost(Path cache){this.cache=cache;}
    public File getCacheDir(){return cache.toFile();}
    public ContentResolver getContentResolver(){return resolver;}
    public void startActivityForResult(Intent intent,int request) {
        check(request==1,"document request code");if(failLaunch)throw new IllegalStateException("picker unavailable");
        launches++;launched=intent;
    }
    static class Input extends ByteArrayInputStream {
        int closed;boolean failClose;Runnable callback;
        Input(byte[] data){super(data);}
        @Override public synchronized int read(byte[] data,int start,int count){if(callback!=null){Runnable call=callback;callback=null;call.run();}return super.read(data,start,count);}
        @Override public void close()throws IOException{closed++;if(failClose)throw new IOException("input close failed");super.close();}
    }
    static class Output extends ByteArrayOutputStream {
        int closed;boolean failClose,failWrite;
        @Override public synchronized void write(byte[] data,int offset,int length){if(failWrite)throw new IllegalStateException("output write failed");super.write(data,offset,length);}
        @Override public void close()throws IOException{closed++;if(failClose)throw new IOException("output close failed");super.close();}
    }
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    static long token(String reply){Matcher m=Pattern.compile("\"token\":(\\d+)").matcher(reply);check(m.find(),reply);return Long.parseLong(m.group(1));}
    static String ok(String reply){check(reply!=null&&!reply.contains("\"error\":"),reply);return reply;}
    static void error(String reply){check(reply!=null&&reply.contains("\"error\":"),reply);}
    void empty()throws Exception{try(var paths=Files.list(cache)){check(paths.findAny().isEmpty(),"temporary file leaked");}}
    String transfer(long session,String operation){return Native.settingsDocument(session,this,new Uri(),operation);}
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-documents-");Path profile=Files.createDirectory(root.resolve("profile"));
        DocumentHost host=new DocumentHost(Files.createDirectory(root.resolve("cache")));long session=Native.create(profile.toString());
        try {
            check(session!=0,"session");
            for(String operation:new String[]{"dictionary_import","dictionary_export","backup_create","backup_restore"}) {
                ok(Native.settingsPick(session,host,operation));check(operation.equals(Native.pendingDocument(session)),"native pending operation");
                boolean output=operation.equals("dictionary_export")||operation.equals("backup_create");
                check(host.launched.action.equals(output?"android.intent.action.CREATE_DOCUMENT":"android.intent.action.OPEN_DOCUMENT"),"intent action");
                check(host.launched.type.equals("*/*")&&host.launched.category.equals("android.intent.category.OPENABLE"),"openable MIME");
                check(output?host.launched.title.equals(operation.equals("dictionary_export")?"meltype-dictionary.txt":"meltype-backup.json"):host.launched.title==null,"export title");
                int count=host.launches;error(Native.settingsPick(session,host,"backup_create"));check(host.launches==count,"one outstanding picker");
                ok(Native.settingsPickerResult(session,host,2,0,null));check(Native.pendingDocument(session)!=null,"unrelated activity result does not consume");
                ok(Native.settingsPickerResult(session,host,1,0,null));check(Native.pendingDocument(session)==null,"cancel clears pending operation");
            }
            host.failLaunch=true;error(Native.settingsPick(session,host,"dictionary_import"));check(Native.pendingDocument(session)==null,"launch failure clears pending operation");host.failLaunch=false;
            error(Native.settingsPick(session,host,"unknown"));check(Native.pendingDocument(session)==null,"invalid operation no launch");
            Native.restoreDocumentPicker(session,"not-an-operation");check(Native.pendingDocument(session)==null,"invalid saved state rejected");
            Native.restoreDocumentPicker(session,"dictionary_import");String saved=Native.pendingDocument(session);long recreated=Native.create(profile.toString());
            Native.restoreDocumentPicker(recreated,saved);check(saved.equals(Native.pendingDocument(recreated)),"recreated activity resumes valid operation");Native.destroy(recreated);
            ok(Native.settingsPickerResult(session,host,1,-1,new Intent("result")));check(Native.pendingDocument(session)==null,"null URI consumes pending operation");
            ok(Native.control(session,"{\"op\":\"dictionary_set\",\"reading\":\"めるたいぷ\",\"word\":\"Meltype😀\"}"));
            Output output=new Output();host.resolver.output=output;ok(host.transfer(session,"dictionary_export"));check(output.closed==1,"export closed");
            byte[] dictionary=output.toByteArray();check(new String(dictionary,StandardCharsets.UTF_16).contains("Meltype😀"),"UTF-16 dictionary export");host.empty();
            Input picked=new Input(dictionary);host.resolver.input=picked;Intent result=new Intent("result");result.data=new Uri();
            ok(Native.settingsPick(session,host,"dictionary_import"));ok(Native.settingsPickerResult(session,host,1,-1,result));check(picked.closed==1&&Native.pendingDocument(session)==null,"picker result transfers once");
            int opened=host.resolver.reads;ok(Native.settingsPickerResult(session,host,1,-1,result));check(host.resolver.reads==opened,"repeated result without request has no I/O");host.empty();
            ok(Native.control(session,"{\"op\":\"dictionary_remove\",\"reading\":\"めるたいぷ\"}"));
            Input input=new Input(dictionary);host.resolver.input=input;check(ok(host.transfer(session,"dictionary_import")).contains("\"page\":1"),"reload dictionary page");
            check(input.closed==1,"import closed");check(Native.control(session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"import saved");host.empty();
            output=new Output();host.resolver.output=output;ok(host.transfer(session,"backup_create"));byte[] backup=output.toByteArray();host.empty();
            ok(Native.control(session,"{\"op\":\"dictionary_remove\",\"reading\":\"めるたいぷ\"}"));
            host.resolver.input=new Input(backup);String inspected=ok(host.transfer(session,"backup_restore"));long first=token(inspected);
            check(inspected.contains("作成日時:")&&inspected.contains("復元対象:")&&inspected.contains("学習データ")&&!inspected.contains("\\\"files\\\""),"readable restore metadata: "+inspected);
            check(!Native.control(session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"inspection cannot restore");
            ok(Native.settingsRestore(session,first,false));host.empty();
            host.resolver.input=new Input(backup);long second=token(ok(host.transfer(session,"backup_restore")));
            host.resolver.input=new Input(backup);long third=token(ok(host.transfer(session,"backup_restore")));
            error(Native.settingsRestore(session,second,true));ok(Native.settingsRestore(session,second,false));
            ok(Native.settingsRestore(session,third,true));check(Native.control(session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"confirmed restore");host.empty();
            error(Native.settingsRestore(session,third,true));
            host.resolver.input=new Input("invalid backup".getBytes(StandardCharsets.UTF_8));error(host.transfer(session,"backup_restore"));host.empty();
            host.resolver.input=null;error(host.transfer(session,"dictionary_import"));host.empty();
            host.resolver.input=new Input(dictionary);((Input)host.resolver.input).failClose=true;error(host.transfer(session,"dictionary_import"));check(((Input)host.resolver.input).closed==1,"close error reported");host.empty();
            for(int mode=0;mode<3;mode++) {
                output=new Output();output.failWrite=mode==0;output.failClose=mode==1;host.resolver.output=mode==2?null:output;
                error(host.transfer(session,"dictionary_export"));if(mode!=2)check(output.closed==1,"failed export closes");host.empty();
            }
            final int[] closed={0};host.resolver.input=new InputStream(){int total;
                public int read(){throw new AssertionError("bulk only");}
                public int read(byte[] data,int offset,int count){total+=count;return total<=33*1024*1024?count:-1;}
                public void close(){closed[0]++;}
            };String oversized=host.transfer(session,"dictionary_import");error(oversized);check(oversized.contains("32 MiB"),oversized);check(closed[0]==1,"oversize input closes");host.empty();
            host.resolver.input=new InputStream(){public int read(){return 0;}public int read(byte[] b){return 0;}public void close(){closed[0]++;}};
            error(host.transfer(session,"dictionary_import"));check(closed[0]==2,"no-progress stream closes");host.empty();
            int reads=host.resolver.reads,writes=host.resolver.writes;error(host.transfer(session,"unknown"));check(host.resolver.reads==reads&&host.resolver.writes==writes,"invalid operation has no I/O");
            long doomed=Native.create(profile.toString());Input reentrant=new Input(dictionary);reentrant.callback=()->Native.destroy(doomed);host.resolver.input=reentrant;
            error(host.transfer(doomed,"dictionary_import"));check(reentrant.closed==1,"destroy during read closes");host.empty();
            host.resolver.input=new Input(backup);long pending=token(ok(host.transfer(session,"backup_restore")));Native.destroy(session);host.empty();error(Native.settingsRestore(session,pending,true));
            System.out.println("Android JNI documents: Unicode dictionary import/export, staged backup restore/cancel/token guards, bounded read, null/exception/close failures, reentrant destroy and temporary cleanup passed (captured resolver, real JVM streams)");
        } finally {
            Native.destroy(session);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
