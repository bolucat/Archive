package libcore

import (
	"github.com/sagernet/libping"
	"github.com/xtls/xray-core/common"
	"os"
	"runtime"
)

func Setenv(key, value string) error {
	return os.Setenv(key, value)
}

func Unsetenv(key string) error {
	return os.Unsetenv(key)
}

var ipv6Mode int

func SetIPv6Mode(mode int) {
	ipv6Mode = mode
}

func IcmpPing(address string, timeout int32) (int32, error) {
	return libping.IcmpPing(address, timeout)
}

func Gc() {
	runtime.GC()
}

func closeIgnore(closer ...interface{}) {
	for _, c := range closer {
		if ca, ok := c.(common.Closable); ok {
			_ = ca.Close()
		} else if ia, ok := c.(common.Interruptible); ok {
			ia.Interrupt()
		} else if ch, ok := c.(chan interface{}); ok {
			close(ch)
		}
	}
}
