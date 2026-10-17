// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package android.view;
import android.content.Context;
public class View {
    public interface OnClickListener {void onClick(View view);}
    public final Context context;
    public ViewGroup.LayoutParams layoutParams;
    public int minimumWidth=64;
    public int[] padding={0,0,0,0};
    private OnClickListener listener;
    private Object tag;
    public void setTag(Object tag){this.tag=tag;}
    public Object getTag(){return tag;}
    public View(Context context){this.context=context;}
    public void setOnClickListener(OnClickListener listener){this.listener=listener;}
    public boolean performClick(){if(listener==null)return false;listener.onClick(this);return true;}
    public void setMinimumWidth(int value){minimumWidth=value;}
    public void setPadding(int left,int top,int right,int bottom){padding=new int[]{left,top,right,bottom};}
}
