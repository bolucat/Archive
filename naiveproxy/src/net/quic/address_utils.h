// Copyright 2019 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#ifndef NET_QUIC_ADDRESS_UTILS_H_
#define NET_QUIC_ADDRESS_UTILS_H_

#include "base/containers/span.h"
#include "net/base/ip_address.h"
#include "net/base/ip_address_util.h"
#include "net/base/ip_endpoint.h"
#include "net/third_party/quiche/src/quiche/common/quiche_ip_address.h"
#include "net/third_party/quiche/src/quiche/common/quiche_ip_address_family.h"
#include "net/third_party/quiche/src/quiche/quic/platform/api/quic_socket_address.h"

namespace net {

inline IPEndPoint ToIPEndPoint(quic::QuicSocketAddress address) {
  if (!address.IsInitialized()) {
    return IPEndPoint();
  }

  IPEndPoint result;
  sockaddr_storage storage = address.generic_address();
  const bool success = result.FromSockAddr(
      reinterpret_cast<const sockaddr*>(&storage), sizeof(storage));
  DCHECK(success);
  return result;
}

inline IPAddress ToIPAddress(quiche::QuicheIpAddress address) {
  if (!address.IsInitialized()) {
    return IPAddress();
  }

  switch (address.address_family()) {
    case quiche::IpAddressFamily::IP_V4: {
      in_addr raw_address = address.GetIPv4();
      // `s_addr` is a `uint32_t`, but it is already in network byte order.
      return IPAddress(base::byte_span_from_ref(raw_address.s_addr));
    }
    case quiche::IpAddressFamily::IP_V6: {
      in6_addr raw_address = address.GetIPv6();
      return IPAddress(raw_address.s6_addr);
    }
    default:
      DCHECK_EQ(address.address_family(), quiche::IpAddressFamily::IP_UNSPEC);
      return IPAddress();
  }
}

inline quic::QuicSocketAddress ToQuicSocketAddress(IPEndPoint address) {
  if (address.address().empty()) {
    return quic::QuicSocketAddress();
  }

  sockaddr_storage result;
  socklen_t size = sizeof(result);
  if (!address.ToSockAddr(reinterpret_cast<sockaddr*>(&result), &size)) {
    return quic::QuicSocketAddress();
  }
  return quic::QuicSocketAddress(result);
}

inline quiche::QuicheIpAddress ToQuicheIpAddress(net::IPAddress address) {
  if (address.IsIPv4()) {
    return quiche::QuicheIpAddress(ToInAddr(address));
  }
  if (address.IsIPv6()) {
    return quiche::QuicheIpAddress(ToIn6Addr(address));
  }

  DCHECK(address.empty());
  return quiche::QuicheIpAddress();
}

}  // namespace net

#endif  // NET_QUIC_ADDRESS_UTILS_H_
