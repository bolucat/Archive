#!/usr/bin/env python3
"""Check compiled SSDK DTBs and board defaults against original physical roles.
Usage: check-topology.py DIRECTORY_CONTAINING_COMPILED_DTBS
"""
from pathlib import Path
import json
import subprocess
import sys

here = Path(__file__).resolve().parent
manifest = json.loads((here / 'psgmii-port-map.json').read_text())['boards']
network = (here.parent / 'base-files/etc/board.d/02_network').read_text()
start = network.index('ipq40xx_setup_ssdk_interfaces()')
end = network.index('ipq40xx_setup_dsl()')
mock = '''json_select() { :; }
json_get_keys() { eval "$1="; }
ucidef_add_switch() { printf 'ports|'; shift; printf '%s ' "$@"; printf '\\n'; }
ucidef_set_interfaces_lan_wan() { printf 'lan|%s\\nwan|%s\\n' "$1" "$2"; }
ucidef_set_interface_lan() { printf 'lan|%s\\n' "$1"; }
ucidef_set_interface() { printf 'extra|%s\\n' "$*"; }
'''

def get(board, node, prop, kind='x'):
    return subprocess.check_output(['fdtget', '-t', kind,
        str(Path(sys.argv[1]) / (board + '.dtb')), node, prop], text=True).strip()

for row in manifest:
    board = row['board']
    assert get(board, '/soc/switch@c000000', 'compatible', 's') == 'qcom,ess-switch', board
    assert get(board, '/soc/ethernet@c080000', 'compatible', 's') == 'qcom,ess-edma', board
    assert int(get(board, '/soc/switch@c000000', 'switch_mac_mode'), 16) == row.get('mac_mode', 0), board
    if 'phy_mode' in row:
        assert get(board, '/soc/switch@c000000/ports/port@5', 'phy-mode', 's') == row['phy_mode'], board
    for role, mask in row['masks'].items():
        assert int(get(board, '/soc/switch@c000000', 'switch_' + role + '_bmp'), 16) == mask, board
    assert get(board, '/soc/switch@c000000', 'switch_cpu_bmp') == '1', board
    for index, role in [(0, 'wan'), (1, 'lan')]:
        tag = [int(x, 16) for x in get(board, f'/soc/ethernet@c080000/gmac{index}', 'vlan-tag').split()]
        assert tag == [2 if index == 0 else 1, row['masks'][role]], (board, tag)
    for port, phy in row['phys'].items():
        path = '/soc/switch@c000000/qcom,port_phyinfo/port' + port
        assert int(get(board, path, 'port_id'), 16) == int(port), board
        assert int(get(board, path, 'phy_address'), 16) == phy, board
    out = subprocess.check_output(['sh', '-c', mock + network[start:end] +
        '\nipq40xx_setup_interfaces "$1"', 'sh', row['compatible']], text=True)
    values = dict(line.split('|', 1) for line in out.splitlines())
    ports = values['ports'].split()
    physical = [p for p in ports if '@' not in p]
    assert sorted(physical) == sorted(row['ports']), (board, physical)
    assert ports.count('0t@eth1') == 1, board
    assert values['lan'] == 'eth1', board
    if row['masks']['wan']:
        assert ports.count('0t@eth0') == 1 and values['wan'] == 'eth0', board
    else:
        assert '0t@eth0' not in ports and 'wan' not in values, board
    assert values.get('extra', '').split() == row['network'].get('extra', []), board
print(f'{len(manifest)} PSGMII/RGMII boards: DT masks, PHY addresses, physical roles and GMAC defaults PASS')

for row in json.loads((here / 'psgmii-port-map.json').read_text())['inactive_boards']:
    board = row['board']
    for node, compatible in [('/soc/switch@c000000', 'qcom,ess-switch'),
                             ('/soc/ethernet@c080000', 'qcom,ess-edma')]:
        assert get(board, node, 'compatible', 's') == compatible, board
        assert get(board, node, 'status', 's') == 'disabled', board
    out = subprocess.check_output(['sh', '-c', mock + network[start:end] +
        '\nipq40xx_setup_interfaces "$1"', 'sh', row['compatible']], text=True)
    assert out.strip() == 'lan|eth1', (board, out)
print('10 previously disabled profiles: QSDK bindings and disabled hardware retained PASS')
