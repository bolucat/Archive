#!/bin/bash

case "$1" in
    remove | purge | 0)
        if type update-alternatives >/dev/null 2>&1; then
            update-alternatives --remove '${executable}' '/opt/${sanitizedProductName}/${executable}'
        else
            rm -f '/usr/bin/${executable}'
        fi

        APPARMOR_PROFILE_DEST='/etc/apparmor.d/${executable}'
        if [ -f "$APPARMOR_PROFILE_DEST" ]; then
            if apparmor_status --enabled > /dev/null 2>&1; then
                if ! { [ -x '/usr/bin/ischroot' ] && /usr/bin/ischroot; } && hash apparmor_parser 2>/dev/null; then
                    apparmor_parser --remove "$APPARMOR_PROFILE_DEST" || true
                fi
            fi
            rm -f "$APPARMOR_PROFILE_DEST"
        fi

        if [ -d /run/systemd/system ]; then
            systemctl daemon-reload || true
            systemctl reset-failed sing-box-daemon.service || true
        fi
        rm -f /run/sing-box.socket
        ;;
esac
