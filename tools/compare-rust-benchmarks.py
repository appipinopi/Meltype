#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Compare previous/current Rust releases with identical compiler, fixtures and order."""
import argparse
import datetime
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("windows_benchmark", ROOT / "tools/benchmark-windows.py")
benchmark = importlib.util.module_from_spec(spec)
spec.loader.exec_module(benchmark)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def version_key(value):
    match = re.fullmatch(r"(?:flash-v|rust-v)?(\d+)\.(\d+)\.(\d+)(?:-(?:preview\.|pre\.)(\d+))?", value)
    if not match:
        return None
    major, minor, patch, preview = match.groups()
    return (int(major), int(minor), int(patch), int(preview is None), int(preview or 0))


def previous_release():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]["version"]
    current = version_key(version)
    if current is None:
        raise ValueError(f"Unsupported workspace version: {version}; set --baseline-ref explicitly")
    tags = benchmark.command(["git", "tag", "--list"]).splitlines()
    eligible = [(version_key(tag), tag) for tag in tags if tag.startswith(("flash-v", "rust-v"))]
    eligible = [(key, tag) for key, tag in eligible if key is not None and key < current]
    if not eligible:
        raise ValueError(f"No release older than {version}; fetch release tags or set --baseline-ref")
    return max(eligible)[1]


def source_hashes(root):
    paths = [root / "Cargo.toml", root / "Cargo.lock"]
    paths += [p for directory in ("rust/core", "dictionaries") for p in (root / directory).rglob("*") if p.is_file()]
    if root == ROOT:
        paths += [ROOT / "tools" / name for name in ("compare-rust-benchmarks.py", "benchmark-windows.py", "performance/windows/resources.py")]
    return {p.relative_to(root).as_posix(): sha(p) for p in sorted(paths)}


