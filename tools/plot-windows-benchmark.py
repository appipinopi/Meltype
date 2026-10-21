# SPDX-License-Identifier: GPL-3.0-or-later
"""Render all common-core cases and process resources, retaining every outlier.

Requires matplotlib and numpy. Accepts the full benchmark report or its compact
summary.json output, which retains every total-time and resource observation.
"""
import argparse
import csv
import hashlib
import json
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D
from matplotlib.patches import Patch
import numpy as np

VERSIONS = ("csharp", "rust")
LABELS = ("C# NativeAOT", "Rust Release")
COLORS = ("#da8745", "#387cac")
IMAGES = ["speed-32-64.png", "speed-128-256.png", "speed-short-edit.png", "cpu.png", "ram.png"]


def distribution(values):
    values = np.asarray(values, dtype=float)
    if not len(values) or not np.isfinite(values).all():
        raise ValueError("A nonempty, finite sample is required")
    q1, med, q3 = np.percentile(values, [25, 50, 75], method="linear")
    iqr = q3 - q1
    inliers = values[(values >= q1 - 1.5 * iqr) & (values <= q3 + 1.5 * iqr)]
    fliers = values[(values < q1 - 1.5 * iqr) | (values > q3 + 1.5 * iqr)]
    return dict(n=len(values), q1=float(q1), med=float(med), q3=float(q3),
                whislo=float(inliers.min()), whishi=float(inliers.max()),
                fliers=fliers.tolist(), minimum=float(values.min()), maximum=float(values.max()))


def boxes(ax, groups, ylabel, title, *, log=False):
    stats = [distribution(groups[v]) for v in VERSIONS]
    drawing = ax.bxp(stats, positions=[1, 2], widths=.48, patch_artist=True, showfliers=True,
                     manage_ticks=False, medianprops=dict(color="#111111", linewidth=2.2),
                     flierprops=dict(marker="D", markersize=5, markerfacecolor="none",
                                     markeredgecolor="#a52f35", alpha=.9))
    for box, color in zip(drawing["boxes"], COLORS):
        box.set_facecolor(color)
        box.set_alpha(.48)
    # Show every observation too. Fixed offsets make rerenders deterministic.
    for position, version in enumerate(VERSIONS, 1):
        values = groups[version]
        offsets = np.linspace(-.1, .1, len(values))
        ax.scatter(position + offsets, values, s=13, color=COLORS[position - 1], alpha=.42, zorder=2)
        stat = stats[position - 1]
        ax.text(position, .96, f"median {stat['med']:.4g}\nn={stat['n']}, outliers={len(stat['fliers'])}",
                transform=ax.get_xaxis_transform(), ha="center", va="top", fontsize=10,
                bbox=dict(facecolor="white", edgecolor="none", alpha=.87), zorder=5)
    if log:
        if any(value <= 0 for group in groups.values() for value in group):
            raise ValueError("Logarithmic time axes require positive times")
        ax.set_yscale("log")
        ax.set_ylim(top=max(max(g) for g in groups.values()) * 5)
    else:
        bottom, top = ax.get_ylim()
        ax.set_ylim(bottom=max(0, bottom), top=top + (top - bottom) * .24)
    ax.set_xticks([1, 2], LABELS)
    ax.set_xlim(.45, 2.55)
    ax.set_title(title, fontsize=12, pad=10)
    ax.set_ylabel(ylabel)
    ax.grid(axis="y", alpha=.22, which="both")
    ax.spines[["top", "right"]].set_visible(False)
    return {v: stat for v, stat in zip(VERSIONS, stats)}


