use crate::protocol::common::addr::IpAddress;
use crate::protocol::common::atype::Atype;
use crate::protocol::common::command::Command;
use crate::protocol::common::request::{InboundRequest, TransportProtocol};

pub const VERSION: u8 = 5;

#[allow(dead_code)]
pub struct Request {
    version: u8,
    command: Command,
    rsv: u8,
    atype: Atype,
    addr: IpAddress,
    port: u16,
}

#[allow(dead_code)]
pub struct ClientHello {
    version: u8,
    method_size: u8,
    // Assume for now that the number of methods is always 1
    methods: u8,
}

#[allow(dead_code)]
pub struct ServerHello {
    version: u8,
    method: u8,
}

#[allow(dead_code)]
pub struct RequestAck {
    version: u8,
    rep: u8,
    rsv: u8,
    atype: u8,
    addr: IpAddress,
    port: u16,
}

impl ServerHello {
    pub fn new(method: u8) -> ServerHello {
        return ServerHello {
            version: VERSION,
            method,
        };
    }

    pub fn to_bytes(&self) -> [u8; 2] {
        return [self.version, self.method];
    }
}

impl RequestAck {
    pub fn new(rep: u8, rsv: u8, atype: u8, addr: IpAddress, port: u16) -> RequestAck {
        return RequestAck {
            version: VERSION,
            rep,
            rsv,
            atype,
            addr,
            port,
        };
    }
}

impl Request {
    pub fn new(
        version: u8,
        command: Command,
        rsv: u8,
        atype: Atype,
        port: u16,
        addr: IpAddress,
    ) -> Request {
        return Request {
            version,
            command,
            rsv,
            atype,
            port,
            addr,
        };
    }

    #[inline]
    pub fn dump_request(&self) -> String {
        return format!(
            "[{} => {}:{}]",
            self.command.to_string(),
            self.addr,
            self.port
        );
    }

    #[inline]
    pub fn inbound_request(self) -> InboundRequest {
        return match self.command {
            Command::Udp => InboundRequest::new(
                self.atype,
                self.addr,
                self.command,
                self.port,
                TransportProtocol::UDP,
                crate::proxy::base::SupportedProtocols::SOCKS,
            ),
            _ => InboundRequest::new(
                self.atype,
                self.addr,
                self.command,
                self.port,
                TransportProtocol::TCP,
                crate::proxy::base::SupportedProtocols::SOCKS,
            ),
        };
    }
}
