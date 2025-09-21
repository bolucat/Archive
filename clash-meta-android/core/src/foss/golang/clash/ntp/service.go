package ntp

import (
	"context"
	"sync"
	"sync/atomic"
	"time"

	"github.com/metacubex/mihomo/component/dialer"
	"github.com/metacubex/mihomo/component/proxydialer"
	"github.com/metacubex/mihomo/log"

	M "github.com/metacubex/sing/common/metadata"
	"github.com/metacubex/sing/common/ntp"
)

var globalSrv atomic.Pointer[Service]
var globalMu sync.Mutex

type Service struct {
	server         M.Socksaddr
	dialer         proxydialer.SingDialer
	ticker         *time.Ticker
	ctx            context.Context
	cancel         context.CancelFunc
	offset         atomic.Int64 // [time.Duration]
	syncSystemTime bool
}

func ReCreateNTPService(server string, interval time.Duration, dialerProxy string, syncSystemTime bool) {
	globalMu.Lock()
	defer globalMu.Unlock()
	if service := globalSrv.Swap(nil); service != nil {
		service.Stop()
	}
	if server == "" || interval <= 0 {
		return
	}
	ctx, cancel := context.WithCancel(context.Background())
	service := &Service{
		server:         M.ParseSocksaddr(server),
		dialer:         proxydialer.NewByNameSingDialer(dialerProxy, dialer.NewDialer()),
		ticker:         time.NewTicker(interval * time.Minute),
		ctx:            ctx,
		cancel:         cancel,
		syncSystemTime: syncSystemTime,
	}
	service.Start()
	globalSrv.Store(service)
}

func (srv *Service) Start() {
	log.Infoln("NTP service start, sync system time is %t", srv.syncSystemTime)
	go srv.loopUpdate()
}

func (srv *Service) Stop() {
	log.Infoln("NTP service stop")
	srv.cancel()
}

func (srv *Service) Offset() time.Duration {
	return time.Duration(srv.offset.Load())
}

func (srv *Service) update() error {
	var response *ntp.Response
	var err error
	for i := 0; i < 3; i++ {
		response, err = ntp.Exchange(srv.ctx, srv.dialer, srv.server)
		if err != nil {
			if srv.ctx.Err() != nil {
				return nil
			}
			continue
		}
		offset := response.ClockOffset
		if offset > time.Duration(0) {
			log.Infoln("System clock is ahead of NTP time by %s", offset)
		} else if offset < time.Duration(0) {
			log.Infoln("System clock is behind NTP time by %s", -offset)
		}
		srv.offset.Store(int64(offset))
		if srv.syncSystemTime {
			timeNow := response.Time
			syncErr := setSystemTime(timeNow)
			if syncErr == nil {
				log.Infoln("Sync system time success: %s", timeNow.Local().Format(ntp.TimeLayout))
			} else {
				log.Errorln("Write time to system: %s", syncErr)
				srv.syncSystemTime = false
			}
		}
		return nil
	}
	return err
}

func (srv *Service) loopUpdate() {
	defer srv.offset.Store(0)
	defer srv.ticker.Stop()
	for {
		err := srv.update()
		if err != nil {
			log.Warnln("Sync time failed: %s", err)
		}
		select {
		case <-srv.ctx.Done():
			return
		case <-srv.ticker.C:
		}
	}
}

func Now() time.Time {
	now := time.Now()
	if service := globalSrv.Load(); service != nil {
		if offset := service.Offset(); offset.Abs() > 0 {
			now = now.Add(offset)
		}
	}
	return now
}
