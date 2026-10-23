#!/bin/bash

case "$1" in
    remove | 0)
        systemctl disable sing-box-daemon.service || true
        if [ -d /run/systemd/system ]; then
            systemctl stop sing-box-daemon.service || true
        fi
        ;;
esac
