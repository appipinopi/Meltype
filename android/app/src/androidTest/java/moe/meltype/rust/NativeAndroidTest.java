// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.content.Context;
import android.content.Intent;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.text.Editable;
import android.text.Selection;
import android.view.ContextThemeWrapper;
import android.view.KeyEvent;
import android.view.View;
import android.view.inputmethod.BaseInputConnection;
import android.widget.Button;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import org.junit.Test;
import org.junit.runner.RunWith;
import org.json.JSONObject;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicReference;
import static org.junit.Assert.*;

/** Runs on Android's real framework, without registering or activating an IME. */
@RunWith(AndroidJUnit4.class)
public final class NativeAndroidTest {
    private interface Checked {void run() throws Exception;}
    private static void ui(Checked action) throws Throwable {
        AtomicReference<Throwable> failure=new AtomicReference<>();
        InstrumentationRegistry.getInstrumentation().runOnMainSync(()->{
            try{action.run();}catch(Throwable error){failure.set(error);}
        });
        if(failure.get()!=null)throw failure.get();
    }
    private static Context context(){return InstrumentationRegistry.getInstrumentation().getTargetContext();}
    public static final class Widgets extends ContextThemeWrapper {
        private TextView preview;
        private LinearLayout candidates;
        private final List<String> clicks=new ArrayList<>();
        private long session;
        private Editor editor;
        private int options;
        private final List<Intent> intents=new ArrayList<>();
        public Widgets(){this(context());}
        public Widgets(Context base){super(base,android.R.style.Theme_Material_Light_NoActionBar);}
        private View.OnClickListener keyboardCallback(int action,int value){return view->{
            clicks.add(action+":"+value);
            if(session!=0)Native.keyboardAction(session,this,editor,action,value,options);
        };}
        @Override public void startActivity(Intent intent){intents.add(intent);}
    }
    public static final class Editor extends BaseInputConnection {
        private final Editable text=Editable.Factory.getInstance().newEditable("");
        int reads,batches;
        final List<KeyEvent> events=new ArrayList<>();
        final List<Integer> actions=new ArrayList<>();
        Runnable onCommit,onRead;
        Editor(){super(new View(context()),true);Selection.setSelection(text,0);}
        @Override public Editable getEditable(){return text;}
        @Override public CharSequence getTextBeforeCursor(int count,int flags){reads++;if(onRead!=null){Runnable callback=onRead;onRead=null;callback.run();}return super.getTextBeforeCursor(count,flags);}
        @Override public CharSequence getTextAfterCursor(int count,int flags){reads++;return super.getTextAfterCursor(count,flags);}
        @Override public boolean beginBatchEdit(){batches++;return super.beginBatchEdit();}
        @Override public boolean endBatchEdit(){batches--;return super.endBatchEdit();}
        @Override public boolean sendKeyEvent(KeyEvent event){events.add(event);return true;}
        @Override public boolean performEditorAction(int action){actions.add(action);return true;}
        @Override public boolean commitText(CharSequence value,int cursor){
            boolean result=super.commitText(value,cursor);
            if(onCommit!=null){Runnable callback=onCommit;onCommit=null;callback.run();}
            return result;
        }
        void replace(String value,int start,int end){text.replace(0,text.length(),value);Selection.setSelection(text,start,end);}
    }
    private static final class Profile implements AutoCloseable {
        final File root;
        final java.nio.file.Path allowedRoot;
        final long session;
        Profile() throws Exception {
            root=new File(context().getCacheDir(),"rust-instrumentation-"+UUID.randomUUID());
            assertTrue(root.mkdir());
            allowedRoot=root.getCanonicalFile().toPath();
            try(FileOutputStream stream=new FileOutputStream(new File(root,"config.json"))) {
                stream.write("{\"DetectionLevel\":\"Manual\"}".getBytes(StandardCharsets.UTF_8));
            }
            session=Native.create(root.getAbsolutePath());assertTrue(session!=0);
            Native.startInput(session,1,0);
        }
        @Override public void close() throws Exception {Native.destroy(session);remove(allowedRoot,root);}
        private static void remove(java.nio.file.Path allowedRoot,File entry) throws Exception {
            // Only the newly-created test cache child, never a replaced path.
            assertTrue(entry.getCanonicalFile().toPath().startsWith(allowedRoot));
            File[] children=entry.listFiles();if(children!=null)for(File child:children)remove(allowedRoot,child);
            assertTrue("delete "+entry,entry.delete());
        }
    }
    private static void type(long session,Editor editor,String letters) throws Exception {
        for(int scalar:letters.codePoints().toArray()) {
            String reply=Native.editorKey(session,editor,0,scalar,0,false,false,Native.composingText(session));
            assertTrue(new JSONObject(Native.applyEditor(session,editor,reply,Native.composingText(session))).getBoolean("consumed"));
            assertEquals(0,editor.batches);
        }
    }
    @Test public void keyboardAndCandidatesUseRealFrameworkWidgets() throws Throwable {ui(()->{
        Widgets service=new Widgets();LinearLayout root=(LinearLayout)Native.createInputView(service);
        assertNotNull(root);assertEquals(9,root.getChildCount());assertEquals(LinearLayout.VERTICAL,root.getOrientation());
        assertSame(service.preview,root.getChildAt(0));
        assertEquals(20f*service.getResources().getDisplayMetrics().scaledDensity,service.preview.getTextSize(),.1f);
        assertSame(service.candidates,((HorizontalScrollView)root.getChildAt(1)).getChildAt(0));
        String[] rows={"qwertyuiop","asdfghjkl","zxcvbnm","1234567890","-.,!?@/"};
        for(int rowIndex=0;rowIndex<5;rowIndex++) {
            LinearLayout row=(LinearLayout)root.getChildAt(rowIndex+2);assertEquals(rows[rowIndex].length(),row.getChildCount());
            for(int i=0;i<row.getChildCount();i++) {
                Button button=(Button)row.getChildAt(i);assertEquals(String.valueOf(rows[rowIndex].charAt(i)),button.getText().toString());
                LinearLayout.LayoutParams params=(LinearLayout.LayoutParams)button.getLayoutParams();
                assertEquals(0,params.width);assertEquals(-2,params.height);assertEquals(1f,params.weight,0f);
                assertEquals(0,button.getMinWidth());assertEquals(0,button.getMinimumWidth());assertEquals(1,button.getPaddingLeft());
                button.performClick();
            }
        }
        assertEquals(43,service.clicks.size());
        assertTrue(Native.renderInputView(service,"{\"consumed\":true,\"composingText\":\"かな😀\",\"view\":{\"candidates\":[\"仮名\",\"かな\"],\"predictions\":[\"かなしい\"]}}"));
        assertEquals("かな😀",service.preview.getText().toString());assertEquals(3,service.candidates.getChildCount());
        service.clicks.clear();for(int i=0;i<3;i++)service.candidates.getChildAt(i).performClick();
        assertEquals(java.util.Arrays.asList("10:0","10:1","11:0"),service.clicks);
        root.measure(View.MeasureSpec.makeMeasureSpec(1080,View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(0,View.MeasureSpec.UNSPECIFIED));
        root.layout(0,0,1080,root.getMeasuredHeight());assertTrue(root.getHeight()>0);
        Bitmap image=Bitmap.createBitmap(root.getWidth(),root.getHeight(),Bitmap.Config.ARGB_8888);
        Canvas canvas=new Canvas(image);canvas.drawColor(android.graphics.Color.WHITE);root.draw(canvas);
        int[] pixels=new int[image.getWidth()*image.getHeight()];image.getPixels(pixels,0,image.getWidth(),0,0,image.getWidth(),image.getHeight());
        int painted=0;for(int pixel:pixels)if((pixel>>>24)!=0&&(pixel&0x00ffffff)!=0x00ffffff)painted++;assertTrue("framework preview contains visible widget pixels",painted>1000);
        File artifacts=new File(context().getFilesDir(),"instrumentation-artifacts");assertTrue(artifacts.isDirectory()||artifacts.mkdirs());
        try(FileOutputStream stream=new FileOutputStream(new File(artifacts,"input-view.png"))){assertTrue(image.compress(Bitmap.CompressFormat.PNG,100,stream));}
        image.recycle();
        String metadata="sdk="+android.os.Build.VERSION.SDK_INT+"\nabi="+android.os.Build.SUPPORTED_ABIS[0]+"\nfingerprint="+android.os.Build.FINGERPRINT+"\nwidth="+root.getWidth()+"\nheight="+root.getHeight()+"\n";
        try(FileOutputStream stream=new FileOutputStream(new File(artifacts,"device.txt"))){stream.write(metadata.getBytes(StandardCharsets.UTF_8));}
    });}
    @Test public void realEditableConversionUndoAndUtf16Guards() throws Throwable {ui(()->{
        try(Profile profile=new Profile()) {
            long session=profile.session;Editor editor=new Editor();type(session,editor,"konnichiha");
            Native.applyEditor(session,editor,Native.commit(session),Native.composingText(session));
            assertEquals("こんにちは",editor.text.toString());assertEquals(-1,BaseInputConnection.getComposingSpanStart(editor.text));
            String undo=Native.editorUndo(session,editor,false,"");assertTrue(undo.contains("\"deleteBefore\":5"));
            Native.applyEditor(session,editor,undo,"");assertTrue(BaseInputConnection.getComposingSpanStart(editor.text)>=0);
            Native.finishInput(session,editor,true,Native.composingText(session));assertEquals("こんにちは",editor.text.toString());
            Native.startInput(session,1,0);editor=new Editor();editor.replace("前😀",3,3);
            String edit="{\"consumed\":true,\"commits\":[{\"deleteBefore\":2,\"expect\":\"😀\",\"text\":\"橋\"}],\"view\":null}";
            assertTrue(new JSONObject(Native.applyEditor(session,editor,edit,"")).getBoolean("consumed"));
            assertEquals("前橋",editor.text.toString());assertEquals(2,Selection.getSelectionStart(editor.text));
            editor.replace("前😀",1,3);
            assertFalse(new JSONObject(Native.applyEditor(session,editor,edit,"")).getBoolean("consumed"));
            assertEquals("前😀",editor.text.toString());assertEquals(0,editor.batches);
        }
    });}
    @Test public void privacyAndFallbackUseRealAndroidTypes() throws Throwable {ui(()->{
        try(Profile profile=new Profile()) {
            long session=profile.session;Editor editor=new Editor();editor.replace("秘密",2,2);
            Native.startInput(session,0x81,0);Native.editorKey(session,editor,29,'a',0,true,true,"");assertEquals(0,editor.reads);
            editor.replace("前",1,1);assertTrue(Native.softwareFallback(editor,0,0x1f600,0));assertEquals("前😀",editor.text.toString());
            assertTrue(Native.softwareFallback(editor,8,0,0));assertEquals("前",editor.text.toString());
            Native.softwareFallback(editor,0x25,0,0);assertEquals(2,editor.events.size());
            assertEquals(KeyEvent.ACTION_DOWN,editor.events.get(0).getAction());assertEquals(KeyEvent.ACTION_UP,editor.events.get(1).getAction());
            assertEquals(KeyEvent.KEYCODE_DPAD_LEFT,editor.events.get(0).getKeyCode());
            Native.softwareFallback(editor,13,0,6);assertEquals(java.util.Collections.singletonList(6),editor.actions);
            Native.softwareFallback(editor,13,0,0x40000006);assertEquals("前\n",editor.text.toString());
        }
    });}
    @Test public void reentrantFieldReplacementKeepsNewFrameworkEditorState() throws Throwable {ui(()->{
        try(Profile profile=new Profile()) {
            long session=profile.session;Editor old=new Editor(),current=new Editor();
            old.onCommit=()->{try{Native.startInput(session,1,0);type(session,current,"nihongo");}catch(Exception error){throw new AssertionError(error);}};
            String reply="{\"consumed\":true,\"commits\":[{\"text\":\"old\"},{\"text\":\"extra\"}],\"view\":null}";
            String result=Native.applyEditor(session,old,reply,"");
            assertEquals("old",old.text.toString());assertEquals(0,old.batches);
            assertEquals(current.text.toString(),Native.composingText(session));assertTrue((Native.editorFlags(session)&4)!=0);
            assertTrue(new JSONObject(result).getBoolean("consumed"));
            Native.finishInput(session,current,true,Native.composingText(session));assertEquals("にほんご",current.text.toString());
        }
    });}
    private static void press(LinearLayout root,String label) {
        for(int row=2;row<9;row++) {
            LinearLayout keys=(LinearLayout)root.getChildAt(row);
            for(int i=0;i<keys.getChildCount();i++) {
                Button button=(Button)keys.getChildAt(i);
                if(button.getText().toString().equals(label)){assertTrue(button.performClick());return;}
            }
        }
        fail("missing keyboard button: "+label);
    }
    private static void keys(LinearLayout root,String letters){for(char letter:letters.toCharArray())press(root,String.valueOf(letter));}
    private static void candidate(Widgets service,String label) {
        for(int i=0;i<service.candidates.getChildCount();i++) {
            Button button=(Button)service.candidates.getChildAt(i);
            if(button.getText().toString().equals(label)){assertTrue(button.performClick());return;}
        }
        fail("missing candidate: "+label);
    }
    @Test public void realButtonDispatchUsesRustActionsAndFrameworkEditor() throws Throwable {ui(()->{
        try(Profile profile=new Profile()) {
            Widgets service=new Widgets();service.session=profile.session;service.editor=new Editor();service.options=6;
            LinearLayout root=(LinearLayout)Native.createInputView(service);assertNotNull(root);
            keys(root,"konnichiha");press(root,"確定/↵");assertEquals("こんにちは",service.editor.text.toString());
            assertTrue(service.editor.actions.isEmpty());press(root,"確定/↵");assertEquals(java.util.Collections.singletonList(6),service.editor.actions);
            press(root,"あ/A");press(root,"⇧");press(root,"a");press(root,"a");assertEquals("こんにちはAa",service.editor.text.toString());
            press(root,"あ/A");Native.startInput(profile.session,1,0);service.editor=new Editor();
            keys(root,"hashi");press(root,"変換/空白");candidate(service,"箸");assertEquals("箸",service.editor.text.toString());
            Native.control(profile.session,"{\"op\":\"settings_set\",\"name\":\"PredictiveCandidates\",\"value\":true}");
            Native.control(profile.session,"{\"op\":\"dictionary_set\",\"reading\":\"めるたいぷ\",\"word\":\"Meltype\"}");
            Native.startInput(profile.session,1,0);service.editor=new Editor();keys(root,"meru");candidate(service,"Meltype");assertEquals("Meltype",service.editor.text.toString());
            Native.startInput(profile.session,1,0);service.editor=new Editor();keys(root,"nihongo");press(root,"設定");
            assertEquals("にほんご",service.editor.text.toString());assertEquals(1,service.intents.size());
            assertEquals(SettingsActivity.class.getName(),service.intents.get(0).getComponent().getClassName());
            assertTrue((service.intents.get(0).getFlags()&Intent.FLAG_ACTIVITY_NEW_TASK)!=0);
            Native.startInput(profile.session,1,0);service.editor=new Editor();service.editor.onRead=()->Native.startInput(profile.session,0x81,0);
            press(root,"a");assertEquals("",service.editor.text.toString());assertEquals(3,Native.editorFlags(profile.session));assertEquals(1,service.editor.reads);
        }
    });}
    @Test public void wrappedLabelsStayWithinRowsAcrossWidthsAndFontScale() throws Throwable {ui(()->{
        File artifacts=new File(context().getFilesDir(),"instrumentation-artifacts");assertTrue(artifacts.isDirectory()||artifacts.mkdirs());
        StringBuilder report=new StringBuilder();int wrapped=0;
        int[] widths={1080,720,1080};float[] scales={1f,1f,1.5f};
        String[] names={null,"input-view-narrow.png","input-view-large-text.png"};
        for(int sample=0;sample<widths.length;sample++) {
            Configuration config=new Configuration(context().getResources().getConfiguration());config.fontScale=scales[sample];
            Widgets service=new Widgets(context().createConfigurationContext(config));
            LinearLayout root=(LinearLayout)Native.createInputView(service);assertNotNull(root);
            root.measure(View.MeasureSpec.makeMeasureSpec(widths[sample],View.MeasureSpec.EXACTLY),View.MeasureSpec.makeMeasureSpec(0,View.MeasureSpec.UNSPECIFIED));
            root.layout(0,0,root.getMeasuredWidth(),root.getMeasuredHeight());
            report.append("width=").append(widths[sample]).append(" fontScale=").append(scales[sample]).append(" rootHeight=").append(root.getHeight()).append('\n');
            for(int group=2;group<9;group++) {
                LinearLayout row=(LinearLayout)root.getChildAt(group);assertFalse(row.isBaselineAligned());
                for(int i=0;i<row.getChildCount();i++) {
                    Button button=(Button)row.getChildAt(i);String label=button.getText().toString();
                    String detail="width="+widths[sample]+" scale="+scales[sample]+" label="+label;
                    assertTrue(detail+" top="+button.getTop(),button.getTop()>=0);
                    assertTrue(detail+" bottom="+button.getBottom()+" rowHeight="+row.getHeight(),button.getBottom()<=row.getHeight());
                    android.text.Layout layout=button.getLayout();assertNotNull(detail,layout);
                    int available=button.getHeight()-button.getCompoundPaddingTop()-button.getCompoundPaddingBottom();
                    assertTrue(detail+" textHeight="+layout.getHeight()+" available="+available,layout.getHeight()<=available+1);
                    assertEquals(detail,0,layout.getEllipsisCount(layout.getLineCount()-1));
                    assertEquals(detail,layout.getText().length(),layout.getLineEnd(layout.getLineCount()-1));
                    if(layout.getLineCount()>1)wrapped++;
                }
            }
            if(names[sample]!=null) {
                Bitmap image=Bitmap.createBitmap(root.getWidth(),root.getHeight(),Bitmap.Config.ARGB_8888);
                Canvas canvas=new Canvas(image);canvas.drawColor(android.graphics.Color.WHITE);root.draw(canvas);
                try(FileOutputStream stream=new FileOutputStream(new File(artifacts,names[sample]))){assertTrue(image.compress(Bitmap.CompressFormat.PNG,100,stream));}
                image.recycle();
            }
        }
        assertTrue("at least one label actually wrapped",wrapped>0);report.append("wrappedLabels=").append(wrapped).append('\n');
        try(FileOutputStream stream=new FileOutputStream(new File(artifacts,"label-fit.txt"))){stream.write(report.toString().getBytes(StandardCharsets.UTF_8));}
    });}
}
