use reqwest::Method;

use crate::{Client, Error, Result, retry::RequestMetadata};

/// DNS RR type sent to Mihomo. Constants cover common queries while `new`
/// remains forward-compatible with types added by miekg/dns.
#[derive(Clone, Debug, PartialEq, Eq, Hash, specta::Type)]
#[specta(transparent)]
pub struct DnsRecordType(String);

impl DnsRecordType {
    pub const A: &'static str = "A";
    pub const AAAA: &'static str = "AAAA";
    pub const CNAME: &'static str = "CNAME";
    pub const MX: &'static str = "MX";
    pub const NS: &'static str = "NS";
    pub const PTR: &'static str = "PTR";
    pub const SOA: &'static str = "SOA";
    pub const SRV: &'static str = "SRV";
    pub const TXT: &'static str = "TXT";
    pub const HTTPS: &'static str = "HTTPS";
    pub const SVCB: &'static str = "SVCB";

    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(Error::InvalidArgument {
                argument: "type",
                message: "must be a case-sensitive miekg/dns record type".to_owned(),
            });
        }
        Ok(Self(value))
    }

    pub fn a() -> Self {
        Self(Self::A.to_owned())
    }

    pub fn aaaa() -> Self {
        Self(Self::AAAA.to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, specta::Type)]
pub struct DnsQuery {
    pub name: String,
    pub record_type: DnsRecordType,
}

impl DnsQuery {
    pub fn new(name: impl Into<String>, record_type: DnsRecordType) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(Error::InvalidArgument {
                argument: "name",
                message: "must not be empty".to_owned(),
            });
        }
        Ok(Self { name, record_type })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct DnsQuestion {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Qtype")]
    pub query_type: u16,
    #[serde(rename = "Qclass")]
    pub query_class: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct DnsRecord {
    pub name: String,
    #[serde(rename = "type")]
    pub record_type: u16,
    #[serde(rename = "TTL")]
    pub ttl: u32,
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct DnsResponse {
    #[serde(rename = "Status")]
    pub status: i64,
    #[serde(rename = "Question")]
    pub question: Vec<DnsQuestion>,
    #[serde(rename = "TC")]
    pub truncated: bool,
    #[serde(rename = "RD")]
    pub recursion_desired: bool,
    #[serde(rename = "RA")]
    pub recursion_available: bool,
    #[serde(rename = "AD")]
    pub authenticated_data: bool,
    #[serde(rename = "CD")]
    pub checking_disabled: bool,
    #[serde(rename = "Answer", default)]
    pub answer: Option<Vec<DnsRecord>>,
    #[serde(rename = "Authority", default)]
    pub authority: Option<Vec<DnsRecord>>,
    #[serde(rename = "Additional", default)]
    pub additional: Option<Vec<DnsRecord>>,
}

impl Client {
    pub async fn dns_query(&self, query: &DnsQuery) -> Result<DnsResponse> {
        let pairs = [
            ("name", query.name.as_str()),
            ("type", query.record_type.as_str()),
        ];
        self.send_json(RequestMetadata::new("dns_query", Method::GET, true), || {
            Ok(self.get("/dns/query")?.query(&pairs))
        })
        .await
    }
}
