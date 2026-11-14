#!/usr/bin/env python3
"""Versioned checklist coverage and freshly executed ngspice comparison reporting."""
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SPEC = Path('reports/parity-scope.json')
RESULT = Path('reports/parity.json')
MARKDOWN = Path('docs/PARITY.md')
HISTORY = Path('reports/parity-history.jsonl')
REPORT_TITLE = 'Core parity report'
REPORT_SCRIPT = 'parity.py'
SCOPE_LABEL = 'agreed core'
LOG_DIRECTORY = Path('reports/logs')
SUITES = {
    'core': ['-p', 'spice-core', '--lib'],
    'analytical': ['-p', 'spice-core', '--test', 'analytical'],
    'expressions': ['-p', 'spice-netlist', '--lib'],
    'parser': ['-p', 'spice-netlist', '--test', 'parser'],
    'expansion': ['-p', 'spice-netlist', '--test', 'expansion'],
    'cli': ['-p', 'spice-cli', '--test', 'batch'],
    'oracle': ['-p', 'spice-netlist', '--test', 'ngspice'],
}


def load_spec(root=ROOT):
    spec = json.loads((root / SPEC).read_text())
    if spec['schema_version'] != 1 or not spec['criteria']:
        raise ValueError('unsupported or empty scope')
    if spec['changelog'][-1]['version'] != spec['scope_version']:
        raise ValueError('scope version must have a change-log entry')
    ids = set()
    for item in spec['criteria']:
        if item['id'] in ids or type(item['implemented']) is not bool:
            raise ValueError('duplicate criterion ID or invalid implementation status')
        ids.add(item['id'])
        if not item['evidence'] or not (root / item['evidence']).is_file():
            raise ValueError(f"missing evidence file for {item['id']}")
        for check in item['checks']:
            suite, test = check.split('/', 1)
            if suite not in SUITES and suite not in ('scale','platform'):
                raise ValueError(f'unknown suite {suite}')
            if not test:
                raise ValueError('empty test name')
    return spec


def scope_signature(spec):
    definition = {key: spec[key] for key in ('reference', 'description', 'excluded')}
    definition['criteria'] = [{key: item[key] for key in ('id', 'category', 'title')}
                              for item in spec['criteria']]
    return hashlib.sha256(json.dumps(definition, sort_keys=True).encode()).hexdigest()


def require_scope_version(spec, previous):
    if (previous and previous['scope_version'] == spec['scope_version']
            and previous['scope_sha256'] != scope_signature(spec)):
        raise ValueError('scope definition changed without a scope-version bump')


def source_fingerprint(root=ROOT):
    """Hash tracked source inputs by content/path; no timestamps, build outputs or reports."""
    paths = {root / 'Cargo.toml', root / 'Cargo.lock', root / SPEC}
    for prefix in ('crates', 'examples', 'scripts', '.github/workflows'):
        paths.update(p for p in (root / prefix).rglob('*')
                     if p.is_file() and p.suffix in ('.rs', '.toml', '.cir', '.py', '.sh', '.yml', '.yaml'))
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(path.relative_to(root).as_posix().encode() + b'\0')
        digest.update(path.read_bytes().replace(b'\r\n',b'\n') + b'\0')
    return digest.hexdigest()


def percent(done, total):
    return round(100.0 * done / total, 2) if total else None


def calculate(spec, suites):
    groups = {}
    details = []
    for item in spec['criteria']:
        group = groups.setdefault(item['category'], {'total': 0, 'implemented': 0, 'evidenced': 0})
        checks = []
        for check in item['checks']:
            suite, test = check.split('/', 1)
            result = suites.get(suite, {})
            checks.append(result.get('status') == 'passed'
                          and result.get('tests', {}).get(test) == 'passed')
        evidenced = item['implemented'] and bool(checks) and all(checks)
        group['total'] += 1
        group['implemented'] += int(item['implemented'])
        group['evidenced'] += int(evidenced)
        details.append({**item, 'state': 'evidenced' if evidenced else
                        'implemented, evidence incomplete' if item['implemented'] else 'missing'})
    total = {key: sum(g[key] for g in groups.values()) for key in ('total', 'implemented', 'evidenced')}
    for group in [total, *groups.values()]:
        group['implementation_percent'] = percent(group['implemented'], group['total'])
        group['evidenced_percent'] = percent(group['evidenced'], group['total'])
    oracle = suites.get('oracle', {})
    results = list(oracle.get('tests', {}).values())
    passed = results.count('passed')
    failed = results.count('failed')
    oracle_summary = {'status': oracle.get('status', 'not_run'), 'passed': passed,
                      'failed': failed, 'discovered': len(results),
                      'pass_percent': percent(passed, passed + failed)
                      if (oracle.get('status') == 'passed' or (oracle.get('status') == 'failed' and failed)) and passed + failed else None}
    return {'total': total, 'categories': groups, 'criteria': details, 'oracle': oracle_summary}


