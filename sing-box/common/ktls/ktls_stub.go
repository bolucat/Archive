//go:build !linux || !go1.25 || without_badtls

package ktls

import (
	"os"

	aTLS "github.com/sagernet/sing/common/tls"
)

func NewConn(conn aTLS.Conn, txOffload, rxOffload bool) (aTLS.Conn, error) {
	return nil, os.ErrInvalid
}
