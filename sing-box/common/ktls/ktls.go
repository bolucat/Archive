//go:build linux && go1.25 && !without_badtls

package ktls

import (
	"crypto/tls"
	"io"
	"net"
	"os"
	"syscall"

	"github.com/sagernet/sing-box/common/badtls"
	// C "github.com/sagernet/sing-box/constant"
	E "github.com/sagernet/sing/common/exceptions"
	N "github.com/sagernet/sing/common/network"
	aTLS "github.com/sagernet/sing/common/tls"
)

type Conn struct {
	aTLS.Conn
	conn            net.Conn
	rawConn         *badtls.RawConn
	rawSyscallConn  syscall.RawConn
	readWaitOptions N.ReadWaitOptions
	kernelTx        bool
	kernelRx        bool
	tmp             [16]byte
}

func NewConn(conn aTLS.Conn, txOffload, rxOffload bool) (aTLS.Conn, error) {
	syscallConn, isSyscallConn := N.CastReader[interface {
		io.Reader
		syscall.Conn
	}](conn.NetConn())
	if !isSyscallConn {
		return nil, os.ErrInvalid
	}
	rawSyscallConn, err := syscallConn.SyscallConn()
	if err != nil {
		return nil, err
	}
	rawConn, err := badtls.NewRawConn(conn)
	if err != nil {
		return nil, err
	}
	if *rawConn.Vers != tls.VersionTLS13 {
		return nil, os.ErrInvalid
	}
	for rawConn.RawInput.Len() > 0 {
		err = rawConn.ReadRecord()
		if err != nil {
			return nil, err
		}
		for rawConn.Hand.Len() > 0 {
			err = rawConn.HandlePostHandshakeMessage()
			if err != nil {
				return nil, E.Cause(err, "ktls: failed to handle post-handshake messages")
			}
		}
	}
	kConn := &Conn{
		Conn:           conn,
		conn:           conn.NetConn(),
		rawConn:        rawConn,
		rawSyscallConn: rawSyscallConn,
	}
	err = kConn.setupKernel(txOffload, rxOffload)
	if err != nil {
		return nil, err
	}
	return kConn, nil
}

func (c *Conn) Upstream() any {
	return c.conn
}

func (c *Conn) ReaderReplaceable() bool {
	return c.kernelRx
}

func (c *Conn) WriterReplaceable() bool {
	return c.kernelTx
}
