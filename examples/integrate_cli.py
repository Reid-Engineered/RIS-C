#!/usr/bin/env python3
"""Use the spice CLI from Python's standard library; no Python binding required."""
import argparse
import csv
import io
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('netlist', nargs='?', type=Path,
                        default=Path(__file__).with_name('divider.cir'))
    parser.add_argument('--spice', default='spice', help='path to the spice executable')
    parser.add_argument('--signal', default='v(out)', help='CSV signal column to read')
    args = parser.parse_args()
    try:
        result = subprocess.run(
            [args.spice, 'run', str(args.netlist), '--analysis', 'op', '--output', '-'],
            check=True, capture_output=True, text=True, timeout=60,
        )
    except subprocess.CalledProcessError as error:
        parser.exit(1, error.stderr or 'spice failed\n')
    except OSError as error:
        parser.exit(1, f'Cannot execute spice: {error}\n')
    except subprocess.TimeoutExpired:
        parser.exit(1, 'spice exceeded the 60-second example timeout\n')
    rows = list(csv.DictReader(io.StringIO(result.stdout)))
    if len(rows) != 1 or args.signal not in rows[0]:
        parser.exit(1, f'Expected one operating point with signal {args.signal}\n')
    value = float(rows[0][args.signal])
    print(f'{args.signal} = {value:.6g}')
    return value


if __name__ == '__main__':
    main()
