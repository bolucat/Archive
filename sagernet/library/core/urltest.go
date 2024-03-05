package libcore

import (
	"context"
	"fmt"
	"math/rand"
	gonet "net"
	"net/http"
	"net/url"
	"time"

	"github.com/v2fly/v2ray-core/v5/app/proxyman"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/session"
)

func UrlTest(instance *V2RayInstance, inbound string, link string, timeout int32) (int32, error) {
	connTestUrl, err := url.Parse(link)
	if err != nil {
		return 0, err
	}
	address := net.ParseAddress(connTestUrl.Hostname())
	if address.Family().IsDomain() {
		resolver := &net.Resolver{
			PreferGo: true,
			Dial: func(ctx context.Context, network, address string) (net.Conn, error) {
				ctx = session.ContextWithContent(ctx, &session.Content{
					Protocol: "dns",
				})
				conn, err := instance.dialContext(ctx, net.Destination{
					Network: net.Network_UDP,
					Address: dnsAddress,
					Port:    53,
				})
				if err == nil {
					conn = &pinnedPacketConn{conn}
				}
				return conn, err
			},
		}
		ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
		_, err = resolver.LookupIP(ctx, "ip", address.Domain())
		cancel()
		if err != nil {
			return 0, err
		}
	}
	transport := &http.Transport{
		TLSHandshakeTimeout: time.Duration(timeout) * time.Millisecond,
		DisableKeepAlives:   true,
		DialContext: func(ctx context.Context, network, addr string) (net.Conn, error) {
			dest, err := net.ParseDestination(fmt.Sprintf("%s:%s", network, addr))
			if err != nil {
				return nil, err
			}
			inConn, outConn := gonet.Pipe()
			if inbound != "" {
				ctx = session.ContextWithInbound(ctx, &session.Inbound{Tag: inbound, Conn: outConn})
			}
			ctx = proxyman.SetPreferUseIP(ctx, true)
			go instance.dispatchContext(ctx, dest, outConn)
			return inConn, nil
		},
	}
	req, err := http.NewRequestWithContext(context.Background(), "GET", link, nil)
	req.Header.Set("User-Agent", fmt.Sprintf("curl/7.%d.%d", rand.Int()%54, rand.Int()%2))
	if err != nil {
		return 0, err
	}
	start := time.Now()
	resp, err := (&http.Client{
		Transport: transport,
		Timeout:   time.Duration(timeout) * time.Millisecond,
	}).Do(req)
	if err == nil && resp.StatusCode != http.StatusNoContent && resp.StatusCode != http.StatusOK {
		err = fmt.Errorf("unexcpted response status: %d", resp.StatusCode)
	}
	if err != nil {
		return 0, err
	}
	return int32(time.Since(start).Milliseconds()), nil
}
