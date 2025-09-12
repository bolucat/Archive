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
	mu             sync.RWMutex
	offset         time.Duration
	syncSystemTime bool
	running        bool
}

func ReCreateNTPService(server string, interval time.Duration, dialerProxy string, syncSystemTime bool) {
	globalMu.Lock()
	defer globalMu.Unlock()
	service := globalSrv.Swap(nil)
	if service != nil {
		service.Stop()
	}
	if server == "" {
		return
	}
	ctx, cancel := context.WithCancel(context.Background())
	service = &Service{
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
	srv.mu.Lock()
	defer srv.mu.Unlock()
	log.Infoln("NTP service start, sync system time is %t", srv.syncSystemTime)
	err := srv.update()
	if err != nil {
		log.Errorln("Initialize NTP time failed: %s", err)
		return
	}
	srv.running = true
	go srv.loopUpdate()
}

func (srv *Service) Stop() {
	srv.mu.Lock()
	defer srv.mu.Unlock()
	if srv.running {
		srv.ticker.Stop()
		srv.cancel()
		srv.running = false
	}
}

func (srv *Service) Offset() time.Duration {
	if srv == nil {
		return 0
	}
	srv.mu.RLock()
	defer srv.mu.RUnlock()
	if srv.running {
		return srv.offset
	}
	return 0
}

func (srv *Service) update() error {
	var response *ntp.Response
	var err error
	for i := 0; i < 3; i++ {
		response, err = ntp.Exchange(srv.ctx, srv.dialer, srv.server)
		if err != nil {
			continue
		}
		offset := response.ClockOffset
		if offset > time.Duration(0) {
			log.Infoln("System clock is ahead of NTP time by %s", offset)
		} else if offset < time.Duration(0) {
			log.Infoln("System clock is behind NTP time by %s", -offset)
		}
		srv.mu.Lock()
		srv.offset = offset
		srv.mu.Unlock()
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
	for {
		select {
		case <-srv.ctx.Done():
			return
		case <-srv.ticker.C:
		}
		err := srv.update()
		if err != nil {
			log.Warnln("Sync time failed: %s", err)
		}
	}
}

func Now() time.Time {
	now := time.Now()
	if offset := globalSrv.Load().Offset(); offset.Abs() > 0 {
		now = now.Add(offset)
	}
	return now
}
