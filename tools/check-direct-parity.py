#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare timed direct-input replay hosts; no OS registration or typing."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def project(value):
    response = value["response"]
    view = response.get("view")
    return dict(consumed=response["consumed"], commits=response["commits"],
                text=view["text"] if view else None,
                direct=value["direct"], english=value["english"], document=value["document"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--csharp", type=Path, required=True, help="reference Direct.dll")
    parser.add_argument("--rust", type=Path, required=True, help="direct_trace executable")
    parser.add_argument("--cases", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if (output / "report.json").exists():
        parser.error("choose a fresh output directory")
    cases = json.loads(args.cases.read_text(encoding="utf-8"))
    if isinstance(cases, dict):
        cases = cases["cases"]
    fixture = output / "cases.json"
    fixture.write_text(json.dumps(cases, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    rows = {}
    for label, command in [("csharp", ["dotnet", str(args.csharp.resolve())]),
                           ("rust", [str(args.rust.resolve())])]:
        with (output / f"{label}.jsonl").open("w", encoding="utf-8") as log:
            subprocess.run(command + [str(fixture)], stdout=log, check=True)
        rows[label] = {row["name"]: row for line in
                       (output / f"{label}.jsonl").read_text(encoding="utf-8-sig").splitlines()
                       for row in [json.loads(line)]}
        if len(rows[label]) != len(cases):
            raise RuntimeError("missing or duplicate cases")
    results = []
    for case in cases:
        a, b = [rows[label][case["name"]]["responses"] for label in rows]
        differences = [dict(index=i, expected=project(ar), actual=project(br))
                       for i, (ar, br) in enumerate(zip(a, b, strict=True))
                       if project(ar) != project(br)]
        results.append(dict(name=case["name"], differences=differences))
    report = dict(total=len(cases), passed=sum(not r["differences"] for r in results), cases=results,
                  sha256={label: hashlib.sha256(path.read_bytes()).hexdigest() for label, path in
                          [("csharp", args.csharp), ("rust", args.rust), ("cases", args.cases)]})
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    print(f"Timed direct-input traces: {report['passed']}/{report['total']}")
    for result in results:
        if result["differences"]:
            print(result["name"], json.dumps(result["differences"][0], ensure_ascii=False))
    if report["passed"] != report["total"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
