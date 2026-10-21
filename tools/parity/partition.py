# SPDX-License-Identifier: GPL-3.0-or-later
"""Partition a finite corpus without overlap, retaining scene groups together."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--shards', type=int, default=4)
parser.add_argument('--group-size', type=int, default=1)
args = parser.parse_args()
assert args.shards > 0 and args.group_size > 0
rows = json.loads(args.source.read_text(encoding='utf-8'))
assert rows and len({row['name'] for row in rows}) == len(rows)
assert len(rows) % args.group_size == 0, 'source must contain complete scene groups'
parts = [[] for _ in range(args.shards)]
for offset in range(0, len(rows), args.group_size):
    parts[(offset//args.group_size) % args.shards].extend(rows[offset:offset+args.group_size])
args.output.mkdir(parents=True, exist_ok=False)
hashes = {}
for index, part in enumerate(parts):
    path = args.output/f'cases-{index}.json'
    path.write_text(json.dumps(part, ensure_ascii=False, indent=2), encoding='utf-8')
    hashes[path.name] = hashlib.sha256(path.read_bytes()).hexdigest()
report = dict(source_sha256=hashlib.sha256(args.source.read_bytes()).hexdigest(), requests=len(rows),
              shards=list(map(len, parts)), group_size=args.group_size, sha256=hashes,
              coverage='Each source input appears exactly once. Generation does not mean comparison passed.')
(args.output/'partition.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print('Partitioned', len(rows), 'inputs:', list(map(len, parts)))
