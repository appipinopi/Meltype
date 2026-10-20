# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare setting interactions and candidate sources against unmodified C# APIs."""
import argparse,ctypes as c,hashlib,json,os,subprocess
from pathlib import Path

def keys(text):
    return [[ord(ch.upper()) if ch.isascii() and ch.isalpha() else 0,ord(ch),int(ch.isupper())] for ch in text]

def cases():
    base=dict(SettingsVersion=5,LearningEnabled=False,LiveConversion=False,PredictiveCandidates=False,TabConversion=False)
    rows=[]
    def add(name,raw,tail=None,settings=None,files=None,before='',after='',converter=True):
        row=dict(name=name,settings={**base,**(settings or {})},before=before,after=after,canDelete=True,keys=keys(raw)+(tail if tail is not None else [[32,32,0],[40,0,0],[13,0,0]]))
        if files:row['files']=files
        if converter:
            row['converter']=dict(clauses={r:[[r,t]] for r,t in [('はし','橋'),('えがお','笑顔'),('き','木'),('あつい','暑い'),('めると','既存'),('たす','足す'),('りなっくす','リナックス'),('あどばいす','アドバイス')]},candidates={'はし':['橋','箸','端','はし'],'えがお':['笑顔','😀'],'き':['木','気'],'めると':['既存','追加']})
        rows.append(row)
    add('homophones','hashi')
    add('translation-meaning','ki',settings={'TranslationCandidates':True,'ShowCandidateMeanings':True})
    add('translations-off','ki',settings={'TranslationCandidates':False,'ShowCandidateMeanings':False})
    add('emoji','egao')
    add('loanword','rinakkusu')
    add('context','atsui',before='この料理は',after='気温')
    add('custom-candidate','meruto',files={'dictionaries/candidates.txt':'めると 特別候補 第二候補\n'})
    add('custom-emoji','egao',files={'dictionaries/emoji.txt':'えがお\t[# test]\t( hello world )\n'})
    add('custom-context','hashi',files={'dictionaries/contexts.txt':'はし 特別 : 試験\n'},before='試験')
    add('user-dictionary','meruto',files={'userdict.txt':'めると\tMeltype\nめると\tめるタイプ\n'})
    add('history','hashi',files={'conversions.json':json.dumps({'はし':{'Text':'端','Used':'2026-10-20T00:00:00Z'}})})
    add('translation-history','ki',files={'translations.json':json.dumps({'木':{'wood':20}})})
    add('prediction','kyo',[[9,0,0],[13,0,0]],{'PredictiveCandidates':True},files={'phrases.txt':'きょうのよてい\t今日の予定\t5\t639000000000000000\n'})
    add('english-prediction','goog',[[9,0,0],[13,0,0]],{'PredictiveCandidates':True})
    add('spelling','shumire-shon',[[9,0,0],[32,32,0],[13,0,0]])
    add('math','tasu',before='1')
    add('no-external','hashi',converter=False)
    add('jis-kana','tf',settings={'InputStyle':'Kana'})
    add('live','hashi',settings={'LiveConversion':True})
    add('tab-conversion','hashi',[[9,0,0],[9,0,0],[13,0,0]],{'TabConversion':True})
    return rows

def projection(reply):
    view=reply['view']
    if view:
        # Inactive indices are unsigned in the current Rust ABI. Do not hide
        # differences in active selection, candidates, annotations or prediction.
        view={k:view[k] for k in ['text','converting','candidates','clauses','suggestion','meaning','notes','predictions','selectedPrediction','typed']}
        if view['converting']:
            view.update({k:reply['view'][k] for k in ['selectedClause','selectedIndex']})
    return dict(consumed=reply['consumed'],commits=reply['commits'],view=view)

