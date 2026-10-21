# SPDX-License-Identifier: GPL-3.0-or-later
"""Deterministic pair coverage plus exhaustive interacting Core switch groups."""
from itertools import combinations,product
import json
from pathlib import Path
from check import cases,keys

def domains():
    defaults=json.loads(Path('rust/core/data/settings-defaults.json').read_text(encoding='utf-8'))
    ui=json.loads(Path('rust/settings-app/settings-ui.json').read_text(encoding='utf-8'))
    fields={f['key']:f for f in ui['fields']}
    result={}
    for name,value in defaults.items():
        if isinstance(value,bool):values=[False,True]
        elif isinstance(value,int):values=list(dict.fromkeys([value,0,99999]))
        elif isinstance(value,str):
            enum=ui['enums'].get(fields.get(name,{}).get('type',''))
            values=list(enum) if enum else list(dict.fromkeys([value,'matrix.exe' if name.endswith('Apps') else '#183A5C' if 'Color' in name else 'matrix']))
        elif name=='Profiles':values=[[],[{'Name':'標準','Values':None},{'Name':'仕事','Values':{'LiveConversion':False,'DetectionLevel':'Manual'}}]]
        elif name=='AppKinds':values=[[],[{'Name':'matrix','Base':'Code','LiveConversion':False,'DetectionLevel':'Conservative'}]]
        elif name=='AppRules':values=[[],[{'Process':'matrix.exe','Enabled':False,'Profile':'Code','Kind':'matrix'}],value]
        else:raise AssertionError(name)
        result[name]=values
    return result

def covering_rows(domains):
    names=list(domains);sizes=[len(domains[k]) for k in names]
    rows=[list(values) for values in product(range(sizes[0]),range(sizes[1]))]
    for index in range(2,len(names)):
        remaining={(j,a,b) for j in range(index) for a in range(sizes[j]) for b in range(sizes[index])}
        for row in rows:
            best=max(range(sizes[index]),key=lambda b:sum((j,a,b) in remaining for j,a in enumerate(row)))
            row.append(best)
            remaining.difference_update((j,row[j],best) for j in range(index))
        while remaining:
            j,a,b=min(remaining);row=[None]*(index+1);row[j]=a;row[index]=b
            for other in range(index):
                if row[other] is None:row[other]=max(range(sizes[other]),key=lambda value:(other,value,b) in remaining)
            remaining.difference_update((other,row[other],b) for other in range(index));rows.append(row)
    expected={(i,a,j,b) for i,j in combinations(range(len(names)),2) for a in range(sizes[i]) for b in range(sizes[j])}
    actual={(i,row[i],j,row[j]) for row in rows for i,j in combinations(range(len(names)),2)}
    assert actual==expected
    return [{name:domains[name][row[i]] for i,name in enumerate(names)} for row in rows],len(expected)

def generate():
    domain=domains();configs,pairs=covering_rows(domain)
    scenes=cases();rows=list(scenes)
    for index,config in enumerate(configs):
        for scene in [scenes[0],scenes[12]]:
            row=json.loads(json.dumps(scene));row['settings']=config;row['name']=f'pair-{index}-{scene["name"]}';rows.append(row)
    switches=['TranslationCandidates','ShowCandidateMeanings','PredictiveCandidates','TabConversion','LiveConversion','SeparateKeyRoles','ContinueAfterConversion']
    for index,values in enumerate(product([False,True],repeat=len(switches))):
        for scene in [scenes[0],scenes[3],scenes[12]]:
            row=json.loads(json.dumps(scene));row['settings'].update(zip(switches,values));row['name']=f'interaction-{index}-{scene["name"]}';rows.append(row)
    for index,(level,scored,style,learning) in enumerate(product(['Manual','Balanced','Conservative','Aggressive'],[False,True],['Romaji','Kana'],[False,True])):
        for scene in [scenes[6],scenes[8],scenes[10],scenes[13],scenes[19]]:
            row=json.loads(json.dumps(scene));row['settings'].update(DetectionLevel=level,ScoredSegmentation=scored,InputStyle=style,LearningEnabled=learning);row['name']=f'input-{index}-{scene["name"]}';rows.append(row)
    coverage=dict(settings=len(domain),pairwise_configurations=len(configs),covered_value_pairs=pairs,interaction_switches=switches,interaction_configurations=2**len(switches),input_configurations=32,cases=len(rows),domains=domain)
    return rows,coverage

if __name__=='__main__':
    import argparse
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('output',type=Path);args=parser.parse_args()
    rows,coverage=generate();args.output.mkdir(parents=True,exist_ok=False)
    (args.output/'cases.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
    (args.output/'coverage.json').write_text(json.dumps(coverage,ensure_ascii=False,indent=2),encoding='utf-8')
    print({k:v for k,v in coverage.items() if k!='domains'})
