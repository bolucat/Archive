// Copyright 2018 The Chromium Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#include "net/socket/transport_client_socket.h"

namespace net {

TransportClientSocket::TransportClientSocket() = default;
TransportClientSocket::~TransportClientSocket() = default;

bool TransportClientSocket::SetNoDelay(bool no_delay) {
  NOTIMPLEMENTED();
  return false;
}

bool TransportClientSocket::SetKeepAlive(bool enable, int delay_secs) {
  NOTIMPLEMENTED();
  return false;
}

void TransportClientSocket::SetSocketCreatorForTesting(
    base::RepeatingCallback<std::unique_ptr<net::TransportClientSocket>(void)>
        socket_creator) {
  NOTIMPLEMENTED();
}

}  // namespace net
