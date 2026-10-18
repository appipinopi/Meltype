#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare original legacy InputSession transitions, key replay and feedback.

Synthetic clocks and in-memory queues only. No input injection or OS hooks.
Contribution source/scores are compared; diagnostic contribution prose is not.
Decision summaries and interruption reasons are compared exactly.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def project(row):
    row = json.loads(json.dumps(row))
    decisions = [row.get("decision")] + [f["result"] for f in row["flushes"]]
    for decision in decisions:
        if decision:
            for contribution in decision["contributions"]:
                contribution.pop("reason", None)
    return row


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--csharp", type=Path, required=True)
    p.add_argument("--rust", type=Path, required=True)
    p.add_argument("--cases", type=Path, required=True)
    p.add_argument("--output", type=Path, required=True)
    args = p.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    cases = json.loads(args.cases.read_text(encoding="utf-8"))
    if isinstance(cases, dict):
        cases = cases["cases"]
    fixture = output / "cases.json"
    fixture.write_text(json.dumps(cases, ensure_ascii=False, indent=2), encoding="utf-8")
    rows = {}
    for label, cmd in [("csharp", ["dotnet", str(args.csharp.resolve())]), ("rust", [str(args.rust.resolve())])]:
        with (output / f"{label}.jsonl").open("w", encoding="utf-8") as log:
            subprocess.run(cmd + [str(fixture)], stdout=log, check=True)
        rows[label] = {r["name"]: r for line in (output / f"{label}.jsonl").read_text(encoding="utf-8-sig").splitlines()
                       for r in [json.loads(line)]}
        if len(rows[label]) != len(cases):
            raise RuntimeError("missing or duplicate cases")
    results = []
    for case in cases:
        reference, current = [rows[label][case["name"]]["responses"] for label in rows]
        if len(reference) != len(case["steps"]) or len(current) != len(reference):
            raise RuntimeError("wrong response count")
        differences = [dict(index=i, step=case["steps"][i], expected=project(a), actual=project(b))
                       for i, (a, b) in enumerate(zip(reference, current, strict=True)) if project(a) != project(b)]
        if rows["csharp"][case["name"]]["model"] != rows["rust"][case["name"]]["model"]:
            differences.append(dict(index="model",expected=rows["csharp"][case["name"]]["model"],actual=rows["rust"][case["name"]]["model"]))
        results.append(dict(name=case["name"], differences=differences))
    report = dict(total=len(cases), passed=sum(not r["differences"] for r in results), cases=results,
                  scope=__doc__, baseline="45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46",
                  sha256={name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in
                          [("csharp", args.csharp), ("rust", args.rust), ("cases", args.cases)]})
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print(f"Original hook-session traces: {report['passed']}/{report['total']}")
    for case in results:
        if case["differences"]:
            print(case["name"], json.dumps(case["differences"][0], ensure_ascii=False))
    if report["passed"] != report["total"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
