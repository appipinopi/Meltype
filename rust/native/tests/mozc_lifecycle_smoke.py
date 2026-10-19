# SPDX-License-Identifier: GPL-3.0-or-later
"""Original MozcConverter and Rust transport with the same controlled helper."""
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

helper, reference, rust = (Path(x).resolve() for x in sys.argv[1:4])
cases = Path(__file__).resolve().parents[3]/'tools/parity/mozc-lifecycle/cases.json'
with tempfile.TemporaryDirectory(prefix='meltype-mozc-lifecycle-') as directory:
    root=Path(directory)
    for mode in ['normal','reject-save','ignore-quit']:
        outputs=[]
        for name,command in [('original',['dotnet',str(reference)]),('rust',[str(rust)])]:
            profile=root/(mode+'-'+name);profile.mkdir()
            if mode!='normal':(profile/('fixture-'+mode)).touch()
            start=time.monotonic()
            result=subprocess.run(command+[str(helper),str(profile),str(cases)],capture_output=True,text=True,encoding='utf-8',timeout=20)
            assert result.returncode==0,(mode,name,result.returncode,result.stderr)
            elapsed=time.monotonic()-start
            value=json.loads(result.stdout)
            trace=value['trace']
            assert len([s for s in trace if s.startswith('L\t')])==12,trace
            assert trace.count('S')==2,trace
            assert trace[-1]=='Q',trace
            saved=(profile/'fixture-saved-on-quit').exists()
            assert saved==(mode!='ignore-quit'),(mode,name,saved)
            if mode=='ignore-quit':assert elapsed<5,(name,elapsed)
            outputs.append(value)
        assert outputs[0]==outputs[1],(mode,outputs)
        print(f'Mozc lifecycle {mode}: original/Rust L encoding, empty/NG handling, save every 5 successes and Q matched')
