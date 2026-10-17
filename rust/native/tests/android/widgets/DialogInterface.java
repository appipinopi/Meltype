// SPDX-License-Identifier: GPL-3.0-or-later
package android.content;
public interface DialogInterface {
 interface OnClickListener {void onClick(DialogInterface dialog,int which);}
 interface OnCancelListener {void onCancel(DialogInterface dialog);}
}
