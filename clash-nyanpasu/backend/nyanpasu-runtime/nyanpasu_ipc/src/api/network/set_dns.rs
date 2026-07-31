use crate::api::R;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, net::IpAddr};

pub const NETWORK_SET_DNS_ENDPOINT: &str = "/network/set_dns";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct NetworkSetDnsReq<'n> {
    pub dns_servers: Option<Vec<Cow<'n, IpAddr>>>,
}

pub type NetworkSetDnsRes<'a> = R<'a, ()>;
