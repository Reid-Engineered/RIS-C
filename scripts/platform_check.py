#!/usr/bin/env python3
"""Record native Rust acceptance evidence; do not substitute cross-compilation for execution."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[1]

def numerical_fingerprint(root=ROOT):
    files={root/'Cargo.toml',root/'Cargo.lock',root/'scripts/platform_check.py',root/'.github/workflows/ci.yml'}
    for directory in ['crates','examples']:
        files.update(p for p in(root/directory).rglob('*') if p.is_file() and p.suffix in ['.rs','.toml','.cir'])
    digest=hashlib.sha256()
    for p in sorted(files):
        digest.update(p.relative_to(root).as_posix().encode()+b'\0')
        digest.update(p.read_bytes().replace(b'\r\n',b'\n')+b'\0')
    return digest.hexdigest()

def load_evidence(root=ROOT):
    path=root/'reports/platform-evidence.json'
    if not path.exists():return {'status':'not_run','tests':{},'reason':'native CI evidence not imported'}
    data=json.loads(path.read_text())
    if data['numerical_sha256']!=numerical_fingerprint(root):return {'status':'not_run','tests':{},'reason':'native evidence is stale for this numerical source'}
    checks={}
    for name in ['Linux','Darwin','Windows']:
        records=[r for r in data['jobs'] if r['system']==name and r['numerical_sha256']==data['numerical_sha256']]
        checks[name]='passed' if any(r['conclusion']=='success' and r['passed_tests']>0 and r['returncode']==0 and r['job_url'].startswith('https://github.com/Reid-Engineered/RIS-C/actions/runs/') for r in records) else 'not_run'
    return {'status':'passed' if all(v=='passed' for v in checks.values())else'not_run','tests':checks,'evidence':'reports/platform-evidence.json'}

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',default='platform.json');args=p.parse_args()
    run=subprocess.run(['cargo','test','--workspace','--locked','--color','never'],cwd=ROOT,text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,check=False)
    print(run.stdout,end='')
    passed=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',run.stdout)))
    result={'schema_version':1,'system':platform.system(),'machine':platform.machine(),
        'numerical_sha256':numerical_fingerprint(),'commit_sha':os.environ.get('GITHUB_SHA'),
        'run_id':os.environ.get('GITHUB_RUN_ID'),'run_attempt':os.environ.get('GITHUB_RUN_ATTEMPT'),
        'returncode':run.returncode,'passed_tests':passed,
        'rust_version':subprocess.check_output(['rustc','--version'],text=True).strip(),
        'test_log_sha256':hashlib.sha256(run.stdout.encode()).hexdigest()}
    Path(args.output).write_text(json.dumps(result,indent=2)+'\n')
    print('SPICE_PLATFORM_EVIDENCE='+json.dumps(result,sort_keys=True))
    return run.returncode or (1 if passed==0 else 0)
if __name__=='__main__':sys.exit(main())
