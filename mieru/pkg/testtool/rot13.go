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

package testtool

import (
	"fmt"
	"io"
	mrand "math/rand"
	"net"
	"regexp"

	"github.com/enfein/mieru/v3/pkg/stderror"
)

// TestHelperGenRot13Input generates valid rotate-13 input.
func TestHelperGenRot13Input(size int) []byte {
	if size <= 0 {
		return []byte{}
	}
	data := make([]byte, size)
	for i := 0; i < size; i++ {
		p := mrand.Float32()
		if p <= 0.5 {
			data[i] = byte(mrand.Int31n(26) + 65)
		} else {
			data[i] = byte(mrand.Int31n(26) + 97)
		}
	}
	return data
}

// TestHelperRot13 returns the rotate-13 of the input.
func TestHelperRot13(in []byte) ([]byte, error) {
	if len(in) == 0 {
		return nil, fmt.Errorf("input is empty")
	}
	match, err := regexp.MatchString("[A-Za-z]+", string(in))
	if err != nil {
		return nil, fmt.Errorf("regexp.MatchString() failed: %w", err)
	}
	if !match {
		return nil, fmt.Errorf("input format is invalid")
	}
	out := make([]byte, len(in))
	for i := 0; i < len(in); i++ {
		if (in[i] >= 65 && in[i] <= 77) || (in[i] >= 97 && in[i] <= 109) {
			out[i] = in[i] + 13
		} else if (in[i] >= 78 && in[i] <= 90) || (in[i] >= 110 && in[i] <= 122) {
			out[i] = in[i] - 13
		} else {
			return nil, fmt.Errorf("input format is invalid")
		}
	}
	return out, nil
}

// TestHelperServeConn serves client requests and returns the rotate-13 of
// the input.
func TestHelperServeConn(conn io.ReadWriteCloser) error {
	defer conn.Close()
	buf := make([]byte, 1024*1024) // maximum Read() or Write() size is 1 MB
	for {
		n, err := conn.Read(buf)
		if err != nil {
			return fmt.Errorf("Read() failed: %w", err)
		}
		if n == 0 {
			continue
		}
		out, err := TestHelperRot13(buf[:n])
		if err != nil {
			return fmt.Errorf("rot13() failed: %w", err)
		}
		if _, err = conn.Write(out); err != nil {
			return fmt.Errorf("Write() failed: %w", err)
		}
	}
}

// TestHelperServer is a testing server.
type TestHelperServer struct {
	done chan struct{}
}

// NewTestHelperServer initializes a new testing server.
func NewTestHelperServer() *TestHelperServer {
	return &TestHelperServer{done: make(chan struct{})}
}

// Serve serves incoming connections.
func (s *TestHelperServer) Serve(l net.Listener) error {
	var connErr error
	for {
		select {
		case <-s.done:
			return nil
		default:
		}
		if connErr != nil {
			return connErr
		}
		conn, err := l.Accept()
		if err != nil {
			if stderror.IsEOF(err) || stderror.IsClosed(err) {
				return nil
			}
			return err
		}
		go func() {
			err := TestHelperServeConn(conn)
			if err != nil && !stderror.IsEOF(err) && !stderror.IsClosed(err) {
				connErr = err
			}
		}()
	}
}

// Close stops the testing server.
// To be more flexible, it doesn't shutdown the listener.
// The server can't be reused after close.
func (s *TestHelperServer) Close() error {
	select {
	case <-s.done:
		return nil
	default:
	}
	close(s.done)
	return nil
}
