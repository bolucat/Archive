//go:build darwin && cgo

package tls

/*
#cgo CFLAGS: -x objective-c -fobjc-arc
#cgo LDFLAGS: -framework Foundation -framework Network -framework Security

#include <Foundation/Foundation.h>
#include <Network/Network.h>
#include <Security/Security.h>
#include <Security/SecProtocolMetadata.h>
#include <Security/SecProtocolOptions.h>
#include <Security/SecProtocolTypes.h>
#include <arpa/inet.h>
#include <dlfcn.h>
#include <dispatch/dispatch.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef nw_connection_t _Nullable (*box_nw_connection_create_with_connected_socket_and_parameters_f)(int connected_socket, nw_parameters_t parameters);
typedef const char * _Nullable (*box_sec_protocol_metadata_string_accessor_f)(sec_protocol_metadata_t metadata);

typedef struct box_apple_tls_client {
	void *connection;
	void *ready_semaphore;
	atomic_bool ready;
	atomic_bool ready_done;
	char *ready_error;
} box_apple_tls_client_t;

typedef struct box_apple_tls_state {
	uint16_t version;
	uint16_t cipher_suite;
	char *alpn;
	char *server_name;
	uint8_t *peer_cert_chain;
	size_t peer_cert_chain_len;
} box_apple_tls_state_t;

static dispatch_queue_t box_apple_tls_queue(void) {
	static dispatch_queue_t queue;
	static dispatch_once_t onceToken;
	dispatch_once(&onceToken, ^{
		queue = dispatch_queue_create("sing-box.apple-private-tls", DISPATCH_QUEUE_CONCURRENT);
	});
	return queue;
}

static nw_connection_t box_apple_tls_connection(box_apple_tls_client_t *client) {
	if (client == NULL || client->connection == NULL) {
		return nil;
	}
	return (__bridge nw_connection_t)client->connection;
}

static dispatch_semaphore_t box_apple_tls_ready_semaphore(box_apple_tls_client_t *client) {
	if (client == NULL || client->ready_semaphore == NULL) {
		return nil;
	}
	return (__bridge dispatch_semaphore_t)client->ready_semaphore;
}

static void box_set_error_string(char **error_out, NSString *message) {
	if (error_out == NULL || *error_out != NULL) {
		return;
	}
	if (message == nil) {
		*error_out = strdup("unknown error");
		return;
	}
	const char *utf8 = [message UTF8String];
	*error_out = strdup(utf8 != NULL ? utf8 : "unknown error");
}

static void box_set_error_message(char **error_out, const char *message) {
	if (error_out == NULL || *error_out != NULL) {
		return;
	}
	*error_out = strdup(message != NULL ? message : "unknown error");
}

static void box_set_error_from_nw_error(char **error_out, nw_error_t error) {
	if (error == NULL) {
		box_set_error_message(error_out, "unknown network error");
		return;
	}
	CFErrorRef cfError = nw_error_copy_cf_error(error);
	if (cfError == NULL) {
		box_set_error_message(error_out, "unknown network error");
		return;
	}
	NSString *description = [(__bridge NSError *)cfError description];
	box_set_error_string(error_out, description);
	CFRelease(cfError);
}

static char *box_apple_tls_metadata_copy_negotiated_protocol(sec_protocol_metadata_t metadata) {
	static box_sec_protocol_metadata_string_accessor_f copy_fn;
	static box_sec_protocol_metadata_string_accessor_f get_fn;
	static dispatch_once_t onceToken;
	dispatch_once(&onceToken, ^{
		copy_fn = (box_sec_protocol_metadata_string_accessor_f)dlsym(RTLD_DEFAULT, "sec_protocol_metadata_copy_negotiated_protocol");
		get_fn = (box_sec_protocol_metadata_string_accessor_f)dlsym(RTLD_DEFAULT, "sec_protocol_metadata_get_negotiated_protocol");
	});
	if (copy_fn != NULL) {
		return (char *)copy_fn(metadata);
	}
	if (get_fn != NULL) {
		const char *protocol = get_fn(metadata);
		if (protocol != NULL) {
			return strdup(protocol);
		}
	}
	return NULL;
}

static char *box_apple_tls_metadata_copy_server_name(sec_protocol_metadata_t metadata) {
	static box_sec_protocol_metadata_string_accessor_f copy_fn;
	static box_sec_protocol_metadata_string_accessor_f get_fn;
	static dispatch_once_t onceToken;
	dispatch_once(&onceToken, ^{
		copy_fn = (box_sec_protocol_metadata_string_accessor_f)dlsym(RTLD_DEFAULT, "sec_protocol_metadata_copy_server_name");
		get_fn = (box_sec_protocol_metadata_string_accessor_f)dlsym(RTLD_DEFAULT, "sec_protocol_metadata_get_server_name");
	});
	if (copy_fn != NULL) {
		return (char *)copy_fn(metadata);
	}
	if (get_fn != NULL) {
		const char *server_name = get_fn(metadata);
		if (server_name != NULL) {
			return strdup(server_name);
		}
	}
	return NULL;
}

static NSArray<NSString *> *box_split_lines(const char *content, size_t content_len) {
	if (content == NULL || content_len == 0) {
		return @[];
	}
	NSString *string = [[NSString alloc] initWithBytes:content length:content_len encoding:NSUTF8StringEncoding];
	if (string == nil) {
		return @[];
	}
	NSMutableArray<NSString *> *lines = [NSMutableArray array];
	[string enumerateLinesUsingBlock:^(NSString *line, BOOL *stop) {
		if (line.length > 0) {
			[lines addObject:line];
		}
	}];
	return lines;
}

static NSArray *box_parse_certificates_from_pem(const char *pem, size_t pem_len) {
	if (pem == NULL || pem_len == 0) {
		return @[];
	}
	NSString *content = [[NSString alloc] initWithBytes:pem length:pem_len encoding:NSUTF8StringEncoding];
	if (content == nil) {
		return @[];
	}
	NSString *beginMarker = @"-----BEGIN CERTIFICATE-----";
	NSString *endMarker = @"-----END CERTIFICATE-----";
	NSMutableArray *certificates = [NSMutableArray array];
	NSUInteger searchFrom = 0;
	while (searchFrom < content.length) {
		NSRange beginRange = [content rangeOfString:beginMarker options:0 range:NSMakeRange(searchFrom, content.length - searchFrom)];
		if (beginRange.location == NSNotFound) {
			break;
		}
		NSUInteger bodyStart = beginRange.location + beginRange.length;
		NSRange endRange = [content rangeOfString:endMarker options:0 range:NSMakeRange(bodyStart, content.length - bodyStart)];
		if (endRange.location == NSNotFound) {
			break;
		}
		NSString *base64Section = [content substringWithRange:NSMakeRange(bodyStart, endRange.location - bodyStart)];
		NSArray<NSString *> *components = [base64Section componentsSeparatedByCharactersInSet:[NSCharacterSet whitespaceAndNewlineCharacterSet]];
		NSString *base64Content = [components componentsJoinedByString:@""];
		NSData *der = [[NSData alloc] initWithBase64EncodedString:base64Content options:0];
		if (der != nil) {
			SecCertificateRef certificate = SecCertificateCreateWithData(NULL, (__bridge CFDataRef)der);
			if (certificate != NULL) {
				[certificates addObject:(__bridge id)certificate];
				CFRelease(certificate);
			}
		}
		searchFrom = endRange.location + endRange.length;
	}
	return certificates;
}

static bool box_evaluate_trust(sec_trust_t trust, NSArray *anchors, bool anchor_only) {
	bool result = false;
	SecTrustRef trustRef = sec_trust_copy_ref(trust);
	if (trustRef == NULL) {
		return false;
	}
	if (anchors.count > 0 || anchor_only) {
		CFMutableArrayRef anchorArray = CFArrayCreateMutable(NULL, 0, &kCFTypeArrayCallBacks);
		for (id certificate in anchors) {
			CFArrayAppendValue(anchorArray, (__bridge const void *)certificate);
		}
		SecTrustSetAnchorCertificates(trustRef, anchorArray);
		SecTrustSetAnchorCertificatesOnly(trustRef, anchor_only);
		CFRelease(anchorArray);
	}
	CFErrorRef error = NULL;
	result = SecTrustEvaluateWithError(trustRef, &error);
	if (error != NULL) {
		CFRelease(error);
	}
	CFRelease(trustRef);
	return result;
}

static nw_connection_t box_apple_tls_create_connection(int connected_socket, nw_parameters_t parameters) {
	static box_nw_connection_create_with_connected_socket_and_parameters_f create_fn;
	static dispatch_once_t onceToken;
	dispatch_once(&onceToken, ^{
		char name[] = "sretemarap_dna_tekcos_detcennoc_htiw_etaerc_noitcennoc_wn";
		for (size_t i = 0, j = sizeof(name) - 2; i < j; i++, j--) {
			char t = name[i]; name[i] = name[j]; name[j] = t;
		}
		create_fn = (box_nw_connection_create_with_connected_socket_and_parameters_f)dlsym(RTLD_DEFAULT, name);
	});
	if (create_fn == NULL) {
		return nil;
	}
	return create_fn(connected_socket, parameters);
}

static box_apple_tls_client_t *box_apple_tls_client_create(
	int connected_socket,
	const char *server_name,
	const char *alpn,
	size_t alpn_len,
	uint16_t min_version,
	uint16_t max_version,
	bool insecure,
	const char *anchor_pem,
	size_t anchor_pem_len,
	bool anchor_only,
	char **error_out
) {
	NSArray<NSString *> *alpnList = box_split_lines(alpn, alpn_len);
	NSArray *anchors = box_parse_certificates_from_pem(anchor_pem, anchor_pem_len);
	nw_parameters_t parameters = nw_parameters_create_secure_tcp(^(nw_protocol_options_t tls_options) {
		sec_protocol_options_t sec_options = nw_tls_copy_sec_protocol_options(tls_options);
		if (min_version != 0) {
			sec_protocol_options_set_min_tls_protocol_version(sec_options, (tls_protocol_version_t)min_version);
		}
		if (max_version != 0) {
			sec_protocol_options_set_max_tls_protocol_version(sec_options, (tls_protocol_version_t)max_version);
		}
		if (server_name != NULL && server_name[0] != '\0') {
			sec_protocol_options_set_tls_server_name(sec_options, server_name);
		}
		for (NSString *protocol in alpnList) {
			sec_protocol_options_add_tls_application_protocol(sec_options, protocol.UTF8String);
		}
		sec_protocol_options_set_peer_authentication_required(sec_options, !insecure);
		if (insecure) {
			sec_protocol_options_set_verify_block(sec_options, ^(sec_protocol_metadata_t metadata, sec_trust_t trust, sec_protocol_verify_complete_t complete) {
				complete(true);
			}, box_apple_tls_queue());
		} else if (anchors.count > 0 || anchor_only) {
			sec_protocol_options_set_verify_block(sec_options, ^(sec_protocol_metadata_t metadata, sec_trust_t trust, sec_protocol_verify_complete_t complete) {
				complete(box_evaluate_trust(trust, anchors, anchor_only));
			}, box_apple_tls_queue());
		}
	}, NW_PARAMETERS_DEFAULT_CONFIGURATION);

	nw_connection_t connection = box_apple_tls_create_connection(connected_socket, parameters);
	if (connection == NULL) {
		close(connected_socket);
		box_set_error_message(error_out, "apple TLS: failed to create connection");
		return NULL;
	}

	box_apple_tls_client_t *client = calloc(1, sizeof(box_apple_tls_client_t));
	client->connection = (__bridge_retained void *)connection;
	client->ready_semaphore = (__bridge_retained void *)dispatch_semaphore_create(0);
	atomic_init(&client->ready, false);
	atomic_init(&client->ready_done, false);

	nw_connection_set_state_changed_handler(connection, ^(nw_connection_state_t state, nw_error_t error) {
		if (atomic_load(&client->ready_done)) {
			return;
		}
		switch (state) {
		case nw_connection_state_ready:
			atomic_store(&client->ready, true);
			atomic_store(&client->ready_done, true);
			dispatch_semaphore_signal(box_apple_tls_ready_semaphore(client));
			break;
		case nw_connection_state_failed:
		case nw_connection_state_cancelled:
			box_set_error_from_nw_error(&client->ready_error, error);
			atomic_store(&client->ready_done, true);
			dispatch_semaphore_signal(box_apple_tls_ready_semaphore(client));
			break;
		default:
			break;
		}
	});
	nw_connection_set_queue(connection, box_apple_tls_queue());
	nw_connection_start(connection);
	return client;
}

static int box_apple_tls_client_wait_ready(box_apple_tls_client_t *client, int timeout_msec, char **error_out) {
	dispatch_semaphore_t ready_semaphore = box_apple_tls_ready_semaphore(client);
	if (ready_semaphore == nil) {
		box_set_error_message(error_out, "apple TLS: invalid client");
		return 0;
	}
	if (!atomic_load(&client->ready_done)) {
		dispatch_time_t timeout = DISPATCH_TIME_FOREVER;
		if (timeout_msec >= 0) {
			timeout = dispatch_time(DISPATCH_TIME_NOW, (int64_t)timeout_msec * NSEC_PER_MSEC);
		}
		long wait_result = dispatch_semaphore_wait(ready_semaphore, timeout);
		if (wait_result != 0) {
			return -2;
		}
	}
	if (atomic_load(&client->ready)) {
		return 1;
	}
	if (client->ready_error != NULL) {
		if (error_out != NULL) {
			*error_out = client->ready_error;
			client->ready_error = NULL;
		} else {
			free(client->ready_error);
			client->ready_error = NULL;
		}
	} else {
		box_set_error_message(error_out, "apple TLS: handshake failed");
	}
	return 0;
}

static void box_apple_tls_client_cancel(box_apple_tls_client_t *client) {
	if (client == NULL) {
		return;
	}
	nw_connection_t connection = box_apple_tls_connection(client);
	if (connection != nil) {
		nw_connection_cancel(connection);
	}
}

static void box_apple_tls_client_free(box_apple_tls_client_t *client) {
	if (client == NULL) {
		return;
	}
	free(client->ready_error);
	if (client->ready_semaphore != NULL) {
		CFBridgingRelease(client->ready_semaphore);
	}
	if (client->connection != NULL) {
		CFBridgingRelease(client->connection);
	}
	free(client);
}

static ssize_t box_apple_tls_client_read(box_apple_tls_client_t *client, void *buffer, size_t buffer_len, bool *eof_out, char **error_out) {
	nw_connection_t connection = box_apple_tls_connection(client);
	if (connection == nil) {
		box_set_error_message(error_out, "apple TLS: invalid client");
		return -1;
	}

	dispatch_semaphore_t read_semaphore = dispatch_semaphore_create(0);
	__block NSData *content_data = nil;
	__block bool read_eof = false;
	__block char *local_error = NULL;

	nw_connection_receive(connection, 1, (uint32_t)buffer_len, ^(dispatch_data_t content, nw_content_context_t context, bool is_complete, nw_error_t error) {
		if (content != NULL) {
			const void *mapped = NULL;
			size_t mapped_len = 0;
			dispatch_data_t mapped_data = dispatch_data_create_map(content, &mapped, &mapped_len);
			if (mapped != NULL && mapped_len > 0) {
				content_data = [NSData dataWithBytes:mapped length:mapped_len];
			}
			(void)mapped_data;
		}
		if (error != NULL && content_data.length == 0) {
			box_set_error_from_nw_error(&local_error, error);
		}
		if (is_complete && (context == NULL || nw_content_context_get_is_final(context))) {
			read_eof = true;
		}
		dispatch_semaphore_signal(read_semaphore);
	});

	dispatch_semaphore_wait(read_semaphore, DISPATCH_TIME_FOREVER);
	if (local_error != NULL) {
		if (error_out != NULL) {
			*error_out = local_error;
		} else {
			free(local_error);
		}
		return -1;
	}
	if (eof_out != NULL) {
		*eof_out = read_eof;
	}
	if (content_data == nil || content_data.length == 0) {
		return 0;
	}
	memcpy(buffer, content_data.bytes, content_data.length);
	return (ssize_t)content_data.length;
}

static ssize_t box_apple_tls_client_write(box_apple_tls_client_t *client, const void *buffer, size_t buffer_len, char **error_out) {
	nw_connection_t connection = box_apple_tls_connection(client);
	if (connection == nil) {
		box_set_error_message(error_out, "apple TLS: invalid client");
		return -1;
	}
	if (buffer_len == 0) {
		return 0;
	}

	void *content_copy = malloc(buffer_len);
	memcpy(content_copy, buffer, buffer_len);
	dispatch_data_t content = dispatch_data_create(content_copy, buffer_len, box_apple_tls_queue(), ^{
		free(content_copy);
	});

	dispatch_semaphore_t write_semaphore = dispatch_semaphore_create(0);
	__block char *local_error = NULL;

	nw_connection_send(connection, content, NW_CONNECTION_DEFAULT_STREAM_CONTEXT, false, ^(nw_error_t error) {
		if (error != NULL) {
			box_set_error_from_nw_error(&local_error, error);
		}
		dispatch_semaphore_signal(write_semaphore);
	});

	dispatch_semaphore_wait(write_semaphore, DISPATCH_TIME_FOREVER);
	if (local_error != NULL) {
		if (error_out != NULL) {
			*error_out = local_error;
		} else {
			free(local_error);
		}
		return -1;
	}
	return (ssize_t)buffer_len;
}

static bool box_apple_tls_client_copy_state(box_apple_tls_client_t *client, box_apple_tls_state_t *state, char **error_out) {
	nw_connection_t connection = box_apple_tls_connection(client);
	if (connection == nil) {
		box_set_error_message(error_out, "apple TLS: invalid client");
		return false;
	}
	memset(state, 0, sizeof(box_apple_tls_state_t));

	nw_protocol_definition_t tls_definition = nw_protocol_copy_tls_definition();
	nw_protocol_metadata_t metadata = nw_connection_copy_protocol_metadata(connection, tls_definition);
	if (metadata == NULL || !nw_protocol_metadata_is_tls(metadata)) {
		box_set_error_message(error_out, "apple TLS: metadata unavailable");
		return false;
	}

	sec_protocol_metadata_t sec_metadata = nw_tls_copy_sec_protocol_metadata(metadata);
	state->version = (uint16_t)sec_protocol_metadata_get_negotiated_tls_protocol_version(sec_metadata);
	state->cipher_suite = (uint16_t)sec_protocol_metadata_get_negotiated_tls_ciphersuite(sec_metadata);

	state->alpn = box_apple_tls_metadata_copy_negotiated_protocol(sec_metadata);
	state->server_name = box_apple_tls_metadata_copy_server_name(sec_metadata);

	NSMutableData *chain_data = [NSMutableData data];
	sec_protocol_metadata_access_peer_certificate_chain(sec_metadata, ^(sec_certificate_t certificate) {
		SecCertificateRef certificate_ref = sec_certificate_copy_ref(certificate);
		if (certificate_ref == NULL) {
			return;
		}
		CFDataRef certificate_data = SecCertificateCopyData(certificate_ref);
		CFRelease(certificate_ref);
		if (certificate_data == NULL) {
			return;
		}
		uint32_t certificate_len = (uint32_t)CFDataGetLength(certificate_data);
		uint32_t network_len = htonl(certificate_len);
		[chain_data appendBytes:&network_len length:sizeof(network_len)];
		[chain_data appendBytes:CFDataGetBytePtr(certificate_data) length:certificate_len];
		CFRelease(certificate_data);
	});
	if (chain_data.length > 0) {
		state->peer_cert_chain = malloc(chain_data.length);
		memcpy(state->peer_cert_chain, chain_data.bytes, chain_data.length);
		state->peer_cert_chain_len = chain_data.length;
	}

	return true;
}

static void box_apple_tls_state_free(box_apple_tls_state_t *state) {
	if (state == NULL) {
		return;
	}
	free(state->alpn);
	free(state->server_name);
	free(state->peer_cert_chain);
}

*/
import "C"

