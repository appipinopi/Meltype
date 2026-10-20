# SPDX-License-Identifier: GPL-3.0-or-later
"""Replay frozen C# candidate, annotation and setting interactions through native ABI."""
import argparse,json,tempfile,sys
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
from check import native,projection
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('dll',type=Path)
parser.add_argument('--fixture',type=Path,default=Path('rust/core/tests/data/upstream-settings-candidates.json'))
args=parser.parse_args();rows=json.loads(args.fixture.read_text(encoding='utf-8'))['cases']
with tempfile.TemporaryDirectory(prefix='meltype-candidates-') as root:
    actual=native(args.dll,rows,Path(root))
    for case,reply in zip(rows,actual,strict=True):
        assert case['name']==reply['name']
        for index,(expected,result) in enumerate(zip(case['expected'],reply['expected'],strict=True)):
            got=projection(result)
            assert expected==got,(case['name'],index,expected,got)
print('C# settings/candidate parity:',len(rows),'scenarios passed')
