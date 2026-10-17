// SPDX-License-Identifier: GPL-3.0-or-later
package android.widget;
import android.content.Context;
public class CompoundButton extends Button {
    public interface OnCheckedChangeListener {void onCheckedChanged(CompoundButton button,boolean checked);}
    private boolean checked;
    private OnCheckedChangeListener listener;
    public CompoundButton(Context context){super(context);}
    public void setChecked(boolean value){if(value==checked)return;checked=value;if(listener!=null)listener.onCheckedChanged(this,value);}
    public boolean isChecked(){return checked;}
    public void setOnCheckedChangeListener(OnCheckedChangeListener listener){this.listener=listener;}
    @Override public boolean performClick(){setChecked(!checked);return super.performClick();}
}
