package process

import (
	"encoding/binary"
	"net/netip"
	"strconv"
	"strings"
	"syscall"
	"unsafe"

	"golang.org/x/sys/unix"
)

const (
	procpidpathinfo     = 0xb
	procpidpathinfosize = 1024
	proccallnumpidinfo  = 0x2
)

var structSize = func() int {
	value, _ := syscall.Sysctl("kern.osrelease")
	major, _, _ := strings.Cut(value, ".")
	n, _ := strconv.ParseInt(major, 10, 64)
	switch true {
	case n >= 22:
		return 408
	default:
		// from darwin-xnu/bsd/netinet/in_pcblist.c:get_pcblist_n
		// size/offset are round up (aligned) to 8 bytes in darwin
		// rup8(sizeof(xinpcb_n)) + rup8(sizeof(xsocket_n)) +
		// 2 * rup8(sizeof(xsockbuf_n)) + rup8(sizeof(xsockstat_n))
		return 384
	}
}()

func findProcessName(network string, ip netip.Addr, port int) (uint32, string, error) {
	var spath string
	switch network {
	case TCP:
		spath = "net.inet.tcp.pcblist_n"
	case UDP:
		spath = "net.inet.udp.pcblist_n"
	default:
		return 0, "", ErrInvalidNetwork
	}

	isIPv4 := ip.Is4()

	value, err := unix.SysctlRaw(spath)
	if err != nil {
		return 0, "", err
	}

	buf := value
	itemSize := structSize
	if network == TCP {
		// rup8(sizeof(xtcpcb_n))
		itemSize += 208
	}

	var fallbackUDPProcess string
	// skip the first xinpgen(24 bytes) block
	for i := 24; i+itemSize <= len(buf); i += itemSize {
		// offset of xinpcb_n and xsocket_n
		inp, so := i, i+104

		srcPort := binary.BigEndian.Uint16(buf[inp+18 : inp+20])
		if uint16(port) != srcPort {
			continue
		}

		// xinpcb_n.inp_vflag
		flag := buf[inp+44]

		var (
			srcIP     netip.Addr
			srcIsIPv4 bool
		)
		switch {
		case flag&0x1 > 0 && isIPv4:
			// ipv4
			srcIP, _ = netip.AddrFromSlice(buf[inp+76 : inp+80])
			srcIsIPv4 = true
		case flag&0x2 > 0 && !isIPv4:
			// ipv6
			srcIP, _ = netip.AddrFromSlice(buf[inp+64 : inp+80])
		default:
			continue
		}
		srcIP = srcIP.Unmap()

		if ip == srcIP {
			// xsocket_n.so_last_pid
			pid := readNativeUint32(buf[so+68 : so+72])
			pp, err := getExecPathFromPID(pid)
			return 0, pp, err
		}

		// udp packet connection may be not equal with srcIP
		if network == UDP && srcIP.IsUnspecified() && isIPv4 == srcIsIPv4 {
			fallbackUDPProcess, _ = getExecPathFromPID(readNativeUint32(buf[so+68 : so+72]))
		}
	}

	if network == UDP && fallbackUDPProcess != "" {
		return 0, fallbackUDPProcess, nil
	}

	return 0, "", ErrNotFound
}

func getExecPathFromPID(pid uint32) (string, error) {
	buf := make([]byte, procpidpathinfosize)
	_, _, errno := syscall.Syscall6(
		syscall.SYS_PROC_INFO,
		proccallnumpidinfo,
		uintptr(pid),
		procpidpathinfo,
		0,
		uintptr(unsafe.Pointer(&buf[0])),
		procpidpathinfosize)
	if errno != 0 {
		return "", errno
	}

	return unix.ByteSliceToString(buf), nil
}

func readNativeUint32(b []byte) uint32 {
	return *(*uint32)(unsafe.Pointer(&b[0]))
}
