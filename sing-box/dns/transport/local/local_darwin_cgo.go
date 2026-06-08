//go:build darwin

package local

/*
#include <stdlib.h>
#include <netdb.h>
#include <dns.h>

static int cgo_dns_search(dns_handle_t handle, const char *name, int class, int type,
	unsigned char *answer, int anslen, int *out_h_errno) {
	struct sockaddr_storage from;
	uint32_t fromlen = sizeof(from);
	h_errno = 0;
	int n = dns_search(handle, name, class, type, (char *)answer, anslen,
		(struct sockaddr *)&from, &fromlen);
	*out_h_errno = h_errno;
	return n;
}
*/
import "C"

import (
	"context"
	"errors"
	"net"
	"sync"
	"unsafe"

	"github.com/sagernet/sing-box/dns"
	E "github.com/sagernet/sing/common/exceptions"

	mDNS "github.com/miekg/dns"
)

const (
	darwinResolverHostNotFound = 1
	darwinResolverTryAgain     = 2
	darwinResolverNoRecovery   = 3
	darwinResolverNoData       = 4
)

type darwinDNSHandle struct {
	handle     C.dns_handle_t
	generation uint64
}

// darwinSystemResolver pools libresolv handles. iOS 26.5.1 NULL-derefs in
// libresolv when resolver state is rebuilt by concurrent dns_open(NULL) calls,
// and dns_search corrupts state when two queries share one handle. Handles are
// therefore reused across queries and leased exclusively: each in-flight query
// owns one handle, dns_search runs concurrently on independent handles, while
// dns_open and dns_free are serialized by lifecycleAccess. A leased handle is
// removed from idle and owned by its caller, so Reset/Close only ever free idle
// handles and the caller frees its own handle on release — dns_free never races
// an in-flight dns_search and is never called twice for the same handle.
type darwinSystemResolver struct {
	access          sync.Mutex
	lifecycleAccess sync.Mutex
	idle            []*darwinDNSHandle
	generation      uint64
	closed          bool
}

func newSystemResolver() systemResolver {
	return &darwinSystemResolver{}
}

func (r *darwinSystemResolver) acquire() (*darwinDNSHandle, error) {
	r.access.Lock()
	if r.closed {
		r.access.Unlock()
		return nil, net.ErrClosed
	}
	if count := len(r.idle); count > 0 {
		handle := r.idle[count-1]
		r.idle[count-1] = nil
		r.idle = r.idle[:count-1]
		r.access.Unlock()
		return handle, nil
	}
	generation := r.generation
	r.access.Unlock()

	r.lifecycleAccess.Lock()
	cHandle := C.dns_open(nil)
	r.lifecycleAccess.Unlock()
	if cHandle == nil {
		return nil, dns.RcodeServerFailure
	}
	return &darwinDNSHandle{handle: cHandle, generation: generation}, nil
}

func (r *darwinSystemResolver) release(handle *darwinDNSHandle) {
	r.access.Lock()
	reuse := !r.closed && handle.generation == r.generation
	if reuse {
		r.idle = append(r.idle, handle)
	}
	r.access.Unlock()
	if !reuse {
		r.free(handle)
	}
}

func (r *darwinSystemResolver) free(handle *darwinDNSHandle) {
	r.lifecycleAccess.Lock()
	C.dns_free(handle.handle)
	r.lifecycleAccess.Unlock()
}

func (r *darwinSystemResolver) Reset() {
	r.access.Lock()
	if r.closed {
		r.access.Unlock()
		return
	}
	r.generation++
	idle := r.idle
	r.idle = nil
	r.access.Unlock()
	for _, handle := range idle {
		r.free(handle)
	}
}

func (r *darwinSystemResolver) Close() error {
	r.access.Lock()
	if r.closed {
		r.access.Unlock()
		return nil
	}
	r.closed = true
	idle := r.idle
	r.idle = nil
	r.access.Unlock()
	for _, handle := range idle {
		r.free(handle)
	}
	return nil
}

func (r *darwinSystemResolver) Exchange(ctx context.Context, message *mDNS.Msg) (*mDNS.Msg, error) {
	question := message.Question[0]
	type resolvResult struct {
		response *mDNS.Msg
		err      error
	}
	resultCh := make(chan resolvResult, 1)
	go func() {
		response, err := r.lookup(question.Name, int(question.Qclass), int(question.Qtype))
		resultCh <- resolvResult{response, err}
	}()
	var result resolvResult
	select {
	case <-ctx.Done():
		return nil, ctx.Err()
	case result = <-resultCh:
	}
	if result.err != nil {
		var rcodeError dns.RcodeError
		if errors.As(result.err, &rcodeError) {
			return dns.FixedResponseStatus(message, int(rcodeError)), nil
		}
		return nil, result.err
	}
	result.response.Id = message.Id
	// Workaround for a bug in Apple libresolv: res_query_mDNSResponder
	// (libresolv/res_query.c), used when the resolver has
	// DNS_FLAG_FORWARD_TO_MDNSRESPONDER set (typical inside a Network
	// Extension), writes:
	//
	//     ans->qr = 1;
	//     ans->qr = htons(ans->qr);
	//
	// HEADER.qr is a 1-bit bitfield (<arpa/nameser_compat.h>), so
	// htons(1) == 0x0100 gets truncated back to 0, clearing the QR bit.
	// Force it on so downstream clients see a valid response.
	result.response.Response = true
	return result.response, nil
}

func (r *darwinSystemResolver) lookup(name string, class, qtype int) (*mDNS.Msg, error) {
	handle, err := r.acquire()
	if err != nil {
		return nil, err
	}
	response, err := darwinSearch(handle.handle, name, class, qtype)
	r.release(handle)
	return response, err
}

func darwinSearch(handle C.dns_handle_t, name string, class, qtype int) (*mDNS.Msg, error) {
	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cName))

	answer := make([]byte, 4096)
	var hErrno C.int
	n := C.cgo_dns_search(handle, cName, C.int(class), C.int(qtype),
		(*C.uchar)(unsafe.Pointer(&answer[0])), C.int(len(answer)),
		&hErrno)
	if n <= 0 {
		return nil, darwinResolverHErrno(name, int(hErrno))
	}
	var response mDNS.Msg
	err := response.Unpack(answer[:int(n)])
	if err != nil {
		return nil, E.Cause(err, "unpack dns_search response")
	}
	return &response, nil
}

func darwinResolverHErrno(name string, hErrno int) error {
	switch hErrno {
	case darwinResolverHostNotFound:
		return dns.RcodeNameError
	case darwinResolverNoData:
		return dns.RcodeSuccess
	case darwinResolverTryAgain, darwinResolverNoRecovery:
		return dns.RcodeServerFailure
	default:
		return E.New("dns_search: unknown h_errno ", hErrno, " for ", name)
	}
}
