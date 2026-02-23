package speedtest

import (
	"fmt"
	"io"
	"math"
	"net"
	"sync/atomic"
	"time"
)

type Client struct {
	Conn net.Conn
}

// Download performs a download speed test.
// If duration > 0, runs for the specified duration (time-based mode).
// Otherwise, downloads exactly dataSize bytes (size-based mode).
// The callback cb is called every second with interval stats, and once
// at the end with done=true reporting totals.
func (c *Client) Download(dataSize uint32, duration time.Duration, cb func(time.Duration, uint64, bool)) error {
	reqSize := dataSize
	if duration > 0 {
		reqSize = math.MaxUint32
	}
	if err := writeDownloadRequest(c.Conn, reqSize); err != nil {
		return err
	}
	ok, msg, err := readDownloadResponse(c.Conn)
	if err != nil {
		return err
	}
	if !ok {
		return fmt.Errorf("server rejected download request: %s", msg)
	}

	addBytes, stop := startProgressReporter(cb)
	defer stop()

	if duration > 0 {
		c.Conn.SetReadDeadline(time.Now().Add(duration))
	}

	buf := make([]byte, chunkSize)
	startTime := time.Now()
	var totalBytes uint64
	remaining := dataSize

	for duration > 0 || remaining > 0 {
		readSize := uint32(chunkSize)
		if duration == 0 && remaining < readSize {
			readSize = remaining
		}
		n, err := c.Conn.Read(buf[:readSize])
		totalBytes += uint64(n)
		addBytes(uint64(n))
		if duration == 0 {
			remaining -= uint32(n)
		}
		if err != nil {
			if duration > 0 {
				if ne, ok := err.(net.Error); ok && ne.Timeout() {
					break
				}
			} else if remaining == 0 && err == io.EOF {
				break
			}
			return err
		}
	}

	cb(time.Since(startTime), totalBytes, true)
	return nil
}

// Upload performs an upload speed test.
// If duration > 0, runs for the specified duration (time-based mode).
// Otherwise, uploads exactly dataSize bytes (size-based mode).
// The callback cb is called every second with interval stats, and once
// at the end with done=true reporting totals. In size-based mode the
// final callback uses server-reported elapsed time and byte count.
func (c *Client) Upload(dataSize uint32, duration time.Duration, cb func(time.Duration, uint64, bool)) error {
	reqSize := dataSize
	if duration > 0 {
		reqSize = math.MaxUint32
	}
	if err := writeUploadRequest(c.Conn, reqSize); err != nil {
		return err
	}
	ok, msg, err := readUploadResponse(c.Conn)
	if err != nil {
		return err
	}
	if !ok {
		return fmt.Errorf("server rejected upload request: %s", msg)
	}

	addBytes, stop := startProgressReporter(cb)
	defer stop()

	if duration > 0 {
		c.Conn.SetWriteDeadline(time.Now().Add(duration))
	}

	buf := make([]byte, chunkSize)
	startTime := time.Now()
	var totalBytes uint64
	remaining := dataSize

	for duration > 0 || remaining > 0 {
		writeSize := uint32(chunkSize)
		if duration == 0 && remaining < writeSize {
			writeSize = remaining
		}
		n, err := c.Conn.Write(buf[:writeSize])
		totalBytes += uint64(n)
		addBytes(uint64(n))
		if duration == 0 {
			remaining -= uint32(n)
		}
		if err != nil {
			if duration > 0 {
				if ne, ok := err.(net.Error); ok && ne.Timeout() {
					break
				}
			}
			return err
		}
	}

	if duration == 0 {
		elapsed, received, err := readUploadSummary(c.Conn)
		if err != nil {
			return err
		}
		cb(elapsed, uint64(received), true)
	} else {
		cb(time.Since(startTime), totalBytes, true)
	}
	return nil
}

func startProgressReporter(cb func(time.Duration, uint64, bool)) (addBytes func(uint64), stop func()) {
	var counter uint64
	stopChan := make(chan struct{})
	go func() {
		ticker := time.NewTicker(time.Second)
		defer ticker.Stop()
		t := time.Now()
		for {
			select {
			case <-stopChan:
				return
			case <-ticker.C:
				cb(time.Since(t), atomic.SwapUint64(&counter, 0), false)
				t = time.Now()
			}
		}
	}()
	return func(n uint64) { atomic.AddUint64(&counter, n) }, func() { close(stopChan) }
}