def parse_tests(output):
    outcomes = {'ok': 'passed', 'FAILED': 'failed', 'ignored': 'ignored'}
    return {match[0]: outcomes[match[1]] for match in
            re.findall(r'^test (\S+) \.\.\. (ok|FAILED|ignored)\b', output, re.MULTILINE)}


def execute(root, name, command, expected=None):
    logs = root / LOG_DIRECTORY
    logs.mkdir(parents=True, exist_ok=True)
    try:
        run = subprocess.run(command, cwd=root, text=True, stdout=subprocess.PIPE,
                             stderr=subprocess.STDOUT, timeout=600, check=False)
        output = run.stdout
        tests = parse_tests(output)
        if expected is not None:
            tests = {expected: 'passed' if run.returncode == 0 and re.search(r'unknowns=10001\b', output) else 'failed'}
        status = 'passed' if run.returncode == 0 and tests and all(v == 'passed' for v in tests.values()) else 'failed'
        returncode = run.returncode
    except (OSError, subprocess.TimeoutExpired) as error:
        output, tests, status, returncode = str(error), {}, 'failed', None
    logfile = LOG_DIRECTORY / f'{name}.log'
    (root / logfile).write_text(output)
    print(f'{name}: {status} ({len(tests)} checks)', flush=True)
    return {'status': status, 'returncode': returncode, 'tests': tests,
            'log': logfile.as_posix(), 'log_sha256': hashlib.sha256(output.encode()).hexdigest()}


def run_suites(root, ngspice, oracle_filter=None):
    # Prefer the session-local toolchain only where installed; normal CI uses cargo.
    cargo = [str(root / 'scripts/cargo-local.sh')] if os.name != 'nt' else ['cargo']
    suites = {}
    for name, args in SUITES.items():
        if name == 'oracle':
            if not ngspice:
                suites[name] = {'status': 'not_run', 'tests': {}, 'reason': 'ngspice executable unavailable'}
                continue
            try:
                version = subprocess.run([ngspice, '--version'], text=True, capture_output=True, check=False, timeout=10)
            except (OSError, subprocess.TimeoutExpired) as error:
                suites[name] = {'status': 'failed', 'tests': {}, 'reason': str(error)}
                continue
            match = re.search(r'ngspice-(\d+)(?=\s|:|$)', version.stdout + version.stderr)
            if version.returncode or not match or match[1] != '47':
                suites[name] = {'status': 'failed', 'tests': {}, 'reason': 'oracle must identify as ngspice 47'}
                continue
            os.environ['NGSPICE_BIN'] = ngspice
        command = cargo + ['test', *args, '--locked', '--color', 'never', '--', '--test-threads=1']
        if name == 'oracle':
            command.append('--ignored')
            if oracle_filter:
                command.append(oracle_filter)
        suites[name] = execute(root, name, command)
        if name == 'oracle':
            suites[name]['reference_version'] = match[0]
            suites[name]['executable_sha256'] = hashlib.sha256(Path(ngspice).read_bytes()).hexdigest()
    suites['scale'] = execute(root, 'scale', cargo + ['run', '-p', 'spice-core', '--example',
                                'ladder', '--release', '--locked', '--', '10000'], 'ladder_10000')
    from platform_check import load_evidence
    suites['platform'] = load_evidence(root)
    return suites


def render(report):
    metrics = report['metrics']
    total = metrics['total']
    oracle = metrics['oracle']
    rate = 'not measured' if oracle['pass_percent'] is None else f"{oracle['pass_percent']:.2f}% ({oracle['passed']}/{oracle['passed'] + oracle['failed']} executed test groups)"
    lines = [f'# {REPORT_TITLE}', '',
             f"Scope: **{report['scope_version']}**, reference **ngspice 47**. Generated {report['generated_at']}.", '',
             f"**Evidence-backed checklist coverage: {total['evidenced_percent']:.2f}% ({total['evidenced']}/{total['total']}).**", '',
             f"Implementation checklist coverage: **{total['implementation_percent']:.2f}% ({total['implemented']}/{total['total']})**.", '',
             f"ngspice comparison pass rate: **{rate}**; oracle status: `{oracle['status']}`.", '',
             f'These percentages describe the {SCOPE_LABEL} checklist, not all of ngspice, numerical accuracy, engineering effort, or production readiness. '
             'Each criterion has one point. Missing, untested, skipped, failed, or unavailable criteria receive no evidence credit; partial implementations receive no implementation credit. '
             'A passing witness covers its named fixtures, not every circuit that might use a feature. Oracle counts are test groups, not individual circuits or sample points.', '',
             '| Category | Implemented | Evidence-backed | Evidence-backed coverage |',
             '| --- | ---: | ---: | ---: |']
    for name, group in metrics['categories'].items():
        lines.append(f"| {name} | {group['implemented']}/{group['total']} | {group['evidenced']}/{group['total']} | {group['evidenced_percent']:.2f}% |")
    lines += ['', '## Acceptance checklist', '', '| ID | Criterion | State | Evidence / witness tests |', '| --- | --- | --- | --- |']
    for item in metrics['criteria']:
        evidence = f"[{item['evidence']}](../{item['evidence']})"
        checks = ', '.join(f'`{check}`' for check in item['checks']) or 'No executable witness registered'
        lines.append(f"| {item['id']} | {item['title']} | {item['state']} | {evidence}; {checks} |")
    lines += ['', '## Refresh and history', '',
              f'Run `python3 scripts/{REPORT_SCRIPT} --run` to execute the registered suites and the 10,000-node benchmark, then regenerate Markdown and JSON. '
              'Set `NGSPICE_BIN` or use `--ngspice /absolute/path`; the local pinned reference is discovered automatically. '
              'Without ngspice, oracle coverage is unmeasured and its criteria lose evidence credit.', '',
              f'Run `python3 scripts/{REPORT_SCRIPT} --check` to detect stale source inputs, changed scope, or edited report totals. '
              'The check validates the recorded snapshot; it does not rerun tests. '
              'CI refreshes the results and uploads reports plus logs even when tests fail.', '',
              f'The denominator is versioned in [{SPEC.name}](../{SPEC.as_posix()}). '
              'Changing criteria requires a scope-version bump and an entry in its change log; compare history only within the same scope version. '
              f'[History](../{HISTORY.as_posix()}) records source/metric changes without duplicating identical reruns. '
              f'[Machine-readable snapshot](../{RESULT.as_posix()}) contains per-suite outcomes and source hashes.', '',
              f"Source SHA-256: `{report['source_sha256']}`.", '']
    return '\n'.join(lines)