import (
	"context"
	"crypto/tls"
	"crypto/x509"
	"encoding/binary"
	"io"
	"math"
	"net"
	"os"
	"strings"
	"sync"
	"syscall"
	"time"
	"unsafe"

	"github.com/sagernet/sing-box/adapter"
	boxConstant "github.com/sagernet/sing-box/constant"
	"github.com/sagernet/sing-box/option"
	"github.com/sagernet/sing/common"
	E "github.com/sagernet/sing/common/exceptions"
	"github.com/sagernet/sing/common/logger"
	"github.com/sagernet/sing/service"

	"golang.org/x/sys/unix"
)

type appleCertificateStore interface {
	StoreKind() string
	CurrentPEM() []string
}

type appleClientConfig struct {
	serverName                 string
	nextProtos                 []string
	handshakeTimeout           time.Duration
	minVersion                 uint16
	maxVersion                 uint16
	insecure                   bool
	anchorPEM                  string
	anchorOnly                 bool
	certificatePublicKeySHA256 [][]byte
}

func (c *appleClientConfig) ServerName() string {
	return c.serverName
}

func (c *appleClientConfig) SetServerName(serverName string) {
	c.serverName = serverName
}

func (c *appleClientConfig) NextProtos() []string {
	return c.nextProtos
}

