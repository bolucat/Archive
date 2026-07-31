#!/bin/bash

DEFAULT_NETWORK_INTERFACE=$(route get default | grep interface | awk '{print $2}')

get_hardware_port() {
    local device=$1
    networksetup -listallhardwareports | awk -v dev="$device" '
        /Hardware Port:/ { 
            port = substr($0, 15)
        }
        $0 ~ "Device: " dev { 
            print port
            exit
        }
    '
}

echo $(get_hardware_port $DEFAULT_NETWORK_INTERFACE)
