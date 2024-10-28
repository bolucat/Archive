#!/usr/bin/env python3

import os
import subprocess

def check_string_output(command):
  return subprocess.check_output(command, stderr=subprocess.STDOUT).decode().strip()

_codenames_db = {}

def open_ubuntu_csv_file(csvpath):
  import csv
  with open(csvpath, newline='') as csvfile:
    reader = csv.DictReader(csvfile, delimiter=',')
    for row in reader:
      version = row['version'].replace(' LTS', '')
      codename = row['series']
      key = codename
      value = 'ubuntu-' + version + '-' + codename
      _codenames_db[key] = value

def open_debian_csv_file(csvpath):
  import csv
  with open(csvpath, newline='') as csvfile:
    reader = csv.DictReader(csvfile, delimiter=',')
    for row in reader:
      version = row['version']
      codename = row['series']
      key = codename
      if version:
        value = 'debian-' + version + '-' + codename
      else:
        value = 'debian-' + codename
      _codenames_db[key] = value


def init_codenames_from_distro_info_data():
  try:
    open_ubuntu_csv_file('/usr/share/distro-info/ubuntu.csv')
  except IOError:
    # shipped by distro-info-data 0.60ubuntu0.2
    open_ubuntu_csv_file('./distro-info/ubuntu.csv')
    pass
  try:
    open_debian_csv_file('/usr/share/distro-info/debian.csv')
  except IOError:
    # shipped by distro-info-data 0.60ubuntu0.2
    open_debian_csv_file('./distro-info/debian.csv')
    pass

def main():
  os.chdir(os.path.dirname(os.path.abspath(__file__)))
  import argparse
  parser = argparse.ArgumentParser()
  parser.add_argument('codename', nargs='?', help='Debian Distribution Name')
  args = parser.parse_args()

  codename = args.codename
  if not codename:
    codename = check_string_output(['lsb_release', '-sc']).lower()

  init_codenames_from_distro_info_data()

  if codename in _codenames_db:
    print(_codenames_db[codename])
  else:
    id = check_string_output(['lsb_release', '-si']).lower()
    release = check_string_output(['lsb_release', '-sr'])
    if codename != 'n/a':
      print("%s-%s-%s" % (id, release, codename))
    else:
      print("%s-%s" % (id, release))

if __name__ == '__main__':
  main()
