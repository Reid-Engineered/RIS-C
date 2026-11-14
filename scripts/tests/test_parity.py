"""Accounting regressions: missing/skipped/failed evidence must never earn parity credit."""
import copy
import importlib.util
import json
import hashlib
import sys
from unittest.mock import patch
from pathlib import Path
import tempfile
import unittest

MODULE = importlib.util.spec_from_file_location('parity', Path(__file__).parents[1] / 'parity.py')
parity = importlib.util.module_from_spec(MODULE)
MODULE.loader.exec_module(parity)


def scope():
    return {'schema_version': 1, 'scope_version': 'test-v1', 'reference': 'ngspice 47',
            'description': 'test', 'excluded': [], 'changelog': [{'version': 'test-v1'}],
            'criteria': [
                {'id': 'one', 'category': 'core', 'title': 'implemented', 'implemented': True,
                 'evidence': 'crates/test.rs', 'checks': ['core/witness']},
                {'id': 'two', 'category': 'core', 'title': 'missing', 'implemented': False,
                 'evidence': 'crates/test.rs', 'checks': []}]}


class ParityTests(unittest.TestCase):
    def test_profile_logs_preserve_independent_evidence_digests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            command = [sys.executable, '-c', "print('test witness ... ok')"]
            core = parity.execute(root, 'oracle', command)
            with patch.object(parity, 'LOG_DIRECTORY', Path('reports/logs/expansion')):
                expansion = parity.execute(root, 'oracle', command)
            self.assertNotEqual(core['log'], expansion['log'])
            for record in [core, expansion]:
                self.assertEqual(record['status'], 'passed')
                self.assertEqual(record['log_sha256'], hashlib.sha256((root/record['log']).read_bytes()).hexdigest())

    def test_missing_items_remain_in_denominator(self):
        metrics = parity.calculate(scope(), {'core': {'status': 'passed', 'tests': {'witness': 'passed'}}})
        self.assertEqual(metrics['total']['implementation_percent'], 50.0)
        self.assertEqual(metrics['total']['evidenced_percent'], 50.0)
        self.assertIsNone(metrics['oracle']['pass_percent'])

    def test_unavailable_failed_ignored_or_unknown_tests_do_not_earn_credit(self):
        for status, tests in [('not_run', {}), ('failed', {'witness': 'passed'}),
                              ('passed', {'witness': 'ignored'}), ('passed', {'other': 'passed'})]:
            metrics = parity.calculate(scope(), {'core': {'status': status, 'tests': tests}})
            self.assertEqual(metrics['total']['evidenced_percent'], 0.0)

    def test_implementation_without_witness_is_not_evidence(self):
        spec = scope()
        spec['criteria'][0]['checks'] = []
        self.assertEqual(parity.calculate(spec, {})['total']['evidenced_percent'], 0.0)

    def test_oracle_pass_rate_is_not_feature_coverage(self):
        metrics = parity.calculate(scope(), {'oracle': {'status': 'passed', 'tests': {'fixture': 'passed'}}})
        self.assertEqual(metrics['oracle']['pass_percent'], 100.0)
        self.assertEqual(metrics['total']['evidenced_percent'], 0.0)
        broken = parity.calculate(scope(), {'oracle': {'status': 'failed', 'tests': {}}})
        self.assertIsNone(broken['oracle']['pass_percent'])

    def test_parse_actual_outcomes_including_ignored(self):
        text = 'test a ... ok\ntest b ... FAILED\ntest c ... ignored, external reference\n'
        self.assertEqual(parity.parse_tests(text), {'a': 'passed', 'b': 'failed', 'c': 'ignored'})

    def test_changed_denominator_requires_scope_version_bump(self):
        spec = scope()
        previous = {'scope_version': spec['scope_version'], 'scope_sha256': parity.scope_signature(spec)}
        spec['criteria'].append(copy.deepcopy(spec['criteria'][0]))
        spec['criteria'][-1]['id'] = 'three'
        with self.assertRaisesRegex(ValueError, 'version'):
            parity.require_scope_version(spec, previous)
        spec['scope_version'] = 'test-v2'
        parity.require_scope_version(spec, previous)

    def test_progress_can_change_without_redefining_scope(self):
        spec = scope()
        previous = {'scope_version': spec['scope_version'], 'scope_sha256': parity.scope_signature(spec)}
        spec['criteria'][1]['implemented'] = True
        spec['criteria'][1]['checks'] = ['core/new_witness']
        parity.require_scope_version(spec, previous)

    def test_freshness_integrity_and_history(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for folder in ['reports', 'docs', 'crates']:
                (root / folder).mkdir()
            (root / 'Cargo.toml').write_text('manifest')
            (root / 'Cargo.lock').write_text('lockfile')
            (root / 'crates/test.rs').write_text('source')
            spec = scope()
            (root / parity.SPEC).write_text(json.dumps(spec))
            report = {'scope_version': spec['scope_version'], 'generated_at': 'test',
                      'scope_sha256': parity.scope_signature(spec),
                      'source_sha256': parity.source_fingerprint(root), 'suites': {},
                      'metrics': parity.calculate(spec, {})}
            parity.persist(report, root)
            parity.check_report(root)
            parity.persist({**report, 'generated_at': 'later'}, root)
            self.assertEqual(len((root / parity.HISTORY).read_text().splitlines()), 1)
            (root / parity.MARKDOWN).write_text('misleading manual edit')
            with self.assertRaisesRegex(ValueError, 'Markdown'):
                parity.check_report(root)
            parity.persist(report, root)
            (root / 'crates/test.rs').write_text('changed source')
            with self.assertRaisesRegex(ValueError, 'stale'):
                parity.check_report(root)

class PlatformEvidenceTests(unittest.TestCase):
    def test_native_evidence_requires_all_platforms_and_current_source(self):
        module=importlib.util.spec_from_file_location('platform_check',Path(__file__).parents[1]/'platform_check.py')
        native=importlib.util.module_from_spec(module);module.loader.exec_module(native)
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            for path in ['Cargo.toml','Cargo.lock','scripts/platform_check.py','.github/workflows/ci.yml']:
                p=root/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text('fixture\n')
            (root/'reports').mkdir()
            fingerprint=native.numerical_fingerprint(root)
            record={'numerical_sha256':fingerprint,'jobs':[{'system':'Linux','numerical_sha256':fingerprint,'conclusion':'success','returncode':0,'passed_tests':32,'job_url':'https://github.com/Reid-Engineered/RIS-C/actions/runs/1/job/1'}]}
            path=root/'reports/platform-evidence.json';path.write_text(json.dumps(record))
            self.assertEqual(native.load_evidence(root)['status'],'not_run')
            for system in ['Darwin','Windows']:record['jobs'].append({**record['jobs'][0],'system':system})
            path.write_text(json.dumps(record));self.assertEqual(native.load_evidence(root)['status'],'passed')
            (root/'Cargo.toml').write_text('changed\n');self.assertEqual(native.load_evidence(root)['status'],'not_run')


if __name__ == '__main__':
    unittest.main()
