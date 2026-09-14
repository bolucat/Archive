#!/bin/bash

ln -sf /opt/sing-box/sing-box /usr/bin/sing-box

if ! { [[ -L /proc/self/ns/user ]] && unshare --user true; }; then
    chmod 4755 /opt/sing-box/chrome-sandbox || true
else
    chmod 0755 /opt/sing-box/chrome-sandbox || true
fi

if hash update-mime-database 2>/dev/null; then
    update-mime-database /usr/share/mime || true
fi

if hash update-desktop-database 2>/dev/null; then
    update-desktop-database /usr/share/applications || true
fi

if [ -d /run/systemd/system ]; then
    systemctl daemon-reload
    if systemctl cat polkit-agent-helper.socket >/dev/null 2>&1; then
        systemctl start polkit-agent-helper.socket
    fi
    systemctl enable sing-box-daemon.service
    systemctl restart sing-box-daemon.service
else
    echo "systemd is not running, skipping the sing-box service startup"
fi
