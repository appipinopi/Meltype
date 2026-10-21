# SPDX-License-Identifier: GPL-3.0-or-later
"""Ensure checkpointing cannot turn failures or changed evidence into a pass."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('batch_compare', Path(__file__).with_name('batch_compare.py'))
batch_compare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(batch_compare)
summary_spec = importlib.util.spec_from_file_location('summarize_shards', Path(__file__).with_name('summarize_shards.py'))
summarize_shards = importlib.util.module_from_spec(summary_spec)
summary_spec.loader.exec_module(summarize_shards)


class BatchEvidence(unittest.TestCase):
    def run_case(self, mode='success', reuse=False):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        cases = root / 'input.json'
        cases.write_text(json.dumps([dict(name='one', text='はし'), dict(name='two', text='き')]), encoding='utf-8')
        dll = root / 'library.dll'
        dll.write_bytes(b'test-only')
        output = root / 'out'
        argv = ['batch_compare.py', 'windows', '--cases', str(cases), '--csharp', str(dll), '--dll', str(dll), '--output', str(output), '--batch-size', '1']
        if reuse:
            argv.append('--reuse-sta')
        calls = []

        def subprocess(argv, **kwargs):
            calls.append(argv)
            result = type('Result', (), dict(returncode=0))()
            if argv[0] == 'dotnet':
                kwargs['stdout'].write('{}\n')
            elif mode == 'crash':
                result.returncode = 2
            else:
                matched = int(mode != 'mismatch')
                report = Path(argv[argv.index('--report')+1])
                report.write_text(json.dumps(dict(checked=1, matched=matched, available=1, timeout_retries=0)), encoding='utf-8')
                result.returncode = int(not matched)
            return result

        def invoke():
            with patch.object(sys, 'argv', argv), patch.object(batch_compare.subprocess, 'run', subprocess), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises(SystemExit) as exit:
                    batch_compare.main()
            return exit.exception.code

        return root, output, calls, invoke

    def test_success_and_resume_without_new_requests(self):
        _, output, calls, invoke = self.run_case()
        self.assertEqual(invoke(), 0)
        self.assertTrue(json.loads((output/'summary.json').read_text())['passed'])
        count = len(calls)
        self.assertEqual(invoke(), 0)
        self.assertEqual(len(calls), count)

    def test_mismatch_is_preserved_on_resume(self):
        _, output, calls, invoke = self.run_case('mismatch')
        self.assertEqual(invoke(), 1)
        count = len(calls)
        self.assertEqual(invoke(), 1)
        self.assertEqual(len(calls), count)
        self.assertEqual(json.loads((output/'summary.json').read_text())['architectures']['x64']['mismatched'], 2)

    def test_shared_lifecycle_is_explicit_in_both_adapters_and_evidence(self):
        _, output, calls, invoke = self.run_case(reuse=True)
        self.assertEqual(invoke(), 0)
        for argv in calls:
            self.assertIn('--reuse' if argv[0] == 'dotnet' else '--batch-probe', argv)
        self.assertTrue(json.loads((output/'summary.json').read_text())['identity']['reuse_sta'])

    def test_process_error_is_not_a_completed_comparison(self):
        _, output, _, invoke = self.run_case('crash')
        self.assertEqual(invoke(), 1)
        summary = json.loads((output/'summary.json').read_text())
        self.assertFalse(summary['passed'])
        self.assertFalse(summary['architectures']['x64']['complete'])

    def test_changed_binary_refuses_resume(self):
        root, _, _, invoke = self.run_case()
        self.assertEqual(invoke(), 0)
        (root/'library.dll').write_bytes(b'changed')
        self.assertIn('Resume refused', invoke())

    def test_changed_report_refuses_resume(self):
        _, output, _, invoke = self.run_case()
        self.assertEqual(invoke(), 0)
        (output/'batch-000000/x64.json').write_text('{}', encoding='utf-8')
        self.assertIn('saved evidence changed', invoke())


class PartitionEvidence(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.rows = [dict(name='one', text='はし'), dict(name='two', text='き')]
        self.source = self.root/'source.json'
        self.source.write_text(json.dumps(self.rows), encoding='utf-8')
        for index, row in enumerate(self.rows):
            (self.root/f'cases-{index}.json').write_text(json.dumps([row]), encoding='utf-8')

    def test_generated_partition_alone_is_not_verification_success(self):
        result = summarize_shards.summarize(self.source, self.root, 2)
        self.assertTrue(result['partition_valid'])
        self.assertFalse(result['passed'])

    def test_overlapping_partition_is_rejected(self):
        (self.root/'cases-1.json').write_text(json.dumps([self.rows[0]]), encoding='utf-8')
        with self.assertRaisesRegex(AssertionError, 'overlapping'):
            summarize_shards.summarize(self.source, self.root, 2)

    def test_missing_partition_input_is_rejected(self):
        (self.root/'cases-1.json').write_text('[]', encoding='utf-8')
        with self.assertRaisesRegex(AssertionError, 'omitted'):
            summarize_shards.summarize(self.source, self.root, 2)

    def test_same_name_with_changed_input_is_rejected(self):
        (self.root/'cases-1.json').write_text(json.dumps([dict(name='two', text='different')]), encoding='utf-8')
        with self.assertRaisesRegex(AssertionError, 'differs from source'):
            summarize_shards.summarize(self.source, self.root, 2)


if __name__ == '__main__':
    unittest.main()
