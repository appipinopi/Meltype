#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""Build the shared Rust engine and a load-unpacked ChromeOS extension ZIP."""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERSION = "0.2.129"
FILES = ("manifest.json", "background.mjs", "controller.mjs", "keys.mjs", "site-profiles.mjs", "options.html", "options.mjs", "options.css")


def run(*args):
    subprocess.run([str(a) for a in args], cwd=ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wasm-bindgen", default="wasm-bindgen")
    parser.add_argument("--out", type=Path, default=ROOT / "dist/chromeos-preview")
    args = parser.parse_args()
    tool = shutil.which(str(args.wasm_bindgen)) or str(Path(args.wasm_bindgen).resolve())
    version = subprocess.check_output([tool, "--version"], encoding="utf-8").strip()
    if version != f"wasm-bindgen {VERSION}":
        parser.error(f"wasm-bindgen {VERSION} is required; found {version}")
    out = args.out.resolve()
    if out == ROOT or ROOT.is_relative_to(out) or out.is_symlink():
        parser.error("output must be a dedicated generated directory")
    allowed = set(FILES) | {"pkg", "LICENSE", "THIRD-PARTY-NOTICES.md", "RUST-DEPENDENCY-NOTICES.txt", "README.md", "SHA256SUMS.json"}
    if out.exists() and any(p.name not in allowed for p in out.iterdir()):
        parser.error("output contains unrelated files; choose an empty directory")
    run("cargo", "build", "--release", "--locked", "--target", "wasm32-unknown-unknown", "-p", "meltype-browser")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT, encoding="utf-8"))
    wasm = Path(metadata["target_directory"]) / "wasm32-unknown-unknown/release/meltype_browser.wasm"
    run(tool, wasm, "--target", "web", "--out-dir", ROOT / "chromeos/pkg")
    run("node", "--test", *sorted((ROOT / "chromeos/tests").glob("*.test.mjs")))
    out.mkdir(parents=True, exist_ok=True)
    for name in FILES + ("README.md",):
        shutil.copyfile(ROOT / "chromeos" / name, out / name)
    for name in ("LICENSE", "THIRD-PARTY-NOTICES.md"):
        shutil.copyfile(ROOT / name, out / name)
    (out / "pkg").mkdir(exist_ok=True)
    for name in ("meltype_browser.js", "meltype_browser_bg.wasm"):
        shutil.copyfile(ROOT / "chromeos/pkg" / name, out / "pkg" / name)
    # Include the license texts from the actual resolved engine/build dependency
    # graph, rather than assuming desktop package notices cover new WASM crates.
    tree = subprocess.check_output(["cargo", "tree", "--locked", "-p", "meltype-browser", "--target", "wasm32-unknown-unknown", "--edges", "normal,build", "--prefix", "none", "--format", "{p}"], cwd=ROOT, encoding="utf-8")
    dependencies = {tuple(m.groups()) for line in tree.splitlines() if (m := re.match(r"^([\w-]+) v(\S+)", line))}
    graph = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version", "1", "--filter-platform", "wasm32-unknown-unknown"], cwd=ROOT, encoding="utf-8"))
    notices = ["Meltype ChromeOS Rust/WASM dependency license texts\n"]
    for package in sorted(graph["packages"], key=lambda p: (p["name"], p["version"])):
        if (package["name"], package["version"]) not in dependencies:
            continue
        notices.append(f'\n=== {package["name"]} {package["version"]} ({package.get("license")}) ===\n')
        folder = Path(package["manifest_path"]).parent
        license_files = [p for p in folder.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE"))]
        if package.get("license_file"):
            license_files.append(folder / package["license_file"])
        if not license_files and package["name"].startswith("meltype-"):
            license_files = [ROOT / "LICENSE"]
        if not license_files:
            raise RuntimeError(f'No license text found for {package["name"]}')
        for path in sorted(set(license_files)):
            notices.append(f'\n--- {path.name} ---\n{path.read_text(encoding="utf-8", errors="replace")}\n')
    (out / "RUST-DEPENDENCY-NOTICES.txt").write_text("".join(notices), encoding="utf-8")
    packaged = [*(out / p for p in FILES), out / "README.md", out / "LICENSE", out / "THIRD-PARTY-NOTICES.md",
                out / "pkg/meltype_browser.js", out / "pkg/meltype_browser_bg.wasm", out / "RUST-DEPENDENCY-NOTICES.txt"]
    checksums = {p.relative_to(out).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in packaged}
    (out / "SHA256SUMS.json").write_text(json.dumps(checksums, indent=2) + "\n", encoding="utf-8")
    with zipfile.ZipFile(out.with_suffix(".zip"), "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in packaged + [out / "SHA256SUMS.json"]:
            archive.write(path, path.relative_to(out).as_posix())
    print(json.dumps({"directory": str(out), "zip": str(out.with_suffix('.zip')), "files": len(checksums)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
