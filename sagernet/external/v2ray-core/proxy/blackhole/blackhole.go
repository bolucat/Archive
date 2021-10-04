//go:build !confonly
// +build !confonly

// Package blackhole is an outbound handler that blocks all connections.
package blackhole

//go:generate go run github.com/v2fly/v2ray-core/v4/common/errors/errorgen

import (
	"context"
	core "github.com/v2fly/v2ray-core/v4"
	"github.com/v2fly/v2ray-core/v4/common/buf"
	"github.com/v2fly/v2ray-core/v4/common/task"
	"github.com/v2fly/v2ray-core/v4/features/policy"
	"time"

	"github.com/v2fly/v2ray-core/v4/common"
	"github.com/v2fly/v2ray-core/v4/transport"
	"github.com/v2fly/v2ray-core/v4/transport/internet"
)

// Handler is an outbound connection that silently swallow the entire payload.
type Handler struct {
	response       ResponseConfig
	keepConnection bool
	timeout        time.Duration
}

// New creates a new blackhole handler.
func New(ctx context.Context, config *Config) (*Handler, error) {
	response, err := config.GetInternalResponse()
	if err != nil {
		return nil, err
	}
	h := &Handler{
		response:       response,
		keepConnection: config.GetKeepConnection(),
	}
	if h.keepConnection {
		c := core.MustFromContext(ctx)
		p := c.GetFeature(policy.ManagerType()).(policy.Manager)
		h.timeout = p.ForLevel(config.GetUserLevel()).Timeouts.ConnectionIdle
	}
	return h, nil
}

// Process implements OutboundHandler.Dispatch().
func (h *Handler) Process(ctx context.Context, link *transport.Link, _ internet.Dialer) error {
	nBytes := h.response.WriteTo(link.Writer)
	if !h.keepConnection {
		if nBytes > 0 {
			// Sleep a little here to make sure the response is sent to client.
			time.Sleep(time.Second)
		}
		common.Interrupt(link.Writer)
	} else {
		ctx, cancel := context.WithTimeout(ctx, h.timeout)
		task.Run(ctx, func() error {
			defer cancel()
			return buf.Copy(link.Reader, buf.Discard)
		})
	}
	return nil
}

func init() {
	common.Must(common.RegisterConfig((*Config)(nil), func(ctx context.Context, config interface{}) (interface{}, error) {
		return New(ctx, config.(*Config))
	}))
}
