package libcore

import (
	"io"
	"net"
	"os"

	"github.com/ulikunitz/xz"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"libcore/comm"
)

type packetConn interface {
	readFrom() (buffer *buf.Buffer, addr net.Addr, err error)
	writeTo(buffer *buf.Buffer, addr net.Addr) (err error)
	io.Closer
}

func Unxz(archive string, path string) error {
	i, err := os.Open(archive)
	if err != nil {
		return err
	}
	r, err := xz.NewReader(i)
	if err != nil {
		comm.CloseIgnore(i)
		return err
	}
	o, err := os.Create(path)
	if err != nil {
		comm.CloseIgnore(i)
		return err
	}
	_, err = io.Copy(o, r)
	comm.CloseIgnore(i, o)
	return err
}

func unxz(path string) error {
	err := Unxz(path, path+".tmp")
	if err != nil {
		return err
	}
	return os.Rename(path+".tmp", path)
}
