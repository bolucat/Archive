#!/bin/bash

if [ -d /run/systemd/system ]; then
    systemctl disable --now sing-box-daemon.service || true
fi
