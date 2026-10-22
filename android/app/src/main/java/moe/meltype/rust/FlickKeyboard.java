// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 藤島
package moe.meltype.rust;

import android.content.Context;
import android.view.MotionEvent;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;

/** Touch input only; kana still pass through the native composition/editor path. */
final class FlickKeyboard {
    interface Listener {
        void key(int scalar);
        void variant(int kind);
        void layout(boolean flick);
    }
    // Center, left, up, right, down. Repeated taps always enter the center kana.
    static final String[] GROUPS={"あいうえお","かきくけこ","さしすせそ",
        "たちつてと","なにぬねの","はひふへほ","まみむめも","や（ゆ）よ",
        "らりるれろ","","わをんー～","、。？！…"};
    private final List<Gesture> gestures=new ArrayList<>();
    private final List<View> romanRows=new ArrayList<>();
    private final LinearLayout panel;
    private final Button toggle;
    private final Listener listener;
    private boolean flick;

    static FlickKeyboard attach(Context context,LinearLayout root,Listener listener,boolean flick) {
        return new FlickKeyboard(context,root,listener,flick);
    }
    private FlickKeyboard(Context context,LinearLayout root,Listener listener,boolean flick) {
        this.listener=listener;
        for(int i=2;i<7;i++)romanRows.add(root.getChildAt(i));
        panel=new LinearLayout(context);panel.setOrientation(LinearLayout.VERTICAL);
        for(int r=0;r<4;r++) {
            LinearLayout row=new LinearLayout(context);row.setBaselineAligned(false);
            for(int c=0;c<3;c++) {
                String group=GROUPS[r*3+c];Button key=new Button(context);
                key.setMinWidth(0);key.setMinimumWidth(0);
                key.setMinHeight(Math.round(64*context.getResources().getDisplayMetrics().density));
                key.setTextSize(16);key.setPadding(1,1,1,1);
                if(group.isEmpty()) {
                    key.setText("゛゜小");key.setContentDescription("直前のかなの濁点・半濁点・小書きを切替");
                    key.setOnClickListener(view->listener.variant(0));
                } else {
                    Gesture gesture=new Gesture(key,group,24*context.getResources().getDisplayMetrics().density);
                    gestures.add(gesture);key.setOnTouchListener(gesture);key.setOnClickListener(gesture);
                }
                row.addView(key,new LinearLayout.LayoutParams(0,-2,1));
            }
            panel.addView(row);
        }
        root.addView(panel,2);
        toggle=new Button(context);toggle.setOnClickListener(view->{show(!this.flick);listener.layout(this.flick);});
        root.addView(toggle);
        show(flick);
    }
    private void show(boolean enabled) {
        cancelGestures();flick=enabled;
        for(View row:romanRows)row.setVisibility(enabled?View.GONE:View.VISIBLE);
        panel.setVisibility(enabled?View.VISIBLE:View.GONE);
        toggle.setText(enabled?"QWERTYに切替":"かなフリックに切替");
    }
    void cancelGestures(){for(Gesture gesture:gestures)gesture.cancel();}
    static int direction(float dx,float dy,float threshold) {
        if(Math.max(Math.abs(dx),Math.abs(dy))<threshold)return 0;
        if(Math.abs(dx)>Math.abs(dy))return dx<0?1:3;
        return dy<0?2:4;
    }
    private final class Gesture implements View.OnTouchListener,View.OnClickListener {
        private final Button key;
        private final String group,label;
        private final float threshold;
        private float x,y;
        private int pointer=-1,choice;
        Gesture(Button key,String group,float threshold) {
            this.key=key;this.group=group;this.threshold=threshold;
            label="↑"+group.charAt(2)+"\n"+group.charAt(1)+"  "+group.charAt(0)+"  "+group.charAt(3)+"\n↓"+group.charAt(4);
            key.setText(label);
            key.setContentDescription(group.charAt(0)+"：左 "+group.charAt(1)+"、上 "+group.charAt(2)+"、右 "+group.charAt(3)+"、下 "+group.charAt(4));
        }
        void cancel(){pointer=-1;choice=0;key.setPressed(false);key.setText(label);}
        @Override public void onClick(View view){int index=choice;choice=0;listener.key(group.charAt(index));}
        @Override public boolean onTouch(View view,MotionEvent event) {
            int action=event.getActionMasked();
            if(action==MotionEvent.ACTION_CANCEL||event.getPointerCount()!=1){cancel();return true;}
            if(action==MotionEvent.ACTION_DOWN){pointer=event.getPointerId(0);x=event.getX();y=event.getY();choice=0;key.setPressed(true);return true;}
            if(pointer<0||event.getPointerId(0)!=pointer)return true;
            choice=direction(event.getX()-x,event.getY()-y,threshold);
            if(action==MotionEvent.ACTION_MOVE){key.setText(String.valueOf(group.charAt(choice)));return true;}
            if(action==MotionEvent.ACTION_UP){pointer=-1;key.setPressed(false);key.setText(label);key.performClick();}
            return true;
        }
    }
}
