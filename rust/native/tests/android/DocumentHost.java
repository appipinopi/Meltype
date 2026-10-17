// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.content.ContentResolver;
import android.net.Uri;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.regex.*;
public final class DocumentHost {
    final Path cache;
    final ContentResolver resolver=new ContentResolver();
    public DocumentHost(Path cache){this.cache=cache;}
    public File getCacheDir(){return cache.toFile();}
    public ContentResolver getContentResolver(){return resolver;}
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
            ok(Native.control(session,"{\"op\":\"dictionary_set\",\"reading\":\"めるたいぷ\",\"word\":\"Meltype😀\"}"));
            Output output=new Output();host.resolver.output=output;ok(host.transfer(session,"dictionary_export"));check(output.closed==1,"export closed");
            byte[] dictionary=output.toByteArray();check(new String(dictionary,StandardCharsets.UTF_16).contains("Meltype😀"),"UTF-16 dictionary export");host.empty();
            ok(Native.control(session,"{\"op\":\"dictionary_remove\",\"reading\":\"めるたいぷ\"}"));
            Input input=new Input(dictionary);host.resolver.input=input;check(ok(host.transfer(session,"dictionary_import")).contains("\"page\":1"),"reload dictionary page");
            check(input.closed==1,"import closed");check(Native.control(session,"{\"op\":\"dictionary_list\"}").contains("Meltype😀"),"import saved");host.empty();
            output=new Output();host.resolver.output=output;ok(host.transfer(session,"backup_create"));byte[] backup=output.toByteArray();host.empty();
            ok(Native.control(session,"{\"op\":\"dictionary_remove\",\"reading\":\"めるたいぷ\"}"));
            host.resolver.input=new Input(backup);long first=token(ok(host.transfer(session,"backup_restore")));
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
