package net

import (
	"context"
	"net"

	"github.com/metacubex/mihomo/common/contextutils"
)

// SetupContextForConn is a helper function that starts connection I/O interrupter goroutine.
func SetupContextForConn(ctx context.Context, conn net.Conn) (done func(*error)) {
	stopc := make(chan struct{})
	stop := contextutils.AfterFunc(ctx, func() {
		// Close the connection, discarding the error
		_ = conn.Close()
		close(stopc)
	})
	return func(inputErr *error) {
		if !stop() {
			// The AfterFunc was started, wait for it to complete.
			<-stopc
			if ctxErr := ctx.Err(); ctxErr != nil && inputErr != nil {
				// Return context error to user.
				inputErr = &ctxErr
			}
		}
	}
}
