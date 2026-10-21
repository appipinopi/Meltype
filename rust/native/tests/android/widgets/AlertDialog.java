// SPDX-License-Identifier: GPL-3.0-or-later
package android.app;
import android.content.*;
public final class AlertDialog implements DialogInterface {
 public static Builder last;public static boolean failShow;
 public static final class Builder {
  public final Context context;public String title,message,positive,negative;public boolean shown;
  public DialogInterface.OnClickListener yes,no;public DialogInterface.OnCancelListener cancel;
  public Builder(Context context){this.context=context;last=this;}
  public Builder setTitle(CharSequence value){title=value.toString();return this;}
  public Builder setMessage(CharSequence value){message=value.toString();return this;}
  public Builder setPositiveButton(CharSequence text,DialogInterface.OnClickListener listener){positive=text.toString();yes=listener;return this;}
  public Builder setNegativeButton(CharSequence text,DialogInterface.OnClickListener listener){negative=text.toString();no=listener;return this;}
  public Builder setOnCancelListener(DialogInterface.OnCancelListener listener){cancel=listener;return this;}
  public AlertDialog show(){if(failShow)throw new IllegalStateException("show failed");shown=true;return new AlertDialog();}
 }
}
