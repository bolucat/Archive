package libcore

import _ "unsafe"

//go:linkname clashLogCh github.com/Dreamacro/clash/log.logCh
var clashLogCh chan interface{}

func init() {
	close(clashLogCh)
}
