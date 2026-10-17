// SPDX-License-Identifier: GPL-3.0-or-later
package android.content;
import android.net.Uri;
import java.io.*;
/** Test-only resolver: actual JVM streams, no device/provider access. */
public final class ContentResolver {
    public InputStream input;
    public OutputStream output;
    public int reads,writes;
    public InputStream openInputStream(Uri uri){reads++;return input;}
    public OutputStream openOutputStream(Uri uri,String mode){
        if(!mode.equals("wt"))throw new AssertionError(mode);writes++;return output;
    }
}
