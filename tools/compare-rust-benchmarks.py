#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare two frozen Windows Rust benchmark hosts, using the same input/order."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import statistics
import sys
import datetime

ROOT=Path(__file__).resolve().parent.parent
spec=importlib.util.spec_from_file_location("windows_benchmark",ROOT/"tools/benchmark-windows.py")
benchmark=importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)

def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument("--old",type=Path,required=True)
    p.add_argument("--new",type=Path,required=True)
    p.add_argument("--old-ref",required=True)
    p.add_argument("--new-ref",required=True)
    p.add_argument("--output",type=Path,required=True)
    p.add_argument("--samples",type=int,default=5)
    p.add_argument("--warmups",type=int,default=2)
    args=p.parse_args()
    if args.samples<3 or args.warmups<1:p.error("samples>=3 and warmups>=1 required")
    output=args.output.resolve()
    output.mkdir(parents=True,exist_ok=True)
    if (output/"report.json").exists():p.error("choose a new output directory")
    binaries={}
    for label,path in [("old",args.old),("new",args.new)]:
        target=output/f"{label}.exe"
        shutil.copyfile(path.resolve(),target)
        binaries[label]=target
    hashes={k:sha(v) for k,v in binaries.items()}
    cases=benchmark.fixtures()
    inputs=output/"cases.json"
    inputs.write_text(json.dumps(cases,ensure_ascii=False,indent=2),encoding="utf-8")
    rows=[];resources=[];traces={};order=[]
    env=os.environ.copy();env["MELTYPE_BENCH_PROGRESS"]="1"
    for round_index in range(2):
        for label in (["old","new"] if round_index==0 else ["new","old"]):
            order.append(label);print(f"Round {round_index+1}/2: {label}",flush=True)
            raw=output/f"{label}-{round_index}.jsonl"
            with raw.open("w",encoding="utf-8") as log:
                usage=benchmark.measure_process([str(binaries[label]),str(inputs),str(args.samples),str(args.warmups)],cwd=ROOT,env=env,stdout=log)
            resources.append(dict(version=label,round=round_index,**usage))
            for line in raw.read_text(encoding="utf-8-sig").splitlines():
                row=json.loads(line);case=next(c for c in cases if c["name"]==row["case"])
                trace=benchmark.verify(row["responses"],case["keys"])
                key=(label,row["case"])
                if key in traces and traces[key]!=trace:raise ValueError(f"unstable output: {key}")
                traces[key]=trace
                rows.append(dict(version=label,round=round_index,case=row["case"],total_ms=row["total_ms"]))
    unchanged=hashes=={k:sha(v) for k,v in binaries.items()}
    results=[]
    for case in cases:
        name=case["name"]
        medians={label:statistics.median(row["total_ms"] for row in rows if row["version"]==label and row["case"]==name) for label in binaries}
        same=traces[("old",name)]==traces[("new",name)]
        results.append(dict(case=name,medians_ms=medians,new_over_old=medians["new"]/medians["old"],matching_projection=same,eligible_for_speed_claim=same and unchanged))
    report=dict(measured_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),old_ref=args.old_ref,new_ref=args.new_ref,
                samples=args.samples,warmups=args.warmups,rounds=2,order=order,binary_sha256=hashes,binaries_unchanged=unchanged,
                rustc=benchmark.command(["rustc","+stable","-Vv"]),results=results,process_resources=resources,
                scope="In-memory keys and JSON, no OS engine/live conversion/prediction/learning. Same compiler must be used when building both binaries. Source/dependency revisions differ; this does not isolate one code change.")
    (output/"report.json").write_text(json.dumps(report,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    print(json.dumps(results,ensure_ascii=False,indent=2),flush=True)

if __name__=="__main__":main()
