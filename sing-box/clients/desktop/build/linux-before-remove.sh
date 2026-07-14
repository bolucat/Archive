#!/bin/bash

case "$1" in
    remove | 0)
        if [ -d /run/systemd/system ]; then
            systemctl disable --now sing-box-daemon.service || true
        fi
        ;;
esac
