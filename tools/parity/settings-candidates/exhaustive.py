# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Resumable Cartesian traversal over DECLARED test domains, never all arbitrary values."""
import argparse,json,math,hashlib
from pathlib import Path
from matrix import domains
from check import cases
p=argparse.ArgumentParser(description=__doc__);p.add_argument('output',type=Path)
p.add_argument('--offset',type=int,default=0);p.add_argument('--limit',type=int,default=128)
p.add_argument('--scenes', choices=['basic', 'all'], default='basic')
a=p.parse_args();assert a.offset>=0 and a.limit>0
values=domains();names=list(values);total=math.prod(map(len,values.values()));assert a.offset<=total
# A coprime stride visits each configuration exactly once and spreads each batch
# across settings; offset is the traversal position, independent of key order.
stride=total*618033988749894848//1000000000000000000
stride|=1
while math.gcd(stride,total)!=1:stride+=2
end=min(total,a.offset+a.limit);rows=[]
scenes=cases() if a.scenes=='all' else [cases()[0],cases()[12]]
for position in range(a.offset,end):
    index=position*stride%total;remainder=index;configuration={}
    for name in names:
        size=len(values[name]);remainder,digit=divmod(remainder,size);configuration[name]=values[name][digit]
    for original in scenes:
        scene=json.loads(json.dumps(original))
        scene['name']=f'cartesian-{position}-{index}-'+scene['name'];scene['settings']=configuration;rows.append(scene)
a.output.mkdir(parents=True,exist_ok=False)
(a.output/'cases.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
report=dict(domains_sha256=hashlib.sha256(json.dumps(values,ensure_ascii=False,separators=(',',':')).encode()).hexdigest(),settings_order=names,settings=len(names),declared_cartesian_configurations=str(total),offset=str(a.offset),next_offset=str(end),stride=str(stride),batch_configurations=end-a.offset,batch_scenarios=len(rows),scenes=a.scenes,scenes_per_configuration=len(scenes),traversal_complete=end==total,arbitrary_numeric_and_string_values_covered=False,scope='Only explicitly declared finite domains; input histories and OS API responses are separate dimensions.')
(a.output/'coverage.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(report,ensure_ascii=False))
