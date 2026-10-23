#!/bin/bash

systemctl disable sing-box-daemon.service || true
if [ -d /run/systemd/system ]; then
    systemctl stop sing-box-daemon.service || true
fi