def build_hosts(args, output):
    baseline = previous_release() if args.baseline_ref == "auto" else args.baseline_ref
    old_commit = benchmark.command(["git", "rev-parse", "--verify", baseline + "^{commit}"])
    new_commit = benchmark.command(["git", "rev-parse", "HEAD"])
    baseline_root = output / "baseline-source"
    baseline_root.mkdir()
    archive = subprocess.check_output(["git", "archive", old_commit, "Cargo.toml", "Cargo.lock", "rust", "dictionaries"], cwd=ROOT)
    with tarfile.open(fileobj=io.BytesIO(archive)) as files:
        files.extractall(baseline_root, filter="data")
    roots = {"old": baseline_root, "new": ROOT}
    snapshots = {label: source_hashes(root) for label, root in roots.items()}
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")).resolve()
    binaries = {}
    build_commands = {}
    # Freeze current first: the old build may reuse/replace the same example output.
    for label in ("new", "old"):
        command = ["cargo", f"+{args.rust_toolchain}", "build", "--manifest-path", str(roots[label] / "Cargo.toml"),
                   "-p", "meltype-core", "--example", "benchmark", "--release", "--locked", "--target-dir", str(target)]
        build_commands[label] = command
        print(f"Building {label}: {baseline if label == 'old' else new_commit[:12]}", flush=True)
        with (output / f"{label}-build.log").open("w", encoding="utf-8") as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = output / f"{label}.exe"
        shutil.copyfile(target / "release/examples/benchmark.exe", binary)
        binaries[label] = binary
    dictionaries = {label: {p.name: sha(p) for p in sorted((root / "dictionaries").glob("*.txt"))} for label, root in roots.items()}
    metadata = dict(old_ref=baseline, new_ref=new_commit, old_commit=old_commit, new_commit=new_commit,
                    working_tree=benchmark.command(["git", "status", "--short"]), build_commands=build_commands,
                    source_sha256=snapshots, dictionaries=dictionaries,
                    dictionaries_match=dictionaries["old"] == dictionaries["new"],
                    dictionary_policy="Each version retains its own bundled dictionaries; differences are part of the version comparison.")
    return binaries, metadata, roots, snapshots


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--old", type=Path)
    parser.add_argument("--new", type=Path)
    parser.add_argument("--old-ref")
    parser.add_argument("--new-ref")
    parser.add_argument("--build", action="store_true", help="Build current and previous release using one toolchain and shared dependencies")
    parser.add_argument("--baseline-ref", default="auto", help="Previous release tag/commit, or auto from the workspace version")
    parser.add_argument("--rust-toolchain", default="stable")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--rounds", type=int, default=2)
    args = parser.parse_args()
    if args.samples < 3 or args.warmups < 1 or args.rounds < 2:
        parser.error("samples>=3, warmups>=1 and rounds>=2 required")
    if os.name != "nt":
        parser.error("CPU/RAM accounting requires Windows")
    if args.build and any((args.old, args.new, args.old_ref, args.new_ref)):
        parser.error("--build cannot be combined with prebuilt binary arguments")
    if not args.build and not all((args.old, args.new, args.old_ref, args.new_ref)):
        parser.error("specify --build, or --old/--new/--old-ref/--new-ref")
    output = args.output.resolve()
    # A fresh directory preserves evidence and prevents reusing partial builds.
    output.mkdir(parents=True, exist_ok=False)
    if args.build:
        binaries, metadata, roots, snapshots = build_hosts(args, output)
    else:
        binaries = {}
        for label, path in (("old", args.old), ("new", args.new)):
            binaries[label] = output / f"{label}.exe"
            shutil.copyfile(path.resolve(), binaries[label])
        metadata = dict(old_ref=args.old_ref, new_ref=args.new_ref)
        roots = snapshots = {}
    hashes = {label: sha(path) for label, path in binaries.items()}
    cases = benchmark.fixtures()
    inputs = output / "cases.json"
    inputs.write_text(json.dumps(cases, ensure_ascii=False, indent=2), encoding="utf-8")
    rows, resources, traces, order = [], [], {}, []
    env = os.environ.copy()
    env["MELTYPE_BENCH_PROGRESS"] = "1"
    for round_index in range(args.rounds):
        for label in (("old", "new") if round_index % 2 == 0 else ("new", "old")):
            order.append(label)
            print(f"Round {round_index + 1}/{args.rounds}: {label}", flush=True)
            raw = output / f"{label}-{round_index}.jsonl"
            with raw.open("w", encoding="utf-8") as log:
                usage = benchmark.measure_process([str(binaries[label]), str(inputs), str(args.samples), str(args.warmups)], cwd=ROOT, env=env, stdout=log)
            resources.append(dict(version=label, round=round_index, **usage))
            (output / "resources.json").write_text(json.dumps(resources, indent=2) + "\n", encoding="utf-8")
            for line in raw.read_text(encoding="utf-8-sig").splitlines():
                row = json.loads(line)
                case = next(case for case in cases if case["name"] == row["case"])
                trace = benchmark.verify(row["responses"], case["keys"])
                key = label, row["case"]
                if key in traces and traces[key] != trace:
                    raise ValueError(f"unstable output: {key}")
                traces[key] = trace
                rows.append(dict(version=label, round=round_index, case=row["case"], sample=row["sample"], total_ms=row["total_ms"]))
            expected = {(case["name"], sample) for case in cases for sample in range(args.samples)}
            observed = [(row["case"], row["sample"]) for row in rows if row["version"] == label and row["round"] == round_index]
            if len(observed) != len(expected) or set(observed) != expected:
                raise ValueError(f"Missing/duplicate observations: {label}, round {round_index}")
    unchanged = hashes == {label: sha(path) for label, path in binaries.items()}
    sources_unchanged = all(source_hashes(root) == snapshots[label] for label, root in roots.items()) if roots else None
    accepted = unchanged and sources_unchanged is not False
    results = []
    for case in cases:
        name = case["name"]
        medians = {label: statistics.median(row["total_ms"] for row in rows if row["version"] == label and row["case"] == name) for label in binaries}
        same = traces["old", name] == traces["new", name]
        results.append(dict(case=name, keys=len(case["keys"]), medians_ms=medians,
                            new_over_old=medians["new"] / medians["old"], matching_projection=same,
                            output_matches=traces["old", name][0] == traces["new", name][0],
                            eligible_for_speed_claim=same and accepted))
    report = dict(measured_at=datetime.datetime.now(datetime.timezone.utc).isoformat(), **metadata,
                  comparison_kind="rust-versions", machine=platform.platform(), cpu=platform.processor(),
                  samples=args.samples, warmups=args.warmups, rounds=args.rounds, order=order,
                  binary_sha256=hashes, binaries_unchanged=unchanged, sources_unchanged=sources_unchanged,
                  accepted_performance_baseline=accepted, fixtures_sha256=sha(inputs),
                  rustc=benchmark.command(["rustc", f"+{args.rust_toolchain}", "-Vv"]),
                  results=results, rows=rows, process_resources=resources,
                  scope="In-memory keys and JSON; no OS engine/live conversion/prediction/learning. Same compiler for both builds. Source/dependency/dictionary revisions differ; this does not isolate one code change.")
    (output / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(results, ensure_ascii=False, indent=2), flush=True)


if __name__ == "__main__":
    main()
