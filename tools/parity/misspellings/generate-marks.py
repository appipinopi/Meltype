# SPDX-License-Identifier: GPL-3.0-or-later
"""Generate/check the original UTF-16 Mn lookup from independent C# captures."""
import argparse
import base64
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[3]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--categories', type=Path, required=True)
parser.add_argument('--shapes', type=Path, required=True)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
category = next(json.loads(line) for line in args.categories.read_text(encoding='utf-8-sig').splitlines() if line.startswith('{'))
shapes = [json.loads(line) for line in args.shapes.read_text(encoding='utf-8-sig').splitlines() if line.startswith('{')]
bitmap = base64.b64decode(category['bitmap'], validate=True)
assert len(bitmap) == 8192 and len(shapes) == 112
ranges = []
start = None
for value in range(65537):
    marked = value < 65536 and bool(bitmap[value // 8] & (1 << (value % 8)))
    if marked and start is None:
        start = value
    elif not marked and start is not None:
        ranges.append([start, value - 1])
        start = None
assert ranges == category['ranges'] and len(ranges) == 212
text = '''// SPDX-License-Identifier: GPL-3.0-or-later
//! Generated from the original UTF-16 char category operation.
//! Source runtime and bitmap: tests/data/upstream-unicode-marks.json.
//! Regenerate with tools/parity/misspellings/generate-marks.py. Non-BMP scalars
//! are two surrogate chars in the original foreach and are not removed.
const RANGES: &[(u32, u32)] = &[
'''
text += ''.join(f'    (0x{a:04x}, 0x{b:04x}),\n' for a, b in ranges)
text += '''];
pub(super) fn is_non_spacing_mark(c: char) -> bool {
    let value = c as u32;
    let index = RANGES.partition_point(|&(_, end)| end < value);
    RANGES.get(index).is_some_and(|&(start, _)| start <= value)
}
'''
fixture = root / 'rust/core/tests/data/upstream-unicode-marks.json'
table = root / 'rust/core/src/linguistics/normalization_marks.rs'
if args.check:
    golden = json.loads(fixture.read_text(encoding='utf-8'))
    assert golden['categories']['ranges'] == ranges
    assert golden['categories']['bitmap'] == category['bitmap']
    assert golden['shapes'] == shapes
    assert table.read_text(encoding='utf-8') == text
else:
    sources = ['tools/parity/misspellings/Program.cs', 'tools/parity/misspellings/shape-cases.json',
               'tools/parity/misspellings/generate-marks.py', 'src/Meltype.Core/Composition/MisspellingDictionary.cs']
    golden = dict(baseline='45e14f86558aa6ea34f2c2ecd038ba95ca8c3c46',
                  sha256={p: hashlib.sha256((root/p).read_bytes()).hexdigest() for p in sources},
                  categories=category, shapes=shapes)
    fixture.write_text(json.dumps(golden, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
    table.write_text(text, encoding='utf-8')
print('Original UTF-16 Mn: 65,536 categories, 212 ranges and 112 shapes verified')
