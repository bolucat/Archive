#!/bin/bash

# Copyright (C) 2026  mieru authors
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

# Make sure this script has executable permission:
# git update-index --chmod=+x <file>

set -e

# Start http server.
./httpserver -port=8080 &
sleep 2

# Start UDP server.
./udpserver -port=9090 &
sleep 1

# Start standalone SOCKS5 server.
./socks5server -host=127.0.0.1 -port=1080 -allow_loopback=true &
sleep 1

echo ">>> socks5 - new connections <<<"
if ! ./sockshttpclient -dst_host=127.0.0.1 -dst_port=8080 \
    -local_proxy_host=127.0.0.1 -local_proxy_port=1080 \
    -test_case=new_conn -num_request=1000; then
    echo "Test socks5 new_conn failed."
    exit 1
fi

echo ">>> socks5 - reuse one connection <<<"
if ! ./sockshttpclient -dst_host=127.0.0.1 -dst_port=8080 \
    -local_proxy_host=127.0.0.1 -local_proxy_port=1080 \
    -test_case=reuse_conn -num_request=1000; then
    echo "Test socks5 reuse_conn failed."
    exit 1
fi

echo ">>> socks5 UDP associate <<<"
if ! ./socksudpclient -dst_host=127.0.0.1 -dst_port=9090 \
    -local_proxy_host=127.0.0.1 -local_proxy_port=1080 \
    -interval_ms=10 -num_request=100 -num_conn=10; then
    echo "Test socks5 udp_associate failed."
    exit 1
fi

echo "Test is successful."
sleep 1
exit 0