def finish(fig, path, title, note):
    fig.suptitle(title, fontsize=15, fontweight="bold", y=.985)
    fig.legend(handles=[Patch(facecolor=c, alpha=.55, label=l) for c, l in zip(COLORS, LABELS)] + [
        Line2D([0], [0], color="#111111", linewidth=2.2, label="Median"),
        Line2D([0], [0], linestyle="none", marker="D", markerfacecolor="none",
               markeredgecolor="#a52f35", label="Outlier (outside 1.5 x IQR)")],
        loc="upper center", bbox_to_anchor=(.5, .96), ncol=4, frameon=False)
    fig.text(.5, .012, note, ha="center", va="bottom", fontsize=9)
    fig.subplots_adjust(left=.085, right=.975, bottom=.105, top=.84, hspace=.65, wspace=.25)
    fig.savefig(path, dpi=170, facecolor="white", metadata={"Software": "Meltype benchmark plotter"})
    plt.close(fig)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--hardware", type=Path, help="Optional Windows processor metadata JSON")
    args = parser.parse_args()
    report = json.loads(args.report.read_text(encoding="utf-8"))
    if not report["accepted_performance_baseline"]:
        raise ValueError("Source/binary integrity verification failed; refuse performance charts")
    if args.hardware:
        report["hardware"] = json.loads(args.hardware.read_text(encoding="utf-8-sig"))
    output = args.output
    output.mkdir(parents=True, exist_ok=True)
    if any((output / name).exists() for name in IMAGES):
        raise ValueError("Choose a new output directory; existing charts are preserved")
    plt.rcParams.update({"font.family": "DejaVu Sans", "font.size": 9, "axes.titleweight": "bold"})
    compact = {k: v for k, v in report.items() if k not in ("rows", "evidence")}
    compact["rows"] = [{k: row[k] for k in ("case", "sample", "round", "version", "total_ms")}
                       for row in report["rows"]]
    compact["full_report_sha256"] = report.get("full_report_sha256", hashlib.sha256(args.report.read_bytes()).hexdigest())
    compact["plotting"] = dict(matplotlib=matplotlib.__version__, numpy=np.__version__,
                                quartile="linear interpolation (Hyndman-Fan type 7)",
                                whiskers="Most extreme observed points within Q1/Q3 +/- 1.5 IQR",
                                outliers="All observations outside fences; retained, not removed",
                                speed_sample="one whole key sequence", resource_sample="one complete process run")
    (output / "summary.json").write_text(json.dumps(compact, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    for name, rows, fields in (
        ("timings.csv", compact["rows"], ["case", "version", "round", "sample", "total_ms"]),
        ("resources.csv", [{k: v for k, v in row.items() if not isinstance(v, (dict, list))}
                           for row in report["process_resources"]], None),
    ):
        with (output / name).open("w", newline="", encoding="utf-8") as file:
            writer = csv.DictWriter(file, fieldnames=fields or list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
    stats = {"speed": {}, "resources": {}}
    stamp = f"{report['measured_at'][:10]} | Rust {report['rust_commit'][:7]} | C# v1.3.0 | n={report['samples_per_round'] * (len(report['order'])//2)} sequences / version / case"
    partitions = [
        (IMAGES[0], [r for r in report["comparisons"] if r["case"].endswith(("_32", "_64"))], "32 / 64 character inputs"),
        (IMAGES[1], [r for r in report["comparisons"] if r["case"].endswith(("_128", "_256"))], "128 / 256 character inputs"),
        (IMAGES[2], [r for r in report["comparisons"] if r["case"].startswith("short_") or r["case"] == "edit_and_katakana"], "Short inputs / editing"),
    ]
    selected = [row["case"] for _, cases, _ in partitions for row in cases]
    if len(selected) != len(set(selected)) or set(selected) != {r["case"] for r in report["comparisons"]}:
        raise ValueError("Chart partitions do not cover every case exactly once")
    for filename, cases, title in partitions:
        fig, axes = plt.subplots((len(cases) + 1) // 2, 2, figsize=(12, 3.5 * ((len(cases) + 1) // 2)), squeeze=False)
        for ax, case in zip(axes.flat, cases):
            groups = {v: [r["total_ms"] for r in report["rows"] if r["version"] == v and r["case"] == case["case"]]
                      for v in VERSIONS}
            suffix = "" if case["eligible_for_speed_claim"] else " [TRACE MISMATCH]"
            stats["speed"][case["case"]] = boxes(ax, groups, "Sequence time (ms, log scale)",
                f"{case['case']}{suffix}\n{case['keys']} keys incl. Enter", log=True)
        for ax in list(axes.flat)[len(cases):]:
            ax.set_visible(False)
        finish(fig, output / filename, f"Shared Core: {title}",
               "Lower is faster. Each dot = one sequence; whiskers = 1.5 x IQR; every outlier is shown.\n"
               "No TSF, candidate rendering, external converter, prediction or learning.\n" + stamp)
    mib = 1024 ** 2
    panels = [
        (IMAGES[3], "CPU / elapsed time across complete benchmark processes", [
            ("cpu_seconds", "CPU time", "seconds", lambda r: r["cpu_seconds"]),
            ("wall_seconds", "Elapsed time", "seconds", lambda r: r["wall_seconds"]),
            ("cpu_percent_one_core", "Mean CPU use\n(one core = 100%)", "%", lambda r: r["cpu_percent_one_core"]),
            ("cpu_percent_machine", "Mean CPU use\n(whole machine = 100%)", "%", lambda r: r["cpu_percent_machine"]),
        ]),
        (IMAGES[4], "Memory across complete benchmark processes", [
            ("peak_working_set_bytes", "Peak working set\n(resident; shared pages included)", "MiB", lambda r: r["peak_working_set_bytes"] / mib),
            ("sampled_mean_working_set_bytes", "Sampled mean working set", "MiB", lambda r: r["sampled_mean_working_set_bytes"] / mib),
            ("sampled_peak_private_bytes", "Sampled peak private bytes\n(committed)", "MiB", lambda r: r["sampled_peak_private_bytes"] / mib),
            ("peak_working_set_percent_physical", "Peak working set /\ntotal physical RAM", "%", lambda r: r["peak_working_set_percent_physical"]),
        ]),
    ]
    for filename, title, metrics in panels:
        fig, axes = plt.subplots(2, 2, figsize=(12, 9))
        for ax, (key, caption, unit, value) in zip(axes.flat, metrics):
            groups = {v: [value(r) for r in report["process_resources"] if r["version"] == v] for v in VERSIONS}
            stats["resources"][key] = boxes(ax, groups, unit, caption)
        finish(fig, output / filename, title,
               "Each dot = one process run; includes startup, dictionaries, sessions, warmups and JSON output.\n"
               "RAM polled at ~50 ms; short peaks may be missed. This is the test host, not a resident IME.\n" + stamp.split(" | n=")[0] + f" | n={len(report['process_resources'])//2} processes / version")
    (output / "boxplot-statistics.json").write_text(json.dumps(stats, indent=2) + "\n", encoding="utf-8")
    matching = [r for r in report["comparisons"] if r["eligible_for_speed_claim"]]
    faster = sum(r["rust_over_csharp"] < 1 for r in matching)
    rows = ["# Windows共通Coreの再測定（2026-10-21）", "",
            f"C# v1.3.0 NativeAOT と Rust `{report['rust_commit'][:7]}` Release。比較可能な入力は {len(matching)}/{len(report['comparisons'])}、Rustの中央値が短い入力は {faster}/{len(matching)}。",
            "", "## 条件と箱ひげ図の読み方", "",
            f"各入力・各版 {report['samples_per_round']} 試行 × {len(report['order'])//2} 巡、各巡 {report['warmups_per_case']} 回ウォームアップ。C#→Rust、Rust→C#を交互に実行。CPU・RAMは各版 {len(report["process_resources"])//2} プロセスを独立した観測値とする。",
            f"測定環境: {report['machine']} / {report.get('hardware', {}).get('Name',report['cpu'])} / {report['process_resources'][0]['logical_processors']}論理CPU / OS可視RAM {report['process_resources'][0]['system_memory_before']['total_bytes']/(1024**3):.2f} GiB。{report['rustc'].splitlines()[0]} / .NET {report['dotnet']}。", "",
            "四分位数は線形補間（type 7）。箱はQ1〜Q3、黒線は中央値、ひげはQ1−1.5×IQR〜Q3＋1.5×IQR内の最端の観測値。赤い菱形は外れ値。外れ値を除外せず、各パネルに中央値・n・外れ値数を表示する。青/橙の点は全観測値。速度図の縦軸は対数で、統計は元のms値から計算する。", "",
            "速度はCoreの打鍵＋JSON応答生成。セッション生成・辞書構築・起動は区間外。CPU・RAMはそれらとウォームアップ・JSON出力を含むプロセス全体。外部変換・OSスペル・ライブ変換・予測・学習・TSF・候補描画・実アプリ入力は対象外。出力/比較対象の打鍵状態の一致は全UI・全候補の同等性を意味しない。", "",
            "## 全入力の中央値", "", "|入力|C# (ms)|Rust (ms)|Rust/C#|比較対象の状態一致|", "|---|---:|---:|---:|:---:|"]
    rows += [f"|{r['case']}|{r['median_total_ms']['csharp']:.4f}|{r['median_total_ms']['rust']:.4f}|{r['rust_over_csharp']:.4f}|{'一致' if r['trace_matches'] else '不一致'}|" for r in report["comparisons"]]
    rows += ["", "## CPU・RAMのプロセス中央値", "", "|指標|C#|Rust|", "|---|---:|---:|"]
    for _, _, metrics in panels:
        for key, caption, unit, _ in metrics:
            values = stats["resources"][key]
            rows.append(f"|{caption.replace(chr(10), ' ')} ({unit})|{values['csharp']['med']:.4f}|{values['rust']['med']:.4f}|")
    rows += ["", "## 全画像", ""]
    rows += [f"![{name}]({name})\n" for name in IMAGES]
    rows += ["## 証跡・再現", "", "[全測定の合計時間CSV](timings.csv)／[CPU・RAMの全観測CSV](resources.csv)／[中央値・四分位・全外れ値の数値](boxplot-statistics.json)／[測定条件・ハッシュ・コンパイラ・全合計時間](summary.json)", "",
             "```powershell", "$env:CARGO_TARGET_DIR='C:/opencode-rust/meltype-target-1.94'", "$env:PYTHONUTF8='1'",
             "python tools/benchmark-windows.py --output dist/windows-benchmark-20261021-final --samples 3 --warmups 1 --rounds 10",
             "# matplotlib/numpyを導入したPythonで実行。公開summary.jsonからも画像を再作成できる。",
             "python tools/plot-windows-benchmark.py <report.jsonまたはsummary.json> --output <新しいフォルダー>", "```", "",
             f"ソースとバイナリの前後一致: sources_unchanged={report['sources_unchanged']}, binaries_unchanged={report['binaries_unchanged']}。全辞書のSHA-256一致を測定開始前に検査した。",
             "最初の試走は描画ライブラリ導入と重なったため途中停止し、集計対象に含めていない。本測定は準備完了後に最初から実施した。外部負荷やOSスケジューリングの影響を完全には除けず、今回の測定を普遍的な速度保証やIME全体の高速化率には使わない。", ""]
    (output / "README.md").write_text("\n".join(rows), encoding="utf-8")
    print(json.dumps({"output": str(output), "images": IMAGES, "matching_cases": len(matching), "faster_cases": faster}))


if __name__ == "__main__":
    main()