def native(dll,rows,directory):
    api=c.CDLL(str(dll.resolve()));p=c.c_void_p
    allocator=c.CDLL('ucrtbase') if os.name=='nt' else c.CDLL(None)
    allocator.malloc.argtypes=[c.c_size_t];allocator.malloc.restype=p
    def allocate(text):
        data=text.encode()+b'\0';ptr=allocator.malloc(len(data));assert ptr;c.memmove(ptr,data,len(data));return ptr
    clause_type=c.CFUNCTYPE(p,c.c_char_p,c.c_char_p);candidate_type=c.CFUNCTYPE(p,c.c_char_p)
    current={}
    @clause_type
    def clauses(raw,context):
        return allocate('\n'.join('\t'.join(part) for part in current.get('clauses',{}).get(raw.decode(),[])))
    @candidate_type
    def candidates(raw):return allocate('\n'.join(current.get('candidates',{}).get(raw.decode(),[])))
    api.meltype_init.argtypes=[clause_type,candidate_type,p];api.meltype_init.restype=c.c_int
    assert api.meltype_init(clauses,candidates,None)==1
    api.meltype_create_at.argtypes=[c.c_char_p];api.meltype_create_at.restype=p
    api.meltype_set_can_delete.argtypes=[p,c.c_int]
    api.meltype_handle_key.argtypes=[p,c.c_int,c.c_int,c.c_int,c.c_char_p,c.c_char_p];api.meltype_handle_key.restype=p
    api.meltype_destroy.argtypes=[p];api.meltype_free.argtypes=[p]
    result=[]
    for index,row in enumerate(rows):
        profile=directory/str(index);profile.mkdir(parents=True)
        (profile/'config.json').write_text(json.dumps(row['settings']),encoding='utf-8')
        for name,text in row.get('files',{}).items():
            path=profile/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8')
        current=row.get('converter',{})
        handle=api.meltype_create_at(str(profile).encode());assert handle
        api.meltype_set_can_delete(handle,int(row['canDelete']))
        replies=[]
        try:
            for key in row['keys']:
                ptr=api.meltype_handle_key(handle,*key,row['before'].encode(),row['after'].encode());assert ptr
                try:replies.append(json.loads(c.string_at(ptr)))
                finally:api.meltype_free(ptr)
        finally:api.meltype_destroy(handle)
        result.append(dict(name=row['name'],expected=replies))
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--csharp',type=Path,required=True);parser.add_argument('--dll',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True);parser.add_argument('--cases',type=Path)
    parser.add_argument('--reference',type=Path);parser.add_argument('--reference-only',action='store_true')
    args=parser.parse_args();out=args.output;out.mkdir(parents=True,exist_ok=False)
    rows=json.loads(args.cases.read_text(encoding='utf-8')) if args.cases else cases()
    source=out/'cases.json';source.write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
    if args.reference:
        assert json.loads((args.reference/'cases.json').read_text(encoding='utf-8'))==rows
        meta=json.loads((args.reference/'reference.json').read_text(encoding='utf-8'))
        assert meta['csharp_sha256']==hashlib.sha256(args.csharp.read_bytes()).hexdigest()
        assert meta['capture_sha256']==hashlib.sha256((args.reference/'csharp.jsonl').read_bytes()).hexdigest()
        (out/'csharp.jsonl').write_bytes((args.reference/'csharp.jsonl').read_bytes())
    else:
        with (out/'csharp.jsonl').open('w',encoding='utf-8') as log:
            subprocess.run(['dotnet',str(args.csharp.resolve()),str(source.resolve()),str((out/'csharp-profiles').resolve())],stdout=log,check=True)
    (out/'reference.json').write_text(json.dumps(dict(csharp_sha256=hashlib.sha256(args.csharp.read_bytes()).hexdigest(),capture_sha256=hashlib.sha256((out/'csharp.jsonl').read_bytes()).hexdigest())),encoding='utf-8')
    if args.reference_only:return
    expected=[json.loads(x) for x in (out/'csharp.jsonl').read_text(encoding='utf-8-sig').splitlines()]
    actual=native(args.dll,rows,out/'rust-profiles')
    (out/'rust.jsonl').write_text('\n'.join(map(json.dumps,actual))+'\n',encoding='utf-8')
    assert len(rows)==len(expected)==len(actual)
    checks=[]
    for row,want,got in zip(rows,expected,actual,strict=True):
        assert row['name']==want['name']==got['name'];assert len(row['keys'])==len(want['expected'])==len(got['expected'])
        bad=[dict(index=i,key=key,expected=projection(a),actual=projection(b)) for i,(key,a,b) in enumerate(zip(row['keys'],want['expected'],got['expected'],strict=True)) if projection(a)!=projection(b)]
        checks.append(dict(name=row['name'],matches=not bad,mismatches=bad))
    report=dict(checks=checks,sha256={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in [args.csharp,args.dll,Path(__file__),Path('tools/parity/settings-candidates/Program.cs')]})
    (out/'report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
    print('matches',sum(x['matches'] for x in checks),'/',len(checks),flush=True)
    for row in checks:
        if row['mismatches']:
            first=row['mismatches'][0];print(row['name'],'key',first['index'],[k for k in ['consumed','commits','view'] if first['expected'][k]!=first['actual'][k]],flush=True)
    raise SystemExit(not all(x['matches'] for x in checks))

if __name__=='__main__':main()
