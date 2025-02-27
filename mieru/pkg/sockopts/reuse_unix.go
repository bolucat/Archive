// Copyright (C) 2022  mieru authors
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//go:build android || darwin || linux

package sockopts

import (
	"syscall"

	"golang.org/x/sys/unix"
)

// ReuseAddrPort sets SO_REUSEADDR and SO_REUSEPORT options to a given connection.
func ReuseAddrPort() Control {
	return func(network, address string, conn syscall.RawConn) error {
		var err error
		conn.Control(func(fd uintptr) { err = ReuseAddrPortRawErr()(fd) })
		return err
	}
}

func ReuseAddrPortRaw() RawControl {
	return func(fd uintptr) {
		ReuseAddrPortRawErr()(fd)
	}
}

func ReuseAddrPortRawErr() RawControlErr {
	return func(fd uintptr) error {
		// Set SO_REUSEADDR
		if err := unix.SetsockoptInt(int(fd), unix.SOL_SOCKET, unix.SO_REUSEADDR, 1); err != nil {
			return err
		}
		// Set SO_REUSEPORT
		if err := unix.SetsockoptInt(int(fd), unix.SOL_SOCKET, unix.SO_REUSEPORT, 1); err != nil {
			return err
		}
		return nil
	}
}
