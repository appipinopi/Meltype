// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
package moe.meltype.rust;

import android.app.Activity;
import android.content.DialogInterface;
import android.content.Intent;
import android.os.Bundle;
import android.widget.*;

/** OS lifecycle/listener forwarding; settings behavior and widgets live in Rust. */
public final class SettingsActivity extends Activity {
    private long session;
    private LinearLayout body;
    private TextView status;
    private SettingsPage form;
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        session=Native.create(getFilesDir().getAbsolutePath());
        if(state!=null)Native.restoreDocumentPicker(session,state.getString("documentOperation"));
        String result=Native.createSettingsActivity(this);
        if(!"{}".equals(result)){Native.destroy(session);session=0;throw new IllegalStateException(result);}
    }
    private void settingsFormReady(){form=new SettingsPage(this,body,operation->Native.settingsUiAction(session,this,operation));}
    private AdapterView.OnItemSelectedListener settingsNavigationCallback(){return new AdapterView.OnItemSelectedListener(){
        public void onNothingSelected(AdapterView<?> parent){}
        public void onItemSelected(AdapterView<?> parent,android.view.View view,int position,long id){Native.settingsUiShow(session,SettingsActivity.this,position);}
    };}
    @Override protected void onSaveInstanceState(Bundle state){state.putString("documentOperation",Native.pendingDocument(session));super.onSaveInstanceState(state);}
    @Override public void onDestroy(){Native.destroy(session);session=0;super.onDestroy();}
    @Override protected void onActivityResult(int code,int result,Intent data){
        super.onActivityResult(code,result,data);Native.settingsUiResult(session,this,code,result,data);
    }
    private DialogInterface.OnClickListener settingsConfirmationClick(String payload,boolean accept){
        return (dialog,which)->Native.settingsUiConfirmation(session,this,payload,accept);
    }
    private DialogInterface.OnCancelListener settingsConfirmationCancel(String payload){
        return dialog->Native.settingsUiConfirmation(session,this,payload,false);
    }
}