func (c *appleClientConfig) SetNextProtos(nextProto []string) {
	c.nextProtos = append(c.nextProtos[:0], nextProto...)
}

func (c *appleClientConfig) HandshakeTimeout() time.Duration {
	return c.handshakeTimeout
}

func (c *appleClientConfig) SetHandshakeTimeout(timeout time.Duration) {
	c.handshakeTimeout = timeout
}

func (c *appleClientConfig) STDConfig() (*STDConfig, error) {
	return nil, E.New("unsupported usage for Apple TLS engine")
}

func (c *appleClientConfig) Client(conn net.Conn) (Conn, error) {
	return nil, os.ErrInvalid
}

func (c *appleClientConfig) Clone() Config {
	return &appleClientConfig{
		serverName:                 c.serverName,
		nextProtos:                 append([]string(nil), c.nextProtos...),
		handshakeTimeout:           c.handshakeTimeout,
		minVersion:                 c.minVersion,
		maxVersion:                 c.maxVersion,
		insecure:                   c.insecure,
		anchorPEM:                  c.anchorPEM,
		anchorOnly:                 c.anchorOnly,
		certificatePublicKeySHA256: append([][]byte(nil), c.certificatePublicKeySHA256...),
	}
}

func (c *appleClientConfig) ClientHandshake(ctx context.Context, conn net.Conn) (Conn, error) {
	rawSyscallConn, ok := common.Cast[syscall.Conn](conn)
	if !ok {
		return nil, E.New("apple TLS: requires fd-backed TCP connection")
	}
	syscallConn, err := rawSyscallConn.SyscallConn()
	if err != nil {
		return nil, E.Cause(err, "access raw connection")
	}

	var dupFD int
	controlErr := syscallConn.Control(func(fd uintptr) {
		dupFD, err = unix.Dup(int(fd))
	})
	if controlErr != nil {
		return nil, E.Cause(controlErr, "access raw connection")
	}
	if err != nil {
		return nil, E.Cause(err, "duplicate raw connection")
	}

	serverName := c.serverName
	serverNamePtr := cStringOrNil(serverName)
	defer cFree(serverNamePtr)

	alpn := strings.Join(c.nextProtos, "\n")
	alpnPtr := cStringOrNil(alpn)
	defer cFree(alpnPtr)

	anchorPEMPtr := cStringOrNil(c.anchorPEM)
	defer cFree(anchorPEMPtr)

	var errorPtr *C.char
	client := C.box_apple_tls_client_create(
		C.int(dupFD),
		serverNamePtr,
		alpnPtr,
		C.size_t(len(alpn)),
		C.uint16_t(c.minVersion),
		C.uint16_t(c.maxVersion),
		C.bool(c.insecure),
		anchorPEMPtr,
		C.size_t(len(c.anchorPEM)),
		C.bool(c.anchorOnly),
		&errorPtr,
	)
	if client == nil {
		if errorPtr != nil {
			defer C.free(unsafe.Pointer(errorPtr))
			return nil, E.New(C.GoString(errorPtr))
		}
		return nil, E.New("apple TLS: create connection")
	}
	if err = waitAppleTLSClientReady(ctx, client); err != nil {
		C.box_apple_tls_client_cancel(client)
		C.box_apple_tls_client_free(client)
		return nil, err
	}

	var state C.box_apple_tls_state_t
	stateOK := C.box_apple_tls_client_copy_state(client, &state, &errorPtr)
	if !bool(stateOK) {
		C.box_apple_tls_client_cancel(client)
		C.box_apple_tls_client_free(client)
		if errorPtr != nil {
			defer C.free(unsafe.Pointer(errorPtr))
			return nil, E.New(C.GoString(errorPtr))
		}
		return nil, E.New("apple TLS: read metadata")
	}
	defer C.box_apple_tls_state_free(&state)

	connectionState, rawCerts, err := parseAppleTLSState(&state)
	if err != nil {
		C.box_apple_tls_client_cancel(client)
		C.box_apple_tls_client_free(client)
		return nil, err
	}
	if len(c.certificatePublicKeySHA256) > 0 {
		err = verifyPublicKeySHA256(c.certificatePublicKeySHA256, rawCerts, nil)
		if err != nil {
			C.box_apple_tls_client_cancel(client)
			C.box_apple_tls_client_free(client)
			return nil, err
		}
	}

	return &appleTLSConn{
		rawConn: conn,
		client:  client,
		state:   connectionState,
		closed:  make(chan struct{}),
	}, nil
}

