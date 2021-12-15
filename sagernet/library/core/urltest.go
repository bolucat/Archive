package libcore

import (
	"context"
	"fmt"
	"math/rand"
	"net/http"
	"time"

	"github.com/v2fly/v2ray-core/v4"
	"github.com/v2fly/v2ray-core/v4/common/net"
	"github.com/v2fly/v2ray-core/v4/common/session"
)

func urlTest(dialContext func(ctx context.Context, network, addr string) (net.Conn, error), link string, timeout int32) (int32, error) {
	transport := &http.Transport{
		TLSHandshakeTimeout: time.Duration(timeout) * time.Millisecond,
		DisableKeepAlives:   true,
		DialContext:         dialContext,
	}
	req, err := http.NewRequestWithContext(context.Background(), "GET", link, nil)
	req.Header.Set("User-Agent", fmt.Sprintf("curl/7.%d.%d", rand.Int()%54, rand.Int()%2))
	if err != nil {
		return 0, newError("create get request").Base(err)
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

func UrlTest(instance *V2RayInstance, inbound string, link string, timeout int32) (int32, error) {
	return urlTest(func(ctx context.Context, network, addr string) (net.Conn, error) {
		dest, err := net.ParseDestination(fmt.Sprintf("%s:%s", network, addr))
		if err != nil {
			return nil, err
		}
		if inbound != "" {
			ctx = session.ContextWithInbound(ctx, &session.Inbound{Tag: inbound})
		}
		return core.Dial(ctx, instance.core, dest)
	}, link, timeout)
}
