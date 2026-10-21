# SPDX-License-Identifier: GPL-3.0-or-later
"""Run real upstream APIs and Rust only against isolated evidence directories."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def cases():
    def keys(raw):
        return [[ord(c.upper()) if c.isascii() and c.isalpha() else 0, ord(c), int(c.isupper())] for c in raw]
    rows=[]
    def add(name, level, scored, before, can_delete, raw, files=None, physical=None):
        settings=dict(SettingsVersion=5,DetectionLevel=level,ScoredSegmentation=scored,
            LiveConversion=False,PredictiveCandidates=False,LearningEnabled=False,TabConversion=False,
            AutoCorrectAfterCommit=False,InputStyle='Kana' if physical else 'Romaji')
        row=dict(name=name,settings=settings,before=before,after=' today' if before else '',
            canDelete=can_delete,keys=(physical or keys(raw))+[[8,0,0]]+keys('a')+[[117 if can_delete else 121,0,0],[13,0,0]])
        if files: row['files']=files
        rows.append(row)
    for raw in ['sushi','google','meetingnosushi','kyouno','amazon','githubnihongo','shumire-shon','aaiu']:
        for level in ['Manual','Balanced','Conservative','Aggressive']:
            for scored in [False,True]:
                for before in ['','we use ']:
                    for can_delete in [False,True]:
                        add(f'{raw}-{level}-{scored}-{bool(before)}-{can_delete}',level,scored,before,can_delete,raw)
    for table in ['kk\tきん\n','ka\tか\nkak\tかく\n','zz\tめると\n']:
        for raw in ['kka','kaka','zza']:
            for level in ['Manual','Balanced','Conservative','Aggressive']:
                for scored in [False,True]:
                    add(f'custom-{len(rows)}',level,scored,'',True,raw,{'dictionaries/romaji.txt':table})
    for raw in ['sushi','google','kyouno','hashi']:
        for level in ['Manual','Balanced','Conservative','Aggressive']:
            for scored in [False,True]:
                files={'languages.json':json.dumps({'sushi':dict(English=True,Count=2,Explicit=True,Used='2026-10-19T00:00:00Z'),'google':dict(English=False,Count=1,Explicit=True,Used='2026-10-19T00:00:00Z')})}
                add(f'learned-{len(rows)}',level,scored,'',True,raw,files)
    for level in ['Manual','Balanced','Conservative','Aggressive']:
        for scored in [False,True]:
            add(f'protected-path-{level}-{scored}',level,scored,'',True,'./z]')
            rows[-1]['keys']=keys('./z]')+[[8,0,0],[13,0,0]]
    for raw,suffix in [('gakkou','ta'),('google','ya')]:
        add(f'retained-{raw}-{suffix}','Balanced',False,'',True,raw)
        rows[-1]['keys']=keys(raw)+[[8,0,0],[8,0,0]]+keys(suffix)+[[13,0,0]]
    add('retained-multiple-commits-learning','Balanced',False,'',True,'gakkou')
    rows[-1]['settings']['LearningEnabled']=True
    rows[-1]['keys']=keys('gakkou')+[[8,0,0],[8,0,0],[118,0,0],[13,0,0]]+keys('gakkou')+[[8,0,0],[8,0,0]]+keys('kou')+[[13,0,0]]+keys('gakkou')+[[8,0,0],[8,0,0]]+keys('ta')+[[13,0,0]]
    return rows


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--csharp',type=Path,required=True)
    parser.add_argument('--rust',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--data-only',action='store_true')
    args=parser.parse_args()
    out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    if (out/'report.json').exists():parser.error('use a new output to retain earlier evidence')
    csharp=['dotnet',str(args.csharp.resolve())];rust=[str(args.rust.resolve())]
    def call(program,mode,directory,*extra):
        return subprocess.check_output([*program,mode,str(directory),*map(str,extra)],encoding='utf-8-sig')
    def snapshot(program,directory):return json.loads(call(program,'snapshot',directory))
    source=out/'original';call(csharp,'seed',source)
    original=snapshot(csharp,source);imported=snapshot(rust,source)
    (out/'original-snapshot.json').write_text(json.dumps(original,ensure_ascii=False,indent=2),encoding='utf-8')
    checks=[]
    checks.append(dict(name='C# files -> Rust public readers',matches=original==imported,expected=original,actual=imported))
    call(rust,'mutate',source);mutated=snapshot(rust,source)
    exported=out/'rust.meltype-backup';call(rust,'backup',source,exported)
    target=out/'csharp-restored';call(csharp,'restore',target,exported)
    reread=snapshot(csharp,target)
    checks.append(dict(name='Rust mutations -> backup -> C# public readers',matches=mutated==reread,expected=mutated,actual=reread))
    call(csharp,'seed',target);updated=snapshot(csharp,target)
    returned=out/'csharp.meltype-backup';call(csharp,'backup',target,returned)
    call(rust,'restore',source,returned);reread=snapshot(rust,source)
    checks.append(dict(name='C# updates -> backup -> existing Rust state -> restart',matches=updated==reread,expected=updated,actual=reread))
    print('data',[(r['name'],r['matches']) for r in checks],flush=True)
    key_checks=[]
    if not args.data_only:
        fixture=out/'cases.json';rows=cases();fixture.write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
        for label,program in [('csharp',csharp),('rust',rust)]:
            with (out/f'{label}-keys.jsonl').open('w',encoding='utf-8') as log:
                subprocess.run([*program,'keys',str(out/f'{label}-profiles'),str(fixture)],stdout=log,check=True)
        expected=[json.loads(s) for s in (out/'csharp-keys.jsonl').read_text(encoding='utf-8-sig').splitlines()]
        actual=[json.loads(s) for s in (out/'rust-keys.jsonl').read_text(encoding='utf-8-sig').splitlines()]
        assert len(expected)==len(actual)==len(rows)
        def projection(reply):
            view=reply['view']
            return dict(consumed=reply['consumed'],commits=reply['commits'],view={k:view[k] for k in ['text','converting','suggestion']} if view else None)
        for case,want,got in zip(rows,expected,actual,strict=True):
            assert want['name']==got['name']==case['name']
            mismatches=[dict(index=i,key=key,expected=projection(a),actual=projection(b)) for i,(key,a,b) in enumerate(zip(case['keys'],want['expected'],got['expected'],strict=True)) if projection(a)!=projection(b)]
            key_checks.append(dict(name=case['name'],matches=not mismatches,mismatches=mismatches))
        print('keys',sum(r['matches'] for r in key_checks),'/',len(rows),flush=True)
    settings_rows=settings_cases()
    settings_file=out/'settings-cases.json';settings_file.write_text(json.dumps(settings_rows,ensure_ascii=False,indent=2),encoding='utf-8')
    settings_outputs={}
    for label,program in [('csharp',csharp),('rust',rust)]:
        contents=call(program,'settings',out/f'{label}-settings',settings_file)
        (out/f'{label}-settings.jsonl').write_text(contents,encoding='utf-8')
        settings_outputs[label]=[json.loads(s) for s in contents.splitlines()]
    settings_checks=[]
    for case,want,got in zip(settings_rows,settings_outputs['csharp'],settings_outputs['rust'],strict=True):
        settings_checks.append(dict(name=case['name'],matches=settings_projection(want)==settings_projection(got),expected=want,actual=got))
    print('settings',sum(x['matches'] for x in settings_checks),'/',len(settings_rows),flush=True)
    report=dict(data=checks,keys=key_checks,settings=settings_checks,source_sha256={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [Path(__file__),Path('tools/parity/core-data/Program.cs'),args.csharp,args.rust]})
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    raise SystemExit(not all(r['matches'] for r in checks+key_checks+settings_checks))


def settings_cases():
    ui=json.loads(Path('rust/settings-app/settings-ui.json').read_text(encoding='utf-8'))
    defaults=json.loads(Path('rust/core/data/settings-defaults.json').read_text(encoding='utf-8'))
    fields={f['key']:f for f in ui['fields']}
    rows=[dict(name='defaults',config={})]
    for name,value in defaults.items():
        if isinstance(value,bool):values=[not value]
        elif isinstance(value,int):values=[-100,0,99999] if name!='SettingsVersion' else [1,2,3,4,5]
        elif isinstance(value,str) and name not in ['ActiveProfile']:
            choices=ui['enums'].get(fields.get(name,{}).get('type',''))
            values=list(choices) if choices else ['', '設定テスト😀']
        else:continue
        for index,value in enumerate(values):
            config=dict(defaults);config[name]=value
            rows.append(dict(name=f'{name}-{index}',config=config))
    rows.extend([
        dict(name='profile-empty-active',config=dict(SettingsVersion=5,ActiveProfile='',Profiles=[{'Name':' 仕事 ','Values':{'LiveConversion':False}}])),
        dict(name='profile-lifecycle',config={},ops=[{'op':'add','name':'仕事'},{'op':'add','name':'趣味'},{'op':'switch','name':'標準'},{'op':'switch','name':'仕事'},{'op':'rename','name':'仕事','newName':'作業'},{'op':'remove','name':'作業'},{'op':'remove','name':'標準'}]),
        dict(name='profile-invalid-operations',config={},ops=[{'op':'add','name':''},{'op':'add','name':'標準'},{'op':'switch','name':'missing'},{'op':'remove','name':'標準'}]),
        dict(name='profile-duplicate-names',config=dict(SettingsVersion=5,ActiveProfile='仕事',Profiles=[{'Name':'仕事','Values':{'LiveConversion':False}},{'Name':' 仕事 ','Values':{'LiveConversion':True}},None,{'Name':'','Values':None}])),
        dict(name='app-rules-normalization',config=dict(SettingsVersion=5,AppRules=[None,{'Process':' Code.exe ','Enabled':True},{'Process':'code.EXE','Enabled':False},{'Process':' '}],AppKinds=[{'Name':'会話','Base':'General','LiveConversion':False}])),
    ])
    return rows


def settings_projection(row):
    # C# discards unknown fields on deserialization. Rust deliberately keeps
    # them for forward compatibility; compare every known setting and retain
    # the complete raw responses in the evidence separately.
    known=set(json.loads(Path('rust/core/data/settings-defaults.json').read_text(encoding='utf-8')))
    row=json.loads(json.dumps(row))
    for trace in row['expected']:
        value={k:v for k,v in trace['settings'].items() if k in known}
        for profile in value['Profiles']:
            if isinstance(profile.get('Values'),dict):
                profile['Values']={k:v for k,v in profile['Values'].items() if k in known}
        trace['settings']=value
    return row


if __name__=='__main__':main()