const appleTLSHandshakePollInterval = 100 * time.Millisecond

func waitAppleTLSClientReady(ctx context.Context, client *C.box_apple_tls_client_t) error {
	for {
		if err := ctx.Err(); err != nil {
			C.box_apple_tls_client_cancel(client)
			return err
		}

		waitTimeout := appleTLSHandshakePollInterval
		if deadline, loaded := ctx.Deadline(); loaded {
			remaining := time.Until(deadline)
			if remaining <= 0 {
				C.box_apple_tls_client_cancel(client)
				if err := ctx.Err(); err != nil {
					return err
				}
				return context.DeadlineExceeded
			}
			if remaining < waitTimeout {
				waitTimeout = remaining
			}
		}

		var errorPtr *C.char
		waitResult := C.box_apple_tls_client_wait_ready(client, C.int(timeoutFromDuration(waitTimeout)), &errorPtr)
		switch waitResult {
		case 1:
			return nil
		case -2:
			continue
		case 0:
			if errorPtr != nil {
				defer C.free(unsafe.Pointer(errorPtr))
				return E.New(C.GoString(errorPtr))
			}
			return E.New("apple TLS: handshake failed")
		default:
			return E.New("apple TLS: invalid handshake state")
		}
	}
}

type appleTLSConn struct {
	rawConn net.Conn
	client  *C.box_apple_tls_client_t
	state   tls.ConnectionState

	readAccess  sync.Mutex
	writeAccess sync.Mutex
	stateAccess sync.RWMutex
	closeOnce   sync.Once
	ioAccess    sync.Mutex
	ioGroup     sync.WaitGroup
	closed      chan struct{}
	readEOF     bool
}

