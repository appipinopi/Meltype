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
    def configure(mode,enabled=True,learning=False):config.write_text(json.dumps(dict(Enabled=enabled,ConversionEngine=mode,LiveConversion=False,PredictiveCandidates=False,LearningEnabled=learning,DetectionLevel='Manual')),encoding='utf-8')
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
        assert selected['commits'][0]['text']=='ALT:はし',selected
        count=len(calls.read_text(encoding='utf-8').splitlines())
        configure('Mozc',learning=True)
        before={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        private=call('reconvert',text='kanji',private=True)
        assert private['consumed'] and private['view']['text']=='MOZC:かんじ',private
        committed=call('commit')
        assert committed['commits'][0]['text']=='MOZC:かんじ',committed
        after={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        assert before==after,'private conversion must not persist learning'
        assert reconvert('kanji')=='MOZC:かんじ'
        call('commit')
        normal={p.relative_to(profile):p.read_bytes() for p in profile.rglob('*') if p.is_file() and 'mozc' not in p.relative_to(profile).parts}
        assert normal!=after,'normal conversion must resume persistent learning'
        count=len(calls.read_text(encoding='utf-8').splitlines())
        configure('Mozc',False)
        disabled=call('reconvert',text='tenki')
        assert not disabled['consumed'] and disabled['view'] is None
        assert len(calls.read_text(encoding='utf-8').splitlines())==count
        configure('Mozc')
        # A valid empty Mozc response keeps Mozc-only availability, so its
        # fallback is the reading; it must not silently invoke Microsoft IME.
        assert reconvert('kara')=='から';call('reset')
        configure('Hybrid')
        assert reconvert('nihongo')=='MOZC:にほんご';call('reset')
        configure('System');count=len(calls.read_text(encoding='utf-8').splitlines())
        assert reconvert('nihongo')==expected;call('reset')
        assert len(calls.read_text(encoding='utf-8').splitlines())==count
        call('close')
    finally:
        process.stdin.close();process.wait(timeout=10)
        assert process.returncode==0
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
