// SPDX-License-Identifier: GPL-3.0-or-later
// SDK-shaped fixture; not part of the Android APK.
package android.widget; import android.content.Context;import android.view.View;import android.view.ViewGroup; public class AdapterView extends ViewGroup {public interface OnItemSelectedListener {void onNothingSelected(AdapterView parent);void onItemSelected(AdapterView parent,View view,int position,long id);}public AdapterView(Context context){super(context);}}
