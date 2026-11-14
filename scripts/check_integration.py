#!/usr/bin/env python3
"""Execute the public examples as an independent Cargo consumer and CLI client."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--git', action='store_true',
                        help='consume the current committed revision through a local Git URL')
    args = parser.parse_args()
    env = os.environ.copy()
    target = Path(env.get('CARGO_TARGET_DIR', ROOT / 'target')).resolve()
    env['CARGO_TARGET_DIR'] = str(target)
    if args.git:
        revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        dependency = f'git = {json.dumps(ROOT.as_uri())}, rev = {json.dumps(revision)}'
    else:
        dependency = f'path = {json.dumps((ROOT / "crates/risc-c").as_posix())}'
    with tempfile.TemporaryDirectory(prefix='risc-c-consumer-') as directory:
        consumer = Path(directory)
        (consumer / 'src/bin').mkdir(parents=True)
        (consumer / 'Cargo.toml').write_text(
            '[package]\nname = "risc-c-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
            f'[dependencies]\nrisc-c = {{ {dependency} }}\n', encoding='utf-8')
        for name in ['netlist', 'builder', 'transient', 'ac', 'stream']:
            shutil.copyfile(ROOT / f'crates/risc-c/examples/{name}.rs', consumer / f'src/bin/{name}.rs')
            subprocess.run(['cargo', 'run', '--quiet', '--bin', name], cwd=consumer,
                           env=env, check=True, timeout=600)
        executable = 'spice.exe' if os.name == 'nt' else 'spice'
        if args.git:
            # Exercise the documented install route without modifying the user's PATH.
            tools = consumer / 'tools'
            subprocess.run(['cargo', 'install', '--git', ROOT.as_uri(), '--rev', revision,
                            '--locked', '--debug', '--root', str(tools), 'risc-c-cli'],
                           cwd=consumer, env=env, check=True, timeout=600)
            spice = tools / 'bin' / executable
        else:
            subprocess.run(['cargo', 'build', '-p', 'risc-c-cli', '--locked'], cwd=ROOT,
                           env=env, check=True, timeout=600)
            spice = target / 'debug' / executable
        subprocess.run([str(spice), '--version'], cwd=consumer, check=True, timeout=30)
        result = subprocess.run(
            [sys.executable, str(ROOT / 'examples/integrate_cli.py'), '--spice', str(spice)],
            cwd=consumer, text=True, capture_output=True, check=True, timeout=30)
        if result.stdout.strip() != 'v(out) = 5':
            raise ValueError(f'Unexpected divider result: {result.stdout}')
        print(result.stdout, end='')
    print('Independent Rust consumer and CLI integration passed.')


if __name__ == '__main__':
    main()
