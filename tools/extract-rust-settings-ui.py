# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Yukishiro, appipinopi
"""Keep Rust settings labels, descriptions, categories and enum labels upstream-aligned."""
import argparse
import json
from pathlib import Path
import re
import subprocess

root = Path(__file__).resolve().parent.parent
baseline = "6310c44b728d5ad88284853f9ef37ea224993b18"
try:
    source = subprocess.check_output(
        ["git", "show", baseline + ":src/Meltype.Core/Config/Settings.cs"],
        cwd=root, stderr=subprocess.PIPE,
    ).decode("utf-8-sig")
except subprocess.CalledProcessError as error:
    raise SystemExit(f"Missing v1.3.0 reference; fetch it first: git fetch --no-tags origin {baseline}") from error

def string(raw):
    return json.loads('"' + raw + '"')

fields = []
pattern = r"public\s+([\w<>?\[\]]+)\s+(\w+)\s*\{\s*get;\s*set;"
previous = 0
for match in re.finditer(pattern, source):
    typ, key = match.groups()
    attrs = source[previous:match.start()]
    previous = match.end()
    if "Category(" not in attrs:
        continue
    attrs = attrs[attrs.rfind("[Category("):]
    def attr(name):
        found = re.search(name + r'\("((?:\\.|[^"\\])*)"\)', attrs)
        return string(found.group(1)) if found else ""
    typ = {"bool": "Boolean", "int": "Int32", "string": "String", "string[]": "String[]"}.get(typ, typ)
    if typ.startswith("List<"):
        typ = "List`1"
    fields.append(dict(key=key, type=typ, category=attr("Category"),
                       label=attr("DisplayName"), description=attr("Description")))
enums = {}
for match in re.finditer(r"public enum (\w+)\s*\{(.*?)\n\}", source, re.S):
    enums[match[1]] = {name: string(label) for label, name in re.findall(
        r'\[Description\("((?:\\.|[^"\\])*)"\)\]\s*(\w+)', match[2])}
assert fields and len({f["key"] for f in fields}) == len(fields)
assert all(f["label"] and f["category"] for f in fields)
data = dict(upstream=baseline, fields=fields, enums=enums)
text = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
path = root / "rust/settings-app/settings-ui.json"
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
if args.check:
    if path.read_text(encoding="utf-8") != text:
        raise SystemExit("Settings UI metadata differs from upstream; run this script to regenerate.")
else:
    path.write_text(text, encoding="utf-8")
print(f"{len(fields)} settings fields and {len(enums)} enum label tables verified")
