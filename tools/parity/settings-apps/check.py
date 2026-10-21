# SPDX-License-Identifier: GPL-3.0-or-later
import argparse,itertools,json,subprocess
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--csharp',type=Path,required=True);parser.add_argument('--rust',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
args.output.mkdir(parents=True,exist_ok=False)
rows=[]
for index,(profile,root_enabled,rule_enabled,looks,stop,kind,process) in enumerate(itertools.product(['General','Code','Game'],[False,True],[False,True],[False,True],[False,True],[False,True],['matrix.exe','MATRIX.EXE','matrix','other.exe'])):
    config=dict(SettingsVersion=5,Enabled=root_enabled,StopInGames=stop,DetectionLevel='Aggressive',LiveConversion=True,
        AppRules=[dict(Process='matrix.exe',Enabled=rule_enabled,Profile=profile,Kind='custom' if kind else None)],
        AppKinds=[dict(Name='custom',Base=profile,DetectionLevel='Conservative',LiveConversion=False,StartInEnglish=True)])
    rows.append(dict(name=str(index),settings=config,process=process,looksLikeGame=looks))
source=args.output/'cases.json';source.write_text(json.dumps(rows),encoding='utf-8')
original=subprocess.check_output(['dotnet',str(args.csharp),str(source),str(args.output/'original-settings')],encoding='utf-8-sig')
actual=subprocess.check_output([str(args.rust),'apps',str(args.output/'rust-settings'),str(source)],encoding='utf-8-sig')
(args.output/'csharp.jsonl').write_text(original,encoding='utf-8');(args.output/'rust.jsonl').write_text(actual,encoding='utf-8')
bad=[dict(expected=a,actual=b) for a,b in zip(map(json.loads,original.splitlines()),map(json.loads,actual.splitlines()),strict=True) if a!=b]
(args.output/'report.json').write_text(json.dumps(dict(total=len(rows),matches=len(rows)-len(bad),mismatches=bad),indent=2),encoding='utf-8')
print(len(rows)-len(bad),'/',len(rows));raise SystemExit(bool(bad))
