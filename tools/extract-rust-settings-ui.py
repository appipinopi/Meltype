# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Yukishiro, appipinopi
"""Keep Rust settings labels, descriptions, categories and enum labels upstream-aligned."""
import argparse
import json
from pathlib import Path
import re

root = Path(__file__).resolve().parent.parent
source = (root / "src/Meltype.Core/Config/Settings.cs").read_text(encoding="utf-8-sig")

def string(raw):
    return json.loads('"' + raw + '"')

fields = []
pattern = r"((?:\s*\[[^\]]*\]\s*)+)public\s+([\w<>?]+)\s+(\w+)\s*\{\s*get;\s*set;"
for match in re.finditer(pattern, source):
    attrs, typ, key = match.groups()
    if "Category(" not in attrs:
        continue
    def attr(name):
        found = re.search(name + r'\("((?:\\.|[^"\\])*)"\)', attrs)
        return string(found.group(1)) if found else ""
    fields.append(dict(key=key, type=typ, category=attr("Category"),
                       label=attr("DisplayName"), description=attr("Description")))
enums = {}
for match in re.finditer(r"public enum (\w+)\s*\{(.*?)\n\}", source, re.S):
    enums[match[1]] = {name: string(label) for label, name in re.findall(
        r'\[Description\("((?:\\.|[^"\\])*)"\)\]\s*(\w+)', match[2])}
assert fields and len({f["key"] for f in fields}) == len(fields)
assert all(f["label"] and f["category"] for f in fields)
data = dict(upstream="45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46", fields=fields, enums=enums)
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
