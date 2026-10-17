// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
final class Native {
    static { System.loadLibrary("meltype_native"); }
    static native long create(String directory);
    static native String control(long handle,String request);
    static native String predict(long handle,int index);
    static native void destroy(long handle);
    static native String key(long handle, int vk, int scalar, int modifiers);
    static native String keyContext(long handle, int vk, int scalar, int modifiers, String before, String after);
    static native String reconvert(long handle, String text);
    static native String undo(long handle, String before);
    static native String commit(long handle);
    static native String reset(long handle);
    static native String select(long handle, int index);
    static native void direct(long handle, boolean enabled);
    static native void privateInput(long handle, boolean enabled);
    static native void canDelete(long handle, boolean enabled);
    static native String applyEditor(long handle, Object connection, String reply, String composingText);
}
