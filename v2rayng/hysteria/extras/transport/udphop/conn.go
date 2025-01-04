package udphop

import (
	"errors"
	"math/rand"
	"net"
	"sync"
	"syscall"
	"time"
)

const (
	packetQueueSize = 1024
	udpBufferSize   = 2048 // QUIC packets are at most 1500 bytes long, so 2k should be more than enough

	defaultHopInterval = 30 * time.Second
)

type udpHopPacketConn struct {
	Addr          net.Addr
	Addrs         []net.Addr
	HopInterval   time.Duration
	ListenUDPFunc ListenUDPFunc

	connMutex   sync.RWMutex
	prevConn    net.PacketConn
	currentConn net.PacketConn
	addrIndex   int

	readBufferSize  int
	writeBufferSize int

	recvQueue chan *udpPacket
	closeChan chan struct{}
	closed    bool

	bufPool sync.Pool
}

type udpPacket struct {
	Buf  []byte
	N    int
	Addr net.Addr
	Err  error
}

type ListenUDPFunc = func() (net.PacketConn, error)

func NewUDPHopPacketConn(addr *UDPHopAddr, hopInterval time.Duration, listenUDPFunc ListenUDPFunc) (net.PacketConn, error) {
	if hopInterval == 0 {
		hopInterval = defaultHopInterval
	} else if hopInterval < 5*time.Second {
		return nil, errors.New("hop interval must be at least 5 seconds")
	}
	if listenUDPFunc == nil {
		listenUDPFunc = func() (net.PacketConn, error) {
			return net.ListenUDP("udp", nil)
		}
	}
	addrs, err := addr.addrs()
	if err != nil {
		return nil, err
	}
	curConn, err := listenUDPFunc()
	if err != nil {
		return nil, err
	}
	hConn := &udpHopPacketConn{
		Addr:          addr,
		Addrs:         addrs,
		HopInterval:   hopInterval,
		ListenUDPFunc: listenUDPFunc,
		prevConn:      nil,
		currentConn:   curConn,
		addrIndex:     rand.Intn(len(addrs)),
		recvQueue:     make(chan *udpPacket, packetQueueSize),
		closeChan:     make(chan struct{}),
		bufPool: sync.Pool{
			New: func() interface{} {
				return make([]byte, udpBufferSize)
			},
		},
	}
	go hConn.recvLoop(curConn)
	go hConn.hopLoop()
	return hConn, nil
}

func (u *udpHopPacketConn) recvLoop(conn net.PacketConn) {
	for {
		buf := u.bufPool.Get().([]byte)
		n, addr, err := conn.ReadFrom(buf)
		if err != nil {
			u.bufPool.Put(buf)
			var netErr net.Error
			if errors.As(err, &netErr) && netErr.Timeout() {
				// Only pass through timeout errors here, not permanent errors
				// like connection closed. Connection close is normal as we close
				// the old connection to exit this loop every time we hop.
				u.recvQueue <- &udpPacket{nil, 0, nil, netErr}
			}
			return
		}
		select {
		case u.recvQueue <- &udpPacket{buf, n, addr, nil}:
			// Packet successfully queued
		default:
			// Queue is full, drop the packet
			u.bufPool.Put(buf)
		}
	}
}

func (u *udpHopPacketConn) hopLoop() {
	ticker := time.NewTicker(u.HopInterval)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			u.hop()
		case <-u.closeChan:
			return
		}
	}
}

func (u *udpHopPacketConn) hop() {
	u.connMutex.Lock()
	defer u.connMutex.Unlock()
	if u.closed {
		return
	}
	newConn, err := u.ListenUDPFunc()
	if err != nil {
		// Could be temporary, just skip this hop
		return
	}
	// We need to keep receiving packets from the previous connection,
	// because otherwise there will be packet loss due to the time gap
	// between we hop to a new port and the server acknowledges this change.
	// So we do the following:
	// Close prevConn,
	// move currentConn to prevConn,
	// set newConn as currentConn,
	// start recvLoop on newConn.
	if u.prevConn != nil {
		_ = u.prevConn.Close() // recvLoop for this conn will exit
	}
	u.prevConn = u.currentConn
	u.currentConn = newConn
	// Set buffer sizes if previously set
	if u.readBufferSize > 0 {
		_ = trySetReadBuffer(u.currentConn, u.readBufferSize)
	}
	if u.writeBufferSize > 0 {
		_ = trySetWriteBuffer(u.currentConn, u.writeBufferSize)
	}
	go u.recvLoop(newConn)
	// Update addrIndex to a new random value
	u.addrIndex = rand.Intn(len(u.Addrs))
}

