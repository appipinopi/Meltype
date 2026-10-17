// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package android.content;
public final class Intent {
    public final Context context;
    public final Class<?> target;
    public int flags;
    public Intent(Context context,Class<?> target){this.context=context;this.target=target;}
    public Intent addFlags(int flags){this.flags|=flags;return this;}
}
