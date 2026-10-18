#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
set -euo pipefail
[[ "$(uname -s)" == Darwin ]] || { echo 'macOS SDK required' >&2; exit 1; }
root="$(cd "$(dirname "$0")/../../.." && pwd)"
helper="$1"
temporary="$(mktemp -d "${TMPDIR:-/tmp}/meltype-register-helper.XXXXXX")"
trap 'rm -rf "$temporary"' EXIT
# The original tool is a test reference; never give it an existing path.
swiftc "$root/mac/Sources/RegisterInputSource/main.swift" -framework Carbon -o "$temporary/LegacyRegister"
python3 - "$helper" "$temporary" <<'PY'
import pathlib,subprocess,sys
helper=pathlib.Path(sys.argv[1]).resolve()
root=pathlib.Path(sys.argv[2]).resolve()
legacy=root/'LegacyRegister'
paths=[str(root/'不存在 辞書 👩‍💻.app'),'missing relative.app','', '\ufeffmissing.app']
for path in paths:
    assert not pathlib.Path(path or '__empty_path__').is_absolute() or not pathlib.Path(path).exists()
    outputs=[]
    for executable in [legacy,helper]:
        result=subprocess.run([str(executable),path,'ignored extra argument'],cwd=root,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
        outputs.append((result.returncode,result.stdout,result.stderr))
    assert outputs[0]==outputs[1],(path,outputs)
    assert outputs[1]==(1,b'',('Meltype.app が見つかりません: '+path+'\n').encode()),(path,outputs)
print('Rust registration helper: original Swift CLI status/stdout/stderr parity for 4 missing absolute/relative/empty/BOM/Unicode path cases passed (Carbon and agent restart NOT_RUN)')
linked=subprocess.check_output(['/usr/bin/otool','-L',str(helper)],text=True)
assert 'libswift' not in linked.lower(),linked
assert 'libMeltypeNative' not in linked and 'llama' not in linked,linked
print('Rust registration helper: standalone Foundation/Carbon executable, no direct Swift/native-core/azooKey dylib dependency')
PY