func (u *udpHopPacketConn) ReadFrom(b []byte) (n int, addr net.Addr, err error) {
	for {
		select {
		case p := <-u.recvQueue:
			if p.Err != nil {
				return 0, nil, p.Err
			}
			// Currently we do not check whether the packet is from
			// the server or not due to performance reasons.
			n := copy(b, p.Buf[:p.N])
			u.bufPool.Put(p.Buf)
			return n, u.Addr, nil
		case <-u.closeChan:
			return 0, nil, net.ErrClosed
		}
	}
}

func (u *udpHopPacketConn) WriteTo(b []byte, addr net.Addr) (n int, err error) {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	if u.closed {
		return 0, net.ErrClosed
	}
	// Skip the check for now, always write to the server,
	// for the same reason as in ReadFrom.
	return u.currentConn.WriteTo(b, u.Addrs[u.addrIndex])
}

func (u *udpHopPacketConn) Close() error {
	u.connMutex.Lock()
	defer u.connMutex.Unlock()
	if u.closed {
		return nil
	}
	// Close prevConn and currentConn
	// Close closeChan to unblock ReadFrom & hopLoop
	// Set closed flag to true to prevent double close
	if u.prevConn != nil {
		_ = u.prevConn.Close()
	}
	err := u.currentConn.Close()
	close(u.closeChan)
	u.closed = true
	u.Addrs = nil // For GC
	return err
}

func (u *udpHopPacketConn) LocalAddr() net.Addr {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	return u.currentConn.LocalAddr()
}

func (u *udpHopPacketConn) SetDeadline(t time.Time) error {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	if u.prevConn != nil {
		_ = u.prevConn.SetDeadline(t)
	}
	return u.currentConn.SetDeadline(t)
}

func (u *udpHopPacketConn) SetReadDeadline(t time.Time) error {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	if u.prevConn != nil {
		_ = u.prevConn.SetReadDeadline(t)
	}
	return u.currentConn.SetReadDeadline(t)
}

func (u *udpHopPacketConn) SetWriteDeadline(t time.Time) error {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	if u.prevConn != nil {
		_ = u.prevConn.SetWriteDeadline(t)
	}
	return u.currentConn.SetWriteDeadline(t)
}

// UDP-specific methods below

func (u *udpHopPacketConn) SetReadBuffer(bytes int) error {
	u.connMutex.Lock()
	defer u.connMutex.Unlock()
	u.readBufferSize = bytes
	if u.prevConn != nil {
		_ = trySetReadBuffer(u.prevConn, bytes)
	}
	return trySetReadBuffer(u.currentConn, bytes)
}

func (u *udpHopPacketConn) SetWriteBuffer(bytes int) error {
	u.connMutex.Lock()
	defer u.connMutex.Unlock()
	u.writeBufferSize = bytes
	if u.prevConn != nil {
		_ = trySetWriteBuffer(u.prevConn, bytes)
	}
	return trySetWriteBuffer(u.currentConn, bytes)
}

func (u *udpHopPacketConn) SyscallConn() (syscall.RawConn, error) {
	u.connMutex.RLock()
	defer u.connMutex.RUnlock()
	sc, ok := u.currentConn.(syscall.Conn)
	if !ok {
		return nil, errors.New("not supported")
	}
	return sc.SyscallConn()
}

func trySetReadBuffer(pc net.PacketConn, bytes int) error {
	sc, ok := pc.(interface {
		SetReadBuffer(bytes int) error
	})
	if ok {
		return sc.SetReadBuffer(bytes)
	}
	return nil
}

func trySetWriteBuffer(pc net.PacketConn, bytes int) error {
	sc, ok := pc.(interface {
		SetWriteBuffer(bytes int) error
	})
	if ok {
		return sc.SetWriteBuffer(bytes)
	}
	return nil
}
