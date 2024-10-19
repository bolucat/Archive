// Copyright (C) 2023  mieru authors
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

package socks5

import (
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"

	"github.com/enfein/mieru/v3/apis/constant"
	"github.com/enfein/mieru/v3/pkg/common"
	"github.com/enfein/mieru/v3/pkg/log"
	"github.com/enfein/mieru/v3/pkg/metrics"
)

const (
	clientTimeout time.Duration = 10 * time.Second
)

var (
	HTTPMetricGroupName = "HTTP proxy"

	HTTPRequests     = metrics.RegisterMetric(HTTPMetricGroupName, "Requests", metrics.COUNTER)
	HTTPConnErrors   = metrics.RegisterMetric(HTTPMetricGroupName, "ConnErrors", metrics.COUNTER)
	HTTPSchemeErrors = metrics.RegisterMetric(HTTPMetricGroupName, "SchemeErrors", metrics.COUNTER)
)

// hopByHopHeaders are HTTP headers that need to be removed by intermediaries.
//
// See RFC 9110 for details.
var hopByHopHeaders = []string{
	"Connection",
	"Proxy-Connection",
	"Keep-Alive",
	"TE",
	"Transfer-Encoding",
	"Upgrade",
}

type HTTPProxy struct {
	ProxyURI string
	client   *http.Client // cached HTTP client
	mu       sync.Mutex
}

var (
	_ http.Handler = &HTTPProxy{}
)

// NewHTTPProxyServer returns a new HTTP proxy server.
func NewHTTPProxyServer(listenAddr string, proxy *HTTPProxy) *http.Server {
	if proxy == nil {
		return nil
	}
	return &http.Server{
		Addr:           listenAddr,
		Handler:        proxy,
		ReadTimeout:    10 * time.Second,
		WriteTimeout:   10 * time.Second,
		MaxHeaderBytes: 1 << 20,
	}
}

// ServeHTTP implements http.Handler interface with a socks5 backend.
func (p *HTTPProxy) ServeHTTP(res http.ResponseWriter, req *http.Request) {
	HTTPRequests.Add(1)
	if log.IsLevelEnabled(log.TraceLevel) {
		log.Tracef("received HTTP proxy request %s %s", req.Method, req.URL.String())
	}

	// Dialer to socks5 server.
	dialFunc := Dial(p.ProxyURI, constant.Socks5ConnectCmd)

	if req.Method == http.MethodConnect {
		// HTTPS
		// Hijack the HTTP connection.
		hijacker, ok := res.(http.Hijacker)
		if !ok {
			HTTPConnErrors.Add(1)
			log.Debugf("http.ResponseWriter doesn't implement http.Hijacker interface")
			return
		}
		httpConn, _, err := hijacker.Hijack()
		if err != nil {
			HTTPConnErrors.Add(1)
			log.Debugf("hijack HTTP connection failed: %v", err)
			return
		}

		// Determine the destination port number.
		port := req.URL.Port()
		if port == "" {
			switch req.URL.Scheme {
			case "http":
				port = "80"
			case "https":
				port = "443"
			default:
				// Unable to determine the port number.
				HTTPSchemeErrors.Add(1)
				log.Debugf("unable to determine HTTP port number from %s", req.URL.Redacted())
				return
			}
		}

		// Dial to socks server.
		socksConn, err := dialFunc("tcp", common.MaybeDecorateIPv6(req.URL.Hostname())+":"+port)
		if err != nil {
			HTTPConnErrors.Add(1)
			log.Debugf("HTTP proxy dial to socks5 server failed: %v", err)
			return
		}
		httpConn.Write([]byte("HTTP/1.1 200 Connection Established\r\n\r\n"))
		common.BidiCopy(httpConn, socksConn)
	} else {
		// HTTP
		p.mu.Lock()
		if p.client == nil {
			tr := &http.Transport{
				Dial: dialFunc,
			}
			p.client = &http.Client{
				Transport: tr,
				CheckRedirect: func(req *http.Request, via []*http.Request) error {
					return nil
				},
				Timeout: clientTimeout,
			}
		}
		p.mu.Unlock()
		defer p.client.CloseIdleConnections()

		outReq := *req
		outReq.RequestURI = ""
		deleteHopByHopHeaders(outReq.Header)

		resp, err := p.client.Do(&outReq)
		if err != nil {
			HTTPConnErrors.Add(1)
			log.Debugf("send HTTP proxy request to socks5 server failed: %v", err)
			return
		}
		defer resp.Body.Close()

		deleteHopByHopHeaders(resp.Header)
		copyHeaders(res.Header(), resp.Header)
		res.WriteHeader(resp.StatusCode)
		io.Copy(res, resp.Body)
	}
}

// HTTPTransportProxyFunc returns the Proxy function used by http.Transport.
func HTTPTransportProxyFunc(proxy string) func(*http.Request) (*url.URL, error) {
	if !strings.HasPrefix(proxy, "http://") && !strings.HasPrefix(proxy, "https://") && !strings.HasPrefix(proxy, "socks5://") {
		return func(r *http.Request) (*url.URL, error) {
			return nil, fmt.Errorf("unsupport proxy URL %s", proxy)
		}
	}
	return func(r *http.Request) (*url.URL, error) {
		return url.Parse(proxy)
	}
}

func copyHeaders(dst, src http.Header) {
	for k, vv := range src {
		for _, v := range vv {
			dst.Add(k, v)
		}
	}
}

func deleteHopByHopHeaders(header http.Header) {
	for _, h := range hopByHopHeaders {
		header.Del(h)
	}
}
