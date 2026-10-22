# SPDX-License-Identifier: GPL-3.0-or-later
"""Actual framed worker with read-only MSIME and a private Mozc helper."""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import time

worker=Path(sys.argv[1]).resolve()
helper=Path(sys.argv[2]).resolve()
reference={row['name']:row for line in Path(sys.argv[3]).read_text(encoding='utf-8-sig').splitlines() if line.startswith('{') for row in [json.loads(line)]}
with tempfile.TemporaryDirectory(prefix='meltype-windows-converter-') as directory:
    root=Path(directory);profile=root/'profile';profile.mkdir()
    shutil.copy2(worker,root/worker.name)
    (root/'mozc').mkdir();shutil.copy2(helper,root/'mozc/meltype_mozc_helper.exe')
    config=profile/'config.json'
    def configure(mode,enabled=True,learning=False,separate=False,level='Manual',tab=False,**extra):config.write_text(json.dumps(dict(Enabled=enabled,ConversionEngine=mode,LiveConversion=False,PredictiveCandidates=False,LearningEnabled=learning,SeparateKeyRoles=separate,DetectionLevel=level,TabConversion=tab,**extra)),encoding='utf-8')
    configure('System')
    env=dict(os.environ,MELTYPE_RUST_DATA_DIR=str(profile));env.pop('MELTYPE_TEST_TIP_NO_CONVERTERS',None)
    process=subprocess.Popen([str(root/worker.name),'--stdio-tip-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
    def call(op,**fields):
        request=json.dumps(dict(op=op,sid='engine',process='test.exe',**fields),ensure_ascii=False).encode()
        process.stdin.write(struct.pack('<I',len(request))+request);process.stdin.flush()
        header=process.stdout.read(4);assert len(header)==4,(op,process.poll())
        size=struct.unpack('<I',header)[0];assert 0<size<=1048576
        return json.loads(process.stdout.read(size))
    def reconvert(text):return call('reconvert',text=text)['view']['text']
    try:
        call('hello')
        expected=reference['plain:にほんご']['converted'] or 'にほんご'
        assert reconvert('nihongo')==expected
        cancelled=call('key',vk=27);assert cancelled['commits'][0]['text']=='nihongo'
        assert not (profile/'mozc/fixture-calls.txt').exists(),'System must not launch Mozc'
        configure('Mozc')
        deadline=time.monotonic()+10
        while True:
            value=reconvert('nihongo');call('reset')
            if value=='MOZC:にほんご':break
            assert time.monotonic()<deadline,value
            time.sleep(.025)
        calls=profile/'mozc/fixture-calls.txt'
        assert Path((profile/'mozc/fixture-cwd.txt').read_text(encoding='utf-8')).resolve()==(root/'mozc').resolve()
        assert int((profile/'mozc/fixture-error-mode.txt').read_text())&0x8001==0x8001
        count=len(calls.read_text(encoding='utf-8').splitlines())
        assert reconvert('nihongo')=='MOZC:にほんご';call('reset')
        assert len(calls.read_text(encoding='utf-8').splitlines())==count,'cached conversion must not query helper again'
        for ch in 'hashi':assert call('key',vk=ord(ch.upper()),ch=ord(ch))['consumed']
        view=call('key',vk=32,ch=32)['view']
        assert view['text']=='MOZC:はし',view
        assert view['candidates'].index('MOZC:はし')<view['candidates'].index('ALT:はし'),view
        selected=call('select',index=view['candidates'].index('ALT:はし'))
        # C# 1.3.0 SelectCandidate changes the composition; Enter commits it.
        assert selected['commits']==[] and selected['view']['text']=='ALT:はし',selected
        committed=call('key',vk=13)
        assert committed['commits'][0]['text']=='ALT:はし',committed
        # New keys must reach the same actual converter as Space, not only
        # build local kana alternatives in Core.
        for extra,vk,mods in [(dict(EnglishFirstInput=True),40,0),(dict(MacControlKeys=True),78,2),(dict(ControlKeyBindings='T=Down'),84,2)]:
            configure('Mozc',**extra)
            call('reset')
            for ch in 'hashi':call('key',vk=ord(ch.upper()),ch=ord(ch))
            converted=call('key',vk=vk,mods=mods)
            assert converted['view']['converting'] and converted['view']['text']=='MOZC:はし',converted
            call('key',vk=13)
        # Tab starts conversion only when enabled. Its candidates must come
        # through the worker's converter, and cancellation keeps the input.
        configure('Mozc',tab=True)
        for ch in 'hashi':call('key',vk=ord(ch.upper()),ch=ord(ch))
        tabbed=call('key',vk=9)
        assert tabbed['view']['converting'] and tabbed['view']['text']=='MOZC:はし',tabbed
        advanced=call('key',vk=9)
        assert advanced['view']['selectedIndex']==1 and advanced['view']['text']=='ALT:はし',advanced
        previous=call('key',vk=9,mods=1)
        assert previous['view']['selectedIndex']==0,previous
        cancelled=call('key',vk=8)
        assert cancelled['commits']==[] and not cancelled['view']['converting'] and cancelled['view']['text']=='はし',cancelled
        assert call('commit')['commits'][0]['text']=='はし'
        for ch in 'hashi':call('key',vk=ord(ch.upper()),ch=ord(ch))
        call('key',vk=32,ch=32)
        passed=call('key',vk=9)
        assert not passed['consumed'] and passed['view'] is None and passed['commits'][0]['text']=='MOZC:はし',passed
        for ch in 'hashi':call('key',vk=ord(ch.upper()),ch=ord(ch))
        call('key',vk=118)
        assert call('key',vk=28)['view']['converting']
        cancelled=call('key',vk=27)
        assert cancelled['view']['text']=='ハシ' and not cancelled['view']['converting'],cancelled
        assert call('commit')['commits'][0]['text']=='ハシ'
        configure('Mozc',learning=True)
        call('hello')  # Complete settings schema migration before the privacy snapshot.
        before={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        for ch in 'kanji':assert call('key',vk=ord(ch.upper()),ch=ord(ch),private=True)['consumed']
        private=call('key',vk=32,ch=32,private=True)
        assert private['consumed'] and private['view']['text']=='MOZC:かんじ',private
        committed=call('commit')
        assert committed['commits'][0]['text']=='MOZC:かんじ',committed
        after={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        assert before==after,('private conversion must not persist learning', [str(p) for p in before.keys()|after.keys() if before.get(p)!=after.get(p)])
        assert not any(line.startswith('L\t') for line in calls.read_text(encoding='utf-8').splitlines()),'private conversion must not notify Mozc'
        for ch in 'kanji':assert call('key',vk=ord(ch.upper()),ch=ord(ch))['consumed']
        assert call('key',vk=32,ch=32)['view']['text']=='MOZC:かんじ'
        call('commit')
        normal={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        assert normal!=after,'normal conversion must resume persistent learning'
        # A later query serializes with learning and must bypass both caches.
        assert reconvert('kanji')=='MOZC:かんじ';call('reset')
        assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==1,calls.read_text(encoding='utf-8')
        for ch in 'hashi':assert call('key',vk=ord(ch.upper()),ch=ord(ch))['consumed']
        view=call('key',vk=32,ch=32)['view']
        (profile/'mozc/fixture-slow-learning').touch()
        started=time.monotonic()
        assert call('select',index=view['candidates'].index('ALT:はし'))['commits']==[]
        assert call('key',vk=13)['commits'][0]['text']=='ALT:はし'
        assert time.monotonic()-started<.5,'commit must not wait for slow helper learning'
        configure('System')
        assert reconvert('nihongo')==expected,'System stays usable while Mozc learns'
        call('reset');configure('Mozc',learning=True)
        deadline=time.monotonic()+5
        while True:
            started=time.monotonic()
            value=reconvert('hashi')
            assert time.monotonic()-started<.5,'new input must not join pending helper learning'
            if value=='ALT:はし':break
            call('reset')
            assert time.monotonic()<deadline,'completed learning must replace cached first choice'
            time.sleep(.025)
        call('reset')
        (profile/'mozc/fixture-slow-learning').unlink()
        trace=calls.read_text(encoding='utf-8').splitlines()
        assert any(line.startswith('L\t') and line.endswith('\tはし\x1fALT:はし') for line in trace),trace
        count=len(trace)
        configure('Mozc',False)
        disabled=call('reconvert',text='tenki')
        assert not disabled['consumed'] and disabled['view'] is None
        assert len(calls.read_text(encoding='utf-8').splitlines())==count
        configure('Mozc')
        # A valid empty Mozc response keeps Mozc-only availability, so its
        # fallback is the reading; it must not silently invoke Microsoft IME.
        assert reconvert('kara')=='から';call('reset')
        # F6-F8 change the display, but a subsequent actual conversion
        # still notifies the engine. F9/F10 Space commits the shown Latin form.
        configure('Mozc',learning=True)
        for function_key,raw in [(117,'tenki'),(118,'sakura'),(119,'kumo')]:
            count=len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])
            for ch in raw:assert call('key',vk=ord(ch.upper()),ch=ord(ch))['consumed']
            call('key',vk=function_key)
            converted=call('key',vk=32,ch=32)['view']
            assert converted['converting'] and converted['text'].startswith('MOZC:'),converted
            assert call('commit')['commits'][0]['text']==converted['text']
            reconvert('nihongo');call('reset')
            assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==count+1
        for function_key,expected_latin in [(120,'ｈａｓｈｉ '),(121,'hashi ')]:
            count=len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])
            for ch in 'hashi':call('key',vk=ord(ch.upper()),ch=ord(ch))
            call('key',vk=function_key)
            query_count=len(calls.read_text(encoding='utf-8').splitlines())
            committed=call('key',vk=32,ch=32)
            assert committed['view'] is None and committed['commits'][0]['text']==expected_latin,committed
            assert len(calls.read_text(encoding='utf-8').splitlines())==query_count,'Latin Space must not query the converter'
            reconvert('nihongo');call('reset')
            assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==count
        # SeparateKeyRoles deliberately opens candidates for Latin display too.
        configure('Mozc',learning=True,separate=True)
        for function_key,raw in [(120,'tsuki'),(121,'hoshi')]:
            count=len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])
            for ch in raw:call('key',vk=ord(ch.upper()),ch=ord(ch))
            call('key',vk=function_key)
            converted=call('key',vk=32,ch=32)
            assert converted['commits']==[] and converted['view']['converting'],converted
            assert converted['view']['text'].startswith('MOZC:'),converted
            call('commit')
            reconvert('nihongo');call('reset')
            assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==count+1
        configure('Mozc',learning=True)
        for ch in 'ame':call('key',vk=ord(ch.upper()),ch=ord(ch))
        count=len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])
        shifted=call('key',vk=32,ch=32,mods=1)
        assert shifted['commits']==[] and shifted['view']['text']=='MOZC:あめ',shifted
        candidate_count=len(shifted['view']['candidates'])
        backwards=call('key',vk=32,ch=32,mods=1)
        assert backwards['commits']==[] and backwards['view']['selectedIndex']==candidate_count-1,backwards
        assert call('key',vk=32,ch=32)['view']['selectedIndex']==0
        assert call('commit')['commits'][0]['text']=='MOZC:あめ'
        reconvert('nihongo');call('reset')
        assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==count+1
        configure('Mozc',learning=True,level='Balanced')
        count=len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])
        for ch in 'camera':call('key',vk=ord(ch.upper()),ch=ord(ch))
        english_shift=call('key',vk=32,ch=32,mods=1)
        assert english_shift['commits']==[] and english_shift['view']['text']=='MOZC:かめら',english_shift
        assert call('commit')['commits'][0]['text']=='MOZC:かめら'
        reconvert('nihongo');call('reset')
        assert len([line for line in calls.read_text(encoding='utf-8').splitlines() if line.startswith('L\t')])==count,'English clause must not notify Mozc'
        configure('Hybrid')
        assert reconvert('nihongo')=='MOZC:にほんご';call('reset')
        configure('System');count=len(calls.read_text(encoding='utf-8').splitlines())
        assert reconvert('nihongo')==expected;call('reset')
        assert len(calls.read_text(encoding='utf-8').splitlines())==count
        call('close')
    finally:
        process.stdin.close();process.wait(timeout=10)
        assert process.returncode==0
    assert (profile/'mozc/fixture-saved-on-quit').read_text()=='saved','healthy helper must flush on worker shutdown'
    # Losing the helper during asynchronous learning must not strand its
    # pending owner or keep a stale conversion cache alive.
    (profile/'mozc/fixture-fail-learning').touch()
    configure('Hybrid',learning=True)
    process=subprocess.Popen([str(root/worker.name),'--stdio-tip-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
    try:
        deadline=time.monotonic()+10
        while True:
            value=reconvert('nihongo');call('reset')
            if value=='MOZC:にほんご':break
            assert time.monotonic()<deadline,value
            time.sleep(.025)
        for ch in 'hashi':assert call('key',vk=ord(ch.upper()),ch=ord(ch))['consumed']
        view=call('key',vk=32,ch=32)['view']
        assert call('select',index=view['candidates'].index('ALT:はし'))['commits']==[]
        assert call('key',vk=13)['commits'][0]['text']=='ALT:はし'
        assert reconvert('nihongo')==expected,'learning transport failure must release ownership and fall back'
        configure('System');call('reset');call('close')
    finally:
        process.stdin.close();process.wait(timeout=10);assert process.returncode==0
    (profile/'mozc/fixture-fail-learning').unlink()
    no_helper=root/'without-helper';no_helper.mkdir();shutil.copy2(worker,no_helper/worker.name)
    configure('Mozc')
    process=subprocess.Popen([str(no_helper/worker.name),'--stdio-tip-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
    try:
        assert reconvert('nihongo')==expected,'Mozc unavailable must fall back to System'
        call('reset');call('close')
    finally:
        process.stdin.close();process.wait(timeout=10);assert process.returncode==0
    (profile/'mozc/fixture-malformed').touch()
    count=len(calls.read_text(encoding='utf-8').splitlines())
    process=subprocess.Popen([str(root/worker.name),'--stdio-tip-worker'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,env=env)
    try:
        deadline=time.monotonic()+10
        while True:
            value=reconvert('nihongo');call('reset')
            attempts=len(calls.read_text(encoding='utf-8').splitlines())-count
            assert attempts<=3,'unavailable helper must stop restarting'
            if attempts==3:
                assert value==expected,(expected,value)
                break
            assert time.monotonic()<deadline,(value,attempts)
            time.sleep(.025)
        assert reconvert('nihongo')==expected;call('reset');call('close')
        assert len(calls.read_text(encoding='utf-8').splitlines())==count+3
    finally:
        process.stdin.close();process.wait(timeout=10);assert process.returncode==0
print('Windows worker: real System, asynchronous Mozc, candidates/cache, empty replies, settings, privacy/disabled fields, missing helper and three-failure fallback passed')
