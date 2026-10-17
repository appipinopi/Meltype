// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package android.content;
public final class Intent {
    public final Context context;
    public final Class<?> target;
    public int flags;
    public String action,type,category,title;
    public android.net.Uri data;
    public Intent(Context context,Class<?> target){this.context=context;this.target=target;}
    public Intent(String action){this.context=null;this.target=null;this.action=action;}
    public Intent setType(String type){this.type=type;return this;}
    public Intent addCategory(String category){this.category=category;return this;}
    public Intent putExtra(String name,String value){if(!name.equals("android.intent.extra.TITLE"))throw new AssertionError(name);title=value;return this;}
    public android.net.Uri getData(){return data;}
    public Intent addFlags(int flags){this.flags|=flags;return this;}
}