def persist(report, root=ROOT):
    (root / RESULT).parent.mkdir(parents=True, exist_ok=True)
    (root / MARKDOWN).parent.mkdir(parents=True, exist_ok=True)
    (root / RESULT).write_text(json.dumps(report, indent=2) + '\n')
    (root / MARKDOWN).write_text(render(report))
    snapshot = {key: report[key] for key in ('generated_at', 'scope_version', 'source_sha256')}
    snapshot.update({key: report['metrics'][key] for key in ('total', 'categories', 'oracle')})
    history = root / HISTORY
    last = json.loads(history.read_text().splitlines()[-1]) if history.exists() and history.stat().st_size else None
    if last is None or any(snapshot[key] != last.get(key) for key in snapshot if key != 'generated_at'):
        with history.open('a') as stream:
            stream.write(json.dumps(snapshot, sort_keys=True) + '\n')


def check_report(root=ROOT):
    spec = load_spec(root)
    report = json.loads((root / RESULT).read_text())
    if (report['scope_version'] != spec['scope_version'] or report['scope_sha256'] != scope_signature(spec)
            or report['source_sha256'] != source_fingerprint(root)):
        raise ValueError(f'parity report is stale; run python3 scripts/{REPORT_SCRIPT} --run')
    if report['metrics'] != calculate(spec, report['suites']):
        raise ValueError('recorded metrics do not match the scope and test outcomes')
    if (root / MARKDOWN).read_text() != render(report):
        raise ValueError('Markdown report differs from the recorded snapshot')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument('--run', action='store_true', help='execute suites and update reports/history')
    mode.add_argument('--check', action='store_true', help='validate recorded report freshness without running tests')
    parser.add_argument('--ngspice', help='path to ngspice 47')
    args = parser.parse_args()
    try:
        if args.run:
            spec = load_spec()
            previous = json.loads((ROOT / RESULT).read_text()) if (ROOT / RESULT).exists() else None
            require_scope_version(spec, previous)
            candidate = args.ngspice or os.environ.get('NGSPICE_BIN')
            local = ROOT / 'reference/build/src/ngspice'
            ngspice = str(Path(candidate).resolve()) if candidate else str(local) if local.is_file() else None
            tested_source=source_fingerprint()
            suites = run_suites(ROOT, ngspice, oracle_filter='expansion_' if REPORT_SCRIPT == 'expansion.py' else None)
            if source_fingerprint()!=tested_source:
                raise ValueError('source changed during validation; rerun on a stable snapshot')
            report = {'schema_version': 1, 'scope_version': spec['scope_version'],
                      'scope_sha256': scope_signature(spec),
                      'generated_at': dt.datetime.now(dt.timezone.utc).isoformat(timespec='seconds'),
                      'source_sha256': source_fingerprint(), 'platform': sys.platform,
                      'suites': suites, 'metrics': calculate(spec, suites)}
            persist(report)
            failed = any(s['status'] == 'failed' for s in suites.values())
        else:
            report = check_report()
            failed = False
        totals = report['metrics']['total']
        print(f"{totals['evidenced_percent']:.2f}% evidence-backed; {totals['implementation_percent']:.2f}% implemented ({report['scope_version']}). See {MARKDOWN.as_posix()}.")
        return int(failed)
    except (ValueError, KeyError, OSError) as error:
        print(f'parity: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
