// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;
import android.content.Context;
import android.view.View;
import android.widget.*;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
/** Actual JNI widget construction against SDK-shaped test classes. */
public final class WidgetHost extends Context {
    private TextView preview;
    private LinearLayout candidates;
    private boolean throwCallback;
    private final List<String> clicks=new ArrayList<>();
    private View.OnClickListener keyboardCallback(int action,int value) {
        if(throwCallback)throw new IllegalStateException("test callback failure");
        return view->clicks.add(action+":"+value);
    }
    static void check(boolean value,Object detail){if(!value)throw new AssertionError(detail);}
    static void checkButton(Button button,String label) {
        check(button.text.equals(label),button.text);
        LinearLayout.LayoutParams params=(LinearLayout.LayoutParams)button.layoutParams;
        check(params.width==0&&params.height==-2&&params.weight==1f,"button weights");
        check(button.minWidth==0&&button.minimumWidth==0&&Arrays.equals(button.padding,new int[]{1,1,1,1}),"button dimensions");
        check(button.performClick(),"listener installed");
    }
    public static void main(String[] args) {
        WidgetHost service=new WidgetHost();
        String[] rows={"qwertyuiop","asdfghjkl","zxcvbnm","1234567890","-.,!?@/"};
        for(int iteration=0;iteration<20;iteration++) {
            service.clicks.clear();LinearLayout root=(LinearLayout)Native.createInputView(service);
            check(root!=null&&root.children.size()==9&&root.orientation==1,"root hierarchy");
            check(root.children.get(0)==service.preview&&service.preview.textSize==20f,"preview field and size");
            HorizontalScrollView scroll=(HorizontalScrollView)root.children.get(1);
            check(scroll.children.size()==1&&scroll.children.get(0)==service.candidates&&service.candidates.orientation==0,"candidate scroll and field");
            List<String> expected=new ArrayList<>();
            for(int rowIndex=0;rowIndex<rows.length;rowIndex++) {
                LinearLayout row=(LinearLayout)root.children.get(rowIndex+2);
                check(row.orientation==0&&row.children.size()==rows[rowIndex].length(),"letter row");
                for(int i=0;i<rows[rowIndex].length();i++) {
                    char letter=rows[rowIndex].charAt(i);checkButton((Button)row.children.get(i),String.valueOf(letter));
                    expected.add("0:"+(int)letter);
                }
            }
            String[][] labels={{"⇧","あ/A","⌫","変換/空白","確定/↵","取消","🌐"},{"←","→","縮める","伸ばす","戻す","再変換","設定"}};
            String[][] actions={{"1:0","2:0","3:8","3:32","3:13","3:27","4:0"},{"3:37","3:39","5:0","6:0","7:0","8:0","9:0"}};
            for(int group=0;group<2;group++) {
                LinearLayout row=(LinearLayout)root.children.get(7+group);
                check(row.children.size()==7&&row.orientation==0,"action row");
                for(int i=0;i<7;i++){checkButton((Button)row.children.get(i),labels[group][i]);expected.add(actions[group][i]);}
            }
            check(service.clicks.equals(expected),service.clicks);
        }
        TextView previous=service.preview;LinearLayout previousCandidates=service.candidates;
        service.throwCallback=true;
        check(Native.createInputView(service)==null,"construction exception cleared");
        check(service.preview==previous&&service.candidates==previousCandidates,"failed view must not publish partial fields");
        service.throwCallback=false;
        check(Native.createInputView(service)!=null,"construction recovers");
        service.clicks.clear();
        String reply="{\"consumed\":true,\"composingText\":\"かな😀\",\"view\":{\"candidates\":[\"仮名\",\"かな\"],\"predictions\":[\"かなしい\"]}}";
        check(Native.renderInputView(service,reply),"render consumed result");
        check(service.preview.text.equals("かな😀")&&service.candidates.children.size()==3,"preview/candidate rendering");
        String[] choices={"仮名","かな","かなしい"};
        for(int i=0;i<3;i++) {
            Button button=(Button)service.candidates.children.get(i);
            check(button.text.equals(choices[i])&&button.layoutParams==null&&button.minWidth==64,"candidate default styling");
            check(button.performClick(),"candidate listener");
        }
        check(service.clicks.equals(List.of("10:0","10:1","11:0")),service.clicks);
        check(!Native.renderInputView(service,"{\"consumed\":true,\"composingText\":\"invalid\",\"view\":{\"candidates\":[3]}}"),"invalid render data");
        check(service.preview.text.equals("かな😀")&&service.candidates.children.size()==3,"invalid data must not mutate widgets");
        service.throwCallback=true;
        check(Native.renderInputView(service,reply),"widget failure must preserve already consumed key");
        service.throwCallback=false;
        check(Native.renderInputView(service,reply),"render recovers after callback exception");
        StringBuilder many=new StringBuilder("{\"consumed\":false,\"composingText\":\"long\",\"view\":{\"candidates\":[");
        for(int i=0;i<200;i++){if(i>0)many.append(',');many.append('"').append(i).append('"');}
        many.append("]}}");
        check(!Native.renderInputView(service,many.toString())&&service.candidates.children.size()==200,"large candidate render and JNI refs");
        service.clicks.clear();service.candidates.children.get(199).performClick();
        check(service.clicks.equals(List.of("10:199")),service.clicks);
        check(!Native.renderInputView(service,"{\"consumed\":false,\"composingText\":\"\",\"view\":null}"),"empty view consumption");
        check(service.preview.text.isEmpty()&&service.candidates.children.isEmpty(),"clear view");
        check(!Native.renderInputView(service,null),"null reply exception cleared");
        check(Native.createInputView(null)==null,"missing service");
        System.out.println("Android JNI widgets: hierarchy, all 57 labels/callbacks, row weights, preview/candidates/predictions, repeated builds, 200-candidate refs and UI failure consumption passed (SDK-shaped host, not pixel rendering)");
    }
}
