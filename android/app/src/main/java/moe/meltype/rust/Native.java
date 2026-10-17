// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
final class Native {
    static { System.loadLibrary("meltype_native"); }
    static native long create(String directory);
    static native String control(long handle,String request);
    static native String createSettingsActivity(Object activity);
    static native String renderSettingsPage(Object bridge,String page);
    static native String settingsValues(Object bridge,String operation);
    static native String editSettingsFields(Object bridge,String edits);
    static native String settingsPage(long handle,int page);
    static native String settingsPick(long handle,Object activity,String operation);
    static native String settingsPickerResult(long handle,Object context,int code,int result,Object data);
    static native String pendingDocument(long handle);
    static native void restoreDocumentPicker(long handle,String operation);
    static native String settingsDocument(long handle,Object context,Object uri,String operation);
    static native String settingsRestore(long handle,long token,boolean accept);
    static native String settingsAction(long handle,String action,String fields);
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
    static native int startInput(long handle, int inputType, int imeOptions);
    static native boolean isModifierKey(int keyCode);
    static native String androidKey(long handle, int keyCode, int scalar, int metaState, String before, String after);
    static native String editorKey(long handle, Object connection, int key, int scalar, int modifiers, boolean hardware, boolean privateInput, String composingText);
    static native String editorUndo(long handle, Object connection, boolean privateInput, String composingText);
    static native boolean softwareFallback(Object connection, int key, int scalar, int imeOptions);
    static native boolean selectionChanged(long handle, Object connection, boolean composing, int start, int end, int composingEnd);
    static native String finishInput(long handle, Object connection, boolean composing, String composingText);
    static native String editorReconvert(long handle, Object connection, boolean privateInput, boolean composing);
    static native int editorFlags(long handle);
    static native String composingText(long handle);
    static native String editorView(long handle);
    static native void toggleShift(long handle);
    static native int takeShiftedScalar(long handle, int scalar);
    static native void toggleDirect(long handle);
    static native String toggleInputMode(long handle, Object connection);
    static native Object createInputView(Object service);
    static native boolean renderInputView(Object service, String reply);
    static native void keyboardAction(long handle, Object service, Object connection, int action, int value, int imeOptions);
}
