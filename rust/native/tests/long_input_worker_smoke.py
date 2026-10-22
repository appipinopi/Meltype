# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 appipinopi
"""Burst/long composition through the real worker, with the TSF 800ms budget.

Usage: python long_input_worker_smoke.py path/to/meltype-tip-worker.exe
Uses only a temporary profile and never registers or activates an IME.
"""
import json
import os
from pathlib import Path
import queue
import struct
import subprocess
import sys
import tempfile
import threading
import time


def exercise(worker, live):
    with tempfile.TemporaryDirectory(prefix='flash-long-input-') as directory:
        profile = Path(directory)
        (profile / 'config.json').write_text(json.dumps(dict(
            Enabled=True, ConversionEngine='System', LiveConversion=live,
            PredictiveCandidates=False, LearningEnabled=False,
            DetectionLevel='Manual', SeparateKeyRoles=True,
        )), encoding='utf-8')
        env = dict(os.environ, MELTYPE_FLASH_DATA_DIR=directory)
        env.pop('MELTYPE_TEST_TIP_NO_CONVERTERS', None)
        process = subprocess.Popen([str(worker), '--stdio-tip-worker'],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)
        replies = queue.Queue()

        def read():
            try:
                while header := process.stdout.read(4):
                    size = struct.unpack('<I', header)[0]
                    assert 0 < size <= 1048576
                    replies.put(json.loads(process.stdout.read(size)))
            except Exception as error:
                replies.put(error)
            finally:
                replies.put(EOFError('worker ended'))

        reader = threading.Thread(target=read, daemon=True)
        reader.start()

        def call(op, **fields):
            request = json.dumps(dict(op=op, sid='burst', process='test.exe', **fields)).encode()
            start = time.monotonic()
            process.stdin.write(struct.pack('<I', len(request)) + request)
            process.stdin.flush()
            value = replies.get(timeout=10)
            if isinstance(value, Exception):
                raise value
            elapsed = time.monotonic() - start
            assert elapsed < .8, (live, op, 'TSF would terminate this worker', elapsed)
            return value, elapsed

        try:
            assert call('hello')[0]['active']
            worst = 0
            # No sleeps: pending letters, complete kana and many MSIME chunks.
            raw = 'nihongonyuuryokuwonagakutsuzukemasu' * 10
            for index, ch in enumerate(raw):
                value, elapsed = call('key', vk=ord(ch.upper()), ch=ord(ch))
                worst = max(worst, elapsed)
                assert value['consumed'] and not value['commits'] and value['view'], (index, value)
            converted, elapsed = call('key', vk=32, ch=32)
            worst = max(worst, elapsed)
            assert converted['view']['converting'] and not converted['commits'], converted
            text = converted['view']['text']
            committed, _ = call('key', vk=13)
            assert ''.join(c['text'] for c in committed['commits']) == text
            # A subsequent burst must use the same healthy connection.
            for ch in 'konnnichiha':
                assert call('key', vk=ord(ch.upper()), ch=ord(ch))[0]['consumed']
            assert call('key', vk=13)[0]['commits']
            print(json.dumps(dict(live=live, keys=len(raw), worst_ms=round(worst * 1000))), flush=True)
        finally:
            process.stdin.close()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
            reader.join(timeout=5)
        assert process.returncode == 0


worker = Path(sys.argv[1]).resolve()
for live in (True, False):
    exercise(worker, live)
