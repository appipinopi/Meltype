// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import java.nio.file.Files;
import java.nio.file.Path;

/** Compare migrated events to the former Java mapping through the real core. */
public final class InputHost {
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    // Reference table retained from MeltypeService at bd4682f. It remains test
    // input, independent of the production Rust key policy.
    static int previousVk(int key) {
        return switch(key) {
            case 67 -> 8; case 66 -> 13; case 62 -> 32; case 111 -> 27;
            case 19 -> 38; case 20 -> 40; case 21 -> 37; case 22 -> 39;
            case 61 -> 9; case 136 -> 0x75; case 137 -> 0x76;
            case 138 -> 0x77; case 139 -> 0x78; case 140 -> 0x79;
            case 69 -> 0xbd; case 70 -> 0xbb; case 71 -> 0xdb; case 72 -> 0xdd;
            case 73 -> 0xdc; case 74 -> 0xba; case 75 -> 0xde; case 68 -> 0xc0;
            case 55 -> 0xbc; case 56 -> 0xbe; case 76 -> 0xbf;
            default -> key>=29&&key<=54 ? 0x41+key-29 : key>=7&&key<=16 ? 0x30+key-7 : 0;
        };
    }
    public static void main(String[] args)throws Exception {
        Path root=Files.createTempDirectory("meltype-jni-input-");
        long nativeSession=Native.create(root.resolve("native").toString());
        long reference=Native.create(root.resolve("reference").toString());
        check(nativeSession!=0&&reference!=0,"sessions");
        try {
            for(int type:new int[]{0x81,0x91,0xe1,0x12})check(Native.startInput(nativeSession,type,0)==3,"password:"+type);
            for(int type:new int[]{2,3,4})check(Native.startInput(nativeSession,type,0)==1,"direct:"+type);
            check(Native.startInput(nativeSession,1,0x01000000)==2,"no learning");
            check(Native.startInput(nativeSession,1,0)==0,"normal");
            // Switching fields drops the previous composition and restores mode.
            Native.androidKey(nativeSession,39,'k',0,"","");
            Native.startInput(nativeSession,0x81,0);
            String result=Native.androidKey(nativeSession,29,'a',0,"","");
            check(result.contains("\"consumed\":false")&&result.contains("\"view\":null"),result);
            Native.startInput(nativeSession,1,0x01000000);
            result=Native.androidKey(nativeSession,29,'a',0,"","");
            check(result.contains("\"consumed\":false"),result);
            Native.startInput(nativeSession,1,0);
            result=Native.androidKey(nativeSession,29,'a',0,"","");
            check(result.contains("\"consumed\":true")&&!result.contains("\"view\":null"),result);
            check(Native.commit(nativeSession).contains("\"consumed\":true"),"normal field restored");
            for(int key:new int[]{57,58,59,60,63,78,113,114,117,118,119}) {
                check(Native.isModifierKey(key),key);
                check(Native.androidKey(nativeSession,key,0,0,"","").contains("\"consumed\":false"),key);
            }
            for(int key:new int[]{115,116,143,29,0,-1,999})check(!Native.isModifierKey(key),key);
            int comparisons=0;
            for(int key=0;key<=150;key++) {
                if(Native.isModifierKey(key))continue;
                for(int mods:new int[]{0,1,2,4,8,15}) {
                    int meta=((mods&1)!=0?1:0)|((mods&2)!=0?0x1000:0)|((mods&4)!=0?2:0)|((mods&8)!=0?0x10000:0);
                    Native.startInput(nativeSession,1,0);
                    Native.startInput(reference,1,0);
                    for(int scalar:"nihongo".codePoints().toArray()) {
                        Native.key(nativeSession,0,scalar,0);Native.key(reference,0,scalar,0);
                    }
                    int scalar=key>=29&&key<=54?'a'+key-29:key>=7&&key<=16?'0'+key-7:0;
                    String actual=Native.androidKey(nativeSession,key,scalar,meta,"周辺", "after");
                    String expected=Native.keyContext(reference,previousVk(key),scalar,mods,"周辺", "after");
                    check(actual.equals(expected),"key="+key+" mods="+mods+"\n"+actual+"\n"+expected);
                    comparisons++;
                }
            }
            System.out.println("Android JNI input: "+comparisons+" former Java mapping/core comparisons, privacy/direct policies, modifiers and field transitions passed");
        } finally {
            Native.destroy(nativeSession);Native.destroy(reference);
            try(var paths=Files.walk(root)){for(Path path:paths.sorted(java.util.Comparator.reverseOrder()).toList())Files.delete(path);}
        }
    }
}
