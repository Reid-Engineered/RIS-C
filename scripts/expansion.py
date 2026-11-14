#!/usr/bin/env python3
"""Independent coupling/switch checklist, using the same evidence accounting as core."""
from pathlib import Path
import sys
import parity

parity.SPEC = Path('reports/expansion-scope.json')
parity.RESULT = Path('reports/expansion.json')
parity.MARKDOWN = Path('docs/EXPANSION.md')
parity.HISTORY = Path('reports/expansion-history.jsonl')
parity.REPORT_TITLE = 'Coupling and switch expansion report'
parity.REPORT_SCRIPT = 'expansion.py'
parity.SCOPE_LABEL = 'coupling and switch expansion'
parity.LOG_DIRECTORY = Path('reports/logs/expansion')
parity.SUITES = {name: parity.SUITES[name] for name in ['core', 'expansion', 'oracle']}

if __name__ == '__main__':
    sys.exit(parity.main())
