// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package android.widget;
import android.content.Context;
import android.view.ViewGroup;
public class LinearLayout extends ViewGroup {
    public int orientation;
    public static class LayoutParams extends ViewGroup.LayoutParams {
        public float weight;
        public LayoutParams(int width,int height,float weight){super(width,height);this.weight=weight;}
    }
    public LinearLayout(Context context){super(context);}
    public void setOrientation(int value){orientation=value;}
}
