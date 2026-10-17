// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
// Test-only Android SDK-shaped event for desktop JVM JNI constructor dispatch.
package android.view;
public final class KeyEvent {
    public final int action,keyCode;
    public int scalar,meta;
    public int getUnicodeChar(){return scalar;}
    public int getMetaState(){return meta;}
    public KeyEvent(int action,int keyCode){this.action=action;this.keyCode=keyCode;}
}
