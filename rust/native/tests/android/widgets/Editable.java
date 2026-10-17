// SPDX-License-Identifier: GPL-3.0-or-later
package android.text;
/** SDK-shaped return type for JNI EditText tests, not Android's text engine. */
public final class Editable implements CharSequence {
    private final String value;
    public Editable(String value){this.value=value;}
    public int length(){return value.length();}
    public char charAt(int i){return value.charAt(i);}
    public CharSequence subSequence(int start,int end){return value.subSequence(start,end);}
    public String toString(){return value;}
}