func (c *appleTLSConn) Read(p []byte) (int, error) {
	c.readAccess.Lock()
	defer c.readAccess.Unlock()
	if c.readEOF {
		return 0, io.EOF
	}
	if len(p) == 0 {
		return 0, nil
	}

	client, err := c.acquireClient()
	if err != nil {
		return 0, err
	}
	defer c.releaseClient()

	var eof C.bool
	var errorPtr *C.char
	n := C.box_apple_tls_client_read(client, unsafe.Pointer(&p[0]), C.size_t(len(p)), &eof, &errorPtr)
	switch {
	case n >= 0:
		if bool(eof) {
			c.readEOF = true
			if n == 0 {
				return 0, io.EOF
			}
		}
		return int(n), nil
	default:
		if errorPtr != nil {
			defer C.free(unsafe.Pointer(errorPtr))
			if c.isClosed() {
				return 0, net.ErrClosed
			}
			return 0, E.New(C.GoString(errorPtr))
		}
		return 0, net.ErrClosed
	}
}

func (c *appleTLSConn) Write(p []byte) (int, error) {
	c.writeAccess.Lock()
	defer c.writeAccess.Unlock()
	if len(p) == 0 {
		return 0, nil
	}

	client, err := c.acquireClient()
	if err != nil {
		return 0, err
	}
	defer c.releaseClient()

	var errorPtr *C.char
	n := C.box_apple_tls_client_write(client, unsafe.Pointer(&p[0]), C.size_t(len(p)), &errorPtr)
	if n >= 0 {
		return int(n), nil
	}
	if errorPtr != nil {
		defer C.free(unsafe.Pointer(errorPtr))
		if c.isClosed() {
			return 0, net.ErrClosed
		}
		return 0, E.New(C.GoString(errorPtr))
	}
	return 0, net.ErrClosed
}

