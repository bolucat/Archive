package libcore

import (
	"container/list"
	"net"
	"sync"
	"sync/atomic"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

type AppStats struct {
	Uid          int32
	TcpConn      int32
	UdpConn      int32
	TcpConnTotal int32
	UdpConnTotal int32

	Uplink        int64
	Downlink      int64
	UplinkTotal   int64
	DownlinkTotal int64

	DeactivateAt int32
}

type appStats struct {
	sync.Mutex

	tcpConn      int32
	udpConn      int32
	tcpConnTotal uint32
	udpConnTotal uint32

	uplink        uint64
	downlink      uint64
	uplinkTotal   uint64
	downlinkTotal uint64

	deactivateAt int64

	connections list.List
}

type TrafficListener interface {
	UpdateStats(t *AppStats)
}

func (t *Tun2ray) GetTrafficStatsEnabled() bool {
	return t.trafficStats
}

func (t *Tun2ray) ResetAppTraffics() {
	if !t.trafficStats {
		return
	}

	var toDel []uint16
	t.appStats.Range(func(key, value interface{}) bool {
		uid := key.(uint16)
		toDel = append(toDel, uid)
		return true
	})
	for _, uid := range toDel {
		t.appStats.Delete(uid)
	}
}

func (t *Tun2ray) CloseConnections(uid int32) {
	if !t.trafficStats {
		return
	}
	statsI, ok := t.appStats.Load(uint16(uid))
	if !ok {
		return
	}
	stats := statsI.(*appStats)
	stats.Lock()
	defer stats.Unlock()
	for element := stats.connections.Front(); element != nil; element = element.Next() {
		common.Close(element.Value)
	}
}

func (t *Tun2ray) ReadAppTraffics(listener TrafficListener) error {
	if !t.trafficStats {
		return nil
	}

	var stats []*AppStats

	t.appStats.Range(func(key, value interface{}) bool {
		uid := key.(uint16)
		stat := value.(*appStats)
		export := &AppStats{
			Uid:          int32(uid),
			TcpConn:      stat.tcpConn,
			UdpConn:      stat.udpConn,
			TcpConnTotal: int32(stat.tcpConnTotal),
			UdpConnTotal: int32(stat.udpConnTotal),
			DeactivateAt: int32(stat.deactivateAt),
		}

		uplink := atomic.SwapUint64(&stat.uplink, 0)
		uplinkTotal := atomic.AddUint64(&stat.uplinkTotal, uplink)
		export.Uplink = int64(uplink)
		export.UplinkTotal = int64(uplinkTotal)

		downlink := atomic.SwapUint64(&stat.downlink, 0)
		downlinkTotal := atomic.AddUint64(&stat.downlinkTotal, downlink)
		export.Downlink = int64(downlink)
		export.DownlinkTotal = int64(downlinkTotal)

		stats = append(stats, export)
		return true
	})

	for _, stat := range stats {
		listener.UpdateStats(stat)
	}

	return nil
}

func NewStatsCounterConn(originConn net.Conn, uplink *uint64, downlink *uint64) *internet.StatCounterConn {
	conn := new(internet.StatCounterConn)
	conn.Connection = originConn
	conn.ReadCounter = statsConnWrapper{uplink}
	conn.WriteCounter = statsConnWrapper{downlink}
	return conn
}

type statsConnWrapper struct {
	counter *uint64
}

func (w statsConnWrapper) Value() int64 {
	return int64(atomic.LoadUint64(w.counter))
}

func (w statsConnWrapper) Set(i int64) int64 {
	value := w.Value()
	atomic.StoreUint64(w.counter, uint64(i))
	return value
}

func (w statsConnWrapper) Add(i int64) int64 {
	atomic.AddUint64(w.counter, uint64(i))
	return 0
}

type statsPacketConn struct {
	packetConn
	uplink   *uint64
	downlink *uint64
}

func (c statsPacketConn) readFrom() (buffer *buf.Buffer, addr net.Addr, err error) {
	buffer, addr, err = c.packetConn.readFrom()
	if err == nil {
		atomic.AddUint64(c.downlink, uint64(buffer.Len()))
	}
	return
}

func (c statsPacketConn) writeTo(buffer *buf.Buffer, addr net.Addr) (err error) {
	length := buffer.Len()
	err = c.packetConn.writeTo(buffer, addr)
	if err == nil {
		atomic.AddUint64(c.uplink, uint64(length))
	}
	return
}
