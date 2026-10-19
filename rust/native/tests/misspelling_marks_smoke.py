# SPDX-License-Identifier: GPL-3.0-or-later
"""Original misspelling suggestions through UTF-8 scalar key input and C ABI."""
import ctypes as c
import json
import os
from pathlib import Path
import sys
import tempfile


def main(library):
    api = c.CDLL(library)
    word_type = c.CFUNCTYPE(c.c_int, c.c_char_p)
    @word_type
    def no_os_word(_word):
        return 0
    api.meltype_init.argtypes = [c.c_void_p, c.c_void_p, word_type]
    api.meltype_init.restype = c.c_int
    api.meltype_create_at.argtypes = [c.c_char_p]
    api.meltype_create_at.restype = c.c_void_p
    api.meltype_destroy.argtypes = [c.c_void_p]
    api.meltype_free.argtypes = [c.c_void_p]
    api.meltype_free.restype = None
    api.meltype_destroy.restype = None
    api.meltype_control.argtypes = [c.c_void_p, c.c_char_p]
    api.meltype_control.restype = c.c_void_p
    api.meltype_reset.argtypes = [c.c_void_p]
    api.meltype_reset.restype = c.c_void_p
    api.meltype_handle_key.argtypes = [c.c_void_p, c.c_int, c.c_int, c.c_int, c.c_char_p, c.c_char_p]
    api.meltype_handle_key.restype = c.c_void_p
    def result(pointer):
        assert pointer, 'null ABI response'
        try:
            return json.loads(c.string_at(pointer).decode('utf-8'))
        finally:
            api.meltype_free(pointer)
    golden = json.loads((Path(__file__).resolve().parents[2]/'core/tests/data/upstream-misspellings.json').read_text(encoding='utf-8'))
    expected = {row['reading']: row for row in golden['cases']}
    ui = json.loads((Path(__file__).resolve().parents[2]/'core/tests/data/upstream-misspelling-keys.json').read_text(encoding='utf-8'))
    suggestions = {row['case']['unicodeReading']:row['expected'][-2]['view']['suggestion'] for row in ui['cases'] if row['case']['name'].startswith('unicode-')}
    readings = ['シュミレーション', '\u0903ブレスレット', 'ブレ\u0903スレット', 'ブレ\u20ddスレット',
                'ブレ\ufe0fスレット', 'ブレ\u034fスレット', 'ブレ\u302eスレット', 'ブレ\u302fスレット']
    assert api.meltype_init(None, None, no_os_word) == 1
    with tempfile.TemporaryDirectory(prefix='meltype-marks-') as directory:
        handle = api.meltype_create_at(directory.encode('utf-8'))
        assert handle
        try:
            for name, value in {'LiveConversion': False, 'PredictiveCandidates': False, 'LearningEnabled': False,
                                'CorrectTypos': True, 'DetectionLevel': 'Balanced', 'AutoCorrectAfterCommit': False}.items():
                assert result(api.meltype_control(handle, json.dumps(dict(op='settings_set', name=name, value=value)).encode()))['saved']
            for reading in readings:
                assert expected[reading]['error'] is None
                result(api.meltype_reset(handle))
                # Start through an ordinary key as the original public
                # Session does; Unicode scalars then extend that composition.
                result(api.meltype_handle_key(handle, 65, ord('a'), 0, None, None))
                for scalar in reading:
                    view = result(api.meltype_handle_key(handle, 0, ord(scalar), 0, None, None))['view']
                assert view is not None and not view['converting']
                assert view['suggestion'] == suggestions[reading], (reading, view, suggestions[reading])
        finally:
            api.meltype_destroy(handle)
    print('Native ABI: 8 original Mn/Mc/Me/CGJ/variation-selector/tone-mark suggestion cases passed')


if __name__ == '__main__':
    filename = 'meltype_native.dll' if os.name == 'nt' else ('libmeltype_native.dylib' if sys.platform == 'darwin' else 'libmeltype_native.so')
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.abspath(os.path.join('target', 'release', filename)))