func (c *appleTLSConn) Close() error {
	var closeErr error
	c.closeOnce.Do(func() {
		close(c.closed)
		C.box_apple_tls_client_cancel(c.client)
		closeErr = c.rawConn.Close()
		c.ioAccess.Lock()
		c.ioGroup.Wait()
		C.box_apple_tls_client_free(c.client)
		c.client = nil
		c.ioAccess.Unlock()
	})
	return closeErr
}

func (c *appleTLSConn) LocalAddr() net.Addr {
	return c.rawConn.LocalAddr()
}

func (c *appleTLSConn) RemoteAddr() net.Addr {
	return c.rawConn.RemoteAddr()
}

func (c *appleTLSConn) SetDeadline(t time.Time) error {
	return os.ErrInvalid
}

func (c *appleTLSConn) SetReadDeadline(t time.Time) error {
	return os.ErrInvalid
}

func (c *appleTLSConn) SetWriteDeadline(t time.Time) error {
	return os.ErrInvalid
}

func (c *appleTLSConn) NeedAdditionalReadDeadline() bool {
	return true
}

func (c *appleTLSConn) isClosed() bool {
	select {
	case <-c.closed:
		return true
	default:
		return false
	}
}

func (c *appleTLSConn) acquireClient() (*C.box_apple_tls_client_t, error) {
	c.ioAccess.Lock()
	defer c.ioAccess.Unlock()
	if c.isClosed() {
		return nil, net.ErrClosed
	}
	client := c.client
	if client == nil {
		return nil, net.ErrClosed
	}
	c.ioGroup.Add(1)
	return client, nil
}

