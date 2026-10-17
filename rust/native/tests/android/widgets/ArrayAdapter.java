// SPDX-License-Identifier: GPL-3.0-or-later
// SDK-shaped fixture; not part of the Android APK.
package android.widget; import android.content.Context; public final class ArrayAdapter implements SpinnerAdapter {public final Context context;public final int layout;public final Object[] labels;public ArrayAdapter(Context context,int layout,Object[] labels){this.context=context;this.layout=layout;this.labels=labels;}}
