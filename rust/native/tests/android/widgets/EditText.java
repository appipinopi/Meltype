// SPDX-License-Identifier: GPL-3.0-or-later
package android.widget;
import android.content.Context;
import android.text.Editable;
public class EditText extends TextView {
    public String hint="";public int lines;
    public EditText(Context context){super(context);}
    public void setHint(CharSequence value){hint=value.toString();}
    public void setMinLines(int lines){this.lines=lines;}
    public Editable getText(){return new Editable(text);}
}
