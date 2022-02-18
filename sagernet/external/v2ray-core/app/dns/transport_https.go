package dns

import (
	"bytes"
	"context"
	"fmt"
	"io"
	"net/http"
	"time"

	"golang.org/x/net/dns/dnsmessage"

	"github.com/v2fly/v2ray-core/v5/common"
	"github.com/v2fly/v2ray-core/v5/common/buf"
	"github.com/v2fly/v2ray-core/v5/common/net"
	"github.com/v2fly/v2ray-core/v5/common/task"
	"github.com/v2fly/v2ray-core/v5/features/dns"
	"github.com/v2fly/v2ray-core/v5/features/routing"
	"github.com/v2fly/v2ray-core/v5/transport/internet"
)

var _ dns.Transport = (*HTTPSTransport)(nil)

type HTTPSTransport struct {
	*transportContext
	url        string
	httpClient *http.Client
}

func (t *HTTPSTransport) Close() error {
	if t.cache != nil {
		t.cache.Release()
	}
	t.httpClient.CloseIdleConnections()
	return nil
}

func NewHTTPSTransport(trans *transportContext, dispatcher routing.Dispatcher) *HTTPSTransport {
	return newHTTPSTransport(trans, func(ctx context.Context, network, addr string) (net.Conn, error) {
		dest, err := net.ParseDestination(network + ":" + addr)
		if err != nil {
			return nil, err
		}
		link, err := dispatcher.Dispatch(trans.newContext(), dest)
		if err != nil {
			return nil, err
		}
		return buf.NewConnection(buf.ConnectionInputMulti(link.Writer), buf.ConnectionOutputMulti(link.Reader)), nil
	})
}

func NewHTTPSLocalTransport(trans *transportContext) *HTTPSTransport {
	return newHTTPSTransport(trans, func(ctx context.Context, network, addr string) (net.Conn, error) {
		dest, err := net.ParseDestination(network + ":" + addr)
		if err != nil {
			return nil, err
		}
		return internet.DialSystemDNS(trans.newContext(), dest, nil)
	})
}

func newHTTPSTransport(ctx *transportContext, dialContext func(ctx context.Context, network, addr string) (net.Conn, error)) *HTTPSTransport {
	tr := &http.Transport{
		MaxIdleConns:        30,
		IdleConnTimeout:     5 * time.Minute,
		TLSHandshakeTimeout: 10 * time.Second,
		ForceAttemptHTTP2:   true,
		DialContext:         dialContext,
	}
	dispatchedClient := &http.Client{
		Transport: tr,
		Timeout:   60 * time.Second,
	}
	return &HTTPSTransport{
		transportContext: ctx,
		url:              ctx.destination.Address.Domain(),
		httpClient:       dispatchedClient,
	}
}

func (t *HTTPSTransport) Type() dns.TransportType {
	return dns.TransportTypeExchangeRaw
}

func (t *HTTPSTransport) ExchangeRaw(ctx context.Context, message *buf.Buffer) (*buf.Buffer, error) {
	body := bytes.NewBuffer(message.Bytes())
	req, err := http.NewRequest("POST", t.url, body)
	if err != nil {
		return nil, err
	}

	req.Header.Add("Accept", "application/dns-message")
	req.Header.Add("Content-Type", "application/dns-message")

	var response *buf.Buffer
	return response, task.Run(ctx, func() error {
		resp, err := t.httpClient.Do(req.WithContext(t.ctx))
		if err != nil {
			return err
		}

		defer resp.Body.Close()
		if resp.StatusCode != http.StatusOK {
			io.Copy(io.Discard, resp.Body) // flush resp.Body so that the conn is reusable
			return fmt.Errorf("DOH server returned code %d", resp.StatusCode)
		}

		data, err := io.ReadAll(resp.Body)
		if err != nil {
			return newError("failed to read DOH response").Base(err)
		}
		response = buf.FromBytes(data)
		return nil
	})
}

func (t *HTTPSTransport) Write(ctx context.Context, message *dnsmessage.Message) error {
	return common.ErrNoClue
}

func (t *HTTPSTransport) Exchange(ctx context.Context, message *dnsmessage.Message) (*dnsmessage.Message, error) {
	return nil, common.ErrNoClue
}

func (t *HTTPSTransport) Lookup(ctx context.Context, domain string, strategy dns.QueryStrategy) ([]net.IP, error) {
	return nil, common.ErrNoClue
}