func (c *appleTLSConn) releaseClient() {
	c.ioGroup.Done()
}

func (c *appleTLSConn) NetConn() net.Conn {
	return c.rawConn
}

func (c *appleTLSConn) HandshakeContext(ctx context.Context) error {
	return nil
}

func (c *appleTLSConn) ConnectionState() ConnectionState {
	c.stateAccess.RLock()
	defer c.stateAccess.RUnlock()
	return c.state
}

func newAppleClient(ctx context.Context, logger logger.ContextLogger, serverAddress string, options option.OutboundTLSOptions, allowEmptyServerName bool) (Config, error) {
	if options.Reality != nil && options.Reality.Enabled {
		return nil, E.New("reality is unsupported in Apple TLS engine")
	}
	if options.UTLS != nil && options.UTLS.Enabled {
		return nil, E.New("utls is unsupported in Apple TLS engine")
	}
	if options.ECH != nil && options.ECH.Enabled {
		return nil, E.New("ech is unsupported in Apple TLS engine")
	}
	if options.DisableSNI {
		return nil, E.New("disable_sni is unsupported in Apple TLS engine")
	}
	if len(options.CipherSuites) > 0 {
		return nil, E.New("cipher_suites is unsupported in Apple TLS engine")
	}
	if len(options.CurvePreferences) > 0 {
		return nil, E.New("curve_preferences is unsupported in Apple TLS engine")
	}
	if len(options.ClientCertificate) > 0 || options.ClientCertificatePath != "" || len(options.ClientKey) > 0 || options.ClientKeyPath != "" {
		return nil, E.New("client certificate is unsupported in Apple TLS engine")
	}
	if options.Fragment || options.RecordFragment {
		return nil, E.New("tls fragment is unsupported in Apple TLS engine")
	}
	if options.KernelTx || options.KernelRx {
		return nil, E.New("ktls is unsupported in Apple TLS engine")
	}

	var serverName string
	if options.ServerName != "" {
		serverName = options.ServerName
	} else if serverAddress != "" {
		serverName = serverAddress
	}
	if serverName == "" && !options.Insecure && !allowEmptyServerName {
		return nil, errMissingServerName
	}
	if len(options.CertificatePublicKeySHA256) > 0 && (len(options.Certificate) > 0 || options.CertificatePath != "") {
		return nil, E.New("certificate_public_key_sha256 is conflict with certificate or certificate_path")
	}

	var handshakeTimeout time.Duration
	if options.HandshakeTimeout > 0 {
		handshakeTimeout = options.HandshakeTimeout.Build()
	} else {
		handshakeTimeout = boxConstant.TCPTimeout
	}

	var minVersion uint16
	if options.MinVersion != "" {
		var err error
		minVersion, err = ParseTLSVersion(options.MinVersion)
		if err != nil {
			return nil, E.Cause(err, "parse min_version")
		}
	}
	var maxVersion uint16
	if options.MaxVersion != "" {
		var err error
		maxVersion, err = ParseTLSVersion(options.MaxVersion)
		if err != nil {
			return nil, E.Cause(err, "parse max_version")
		}
	}

	anchorPEM, anchorOnly, err := appleAnchorPEM(ctx, options)
	if err != nil {
		return nil, err
	}
	return &appleClientConfig{
		serverName:                 serverName,
		nextProtos:                 append([]string(nil), options.ALPN...),
		handshakeTimeout:           handshakeTimeout,
		minVersion:                 minVersion,
		maxVersion:                 maxVersion,
		insecure:                   options.Insecure || len(options.CertificatePublicKeySHA256) > 0,
		anchorPEM:                  anchorPEM,
		anchorOnly:                 anchorOnly,
		certificatePublicKeySHA256: append([][]byte(nil), options.CertificatePublicKeySHA256...),
	}, nil
}

