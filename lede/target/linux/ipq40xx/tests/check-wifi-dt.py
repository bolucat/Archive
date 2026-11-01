#!/usr/bin/env python3
"""Check mainline ath10k radio bindings while preserving existing board resources."""
from pathlib import Path
import json
import subprocess
import sys

here = Path(__file__).resolve().parent
rows = json.loads((here / 'wifi-dt-resources.json').read_text())

def get(board, node, prop, kind='bx'):
    return subprocess.check_output(['fdtget', '-t', kind,
        str(Path(sys.argv[1]) / (board + '.dtb')), node, prop], text=True).strip()

for row in rows:
    board = row['board']
    for index, (node, props) in enumerate(row['radios'].items()):
        assert get(board, node, 'compatible', 's') == 'qcom,ipq4019-wifi', board
        for prop, value in props.items():
            actual = bytes(int(x, 16) for x in get(board, node, prop).split()).hex()
            assert actual == value, (board, node, prop, actual, value)
print(f'{len(rows)} boards: ath10k bindings and preserved status/calibration/clock/reset/IRQ resources PASS')