func appleAnchorPEM(ctx context.Context, options option.OutboundTLSOptions) (string, bool, error) {
	if len(options.Certificate) > 0 {
		return strings.Join(options.Certificate, "\n"), true, nil
	}
	if options.CertificatePath != "" {
		content, err := os.ReadFile(options.CertificatePath)
		if err != nil {
			return "", false, E.Cause(err, "read certificate")
		}
		return string(content), true, nil
	}

	certificateStore := service.FromContext[adapter.CertificateStore](ctx)
	if certificateStore == nil {
		return "", false, nil
	}
	store, ok := certificateStore.(appleCertificateStore)
	if !ok {
		return "", false, nil
	}

	switch store.StoreKind() {
	case boxConstant.CertificateStoreSystem, "":
		return strings.Join(store.CurrentPEM(), "\n"), false, nil
	case boxConstant.CertificateStoreMozilla, boxConstant.CertificateStoreChrome, boxConstant.CertificateStoreNone:
		return strings.Join(store.CurrentPEM(), "\n"), true, nil
	default:
		return "", false, E.New("unsupported certificate store for Apple TLS engine: ", store.StoreKind())
	}
}

func parseAppleTLSState(state *C.box_apple_tls_state_t) (tls.ConnectionState, [][]byte, error) {
	rawCerts, peerCertificates, err := parseAppleCertChain(state.peer_cert_chain, state.peer_cert_chain_len)
	if err != nil {
		return tls.ConnectionState{}, nil, err
	}
	var negotiatedProtocol string
	if state.alpn != nil {
		negotiatedProtocol = C.GoString(state.alpn)
	}
	var serverName string
	if state.server_name != nil {
		serverName = C.GoString(state.server_name)
	}
	return tls.ConnectionState{
		Version:            uint16(state.version),
		HandshakeComplete:  true,
		CipherSuite:        uint16(state.cipher_suite),
		NegotiatedProtocol: negotiatedProtocol,
		ServerName:         serverName,
		PeerCertificates:   peerCertificates,
	}, rawCerts, nil
}

func parseAppleCertChain(chain *C.uint8_t, chainLen C.size_t) ([][]byte, []*x509.Certificate, error) {
	if chain == nil || chainLen == 0 {
		return nil, nil, nil
	}
	chainBytes := C.GoBytes(unsafe.Pointer(chain), C.int(chainLen))
	var (
		rawCerts         [][]byte
		peerCertificates []*x509.Certificate
	)
	for len(chainBytes) >= 4 {
		certificateLen := binary.BigEndian.Uint32(chainBytes[:4])
		chainBytes = chainBytes[4:]
		if len(chainBytes) < int(certificateLen) {
			return nil, nil, E.New("apple TLS: invalid certificate chain")
		}
		certificateData := append([]byte(nil), chainBytes[:certificateLen]...)
		certificate, err := x509.ParseCertificate(certificateData)
		if err != nil {
			return nil, nil, E.Cause(err, "parse peer certificate")
		}
		rawCerts = append(rawCerts, certificateData)
		peerCertificates = append(peerCertificates, certificate)
		chainBytes = chainBytes[certificateLen:]
	}
	if len(chainBytes) != 0 {
		return nil, nil, E.New("apple TLS: invalid certificate chain")
	}
	return rawCerts, peerCertificates, nil
}

func timeoutFromDuration(timeout time.Duration) int {
	if timeout <= 0 {
		return 0
	}
	timeoutMilliseconds := int64(timeout / time.Millisecond)
	if timeout%time.Millisecond != 0 {
		timeoutMilliseconds++
	}
	if timeoutMilliseconds > math.MaxInt32 {
		return math.MaxInt32
	}
	return int(timeoutMilliseconds)
}

func cStringOrNil(value string) *C.char {
	if value == "" {
		return nil
	}
	return C.CString(value)
}

func cFree(pointer *C.char) {
	if pointer != nil {
		C.free(unsafe.Pointer(pointer))
	}
}
