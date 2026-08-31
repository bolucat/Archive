//! The single source of truth for the IPC operation set.
//!
//! Each operation is a unit struct with one `IpcOperation` impl naming the
//! method, the path (always the existing `*_ENDPOINT` constant — never a fresh
//! string literal), the request body type and the payload carried in the
//! `data` field of the [`R`](super::R) envelope. The client's generic `call`
//! and the server's `register` both read the same impl, so a route can no
//! longer disagree with the client that talks to it.
//!
//! `/ws/events` is deliberately absent: it is a long-lived binary stream, not a
//! request/response operation, and has no `R` envelope. It keeps its own
//! constant in [`super::ws::events::EVENT_URI`].
//!
//! Written by hand on purpose. Ten impls cost less than a macro to maintain.

use std::fmt::Debug;

use http::Method;
use serde::{Serialize, de::DeserializeOwned};

use super::{
    R,
    core::{
        apply::{CORE_APPLY_ENDPOINT, CoreApplyData},
        check::CORE_CHECK_ENDPOINT,
        recover::CORE_RECOVER_ENDPOINT,
        restart::CORE_RESTART_ENDPOINT,
        start::CORE_START_ENDPOINT,
        stop::CORE_STOP_ENDPOINT,
        v2::{
            CORE_V2_OPERATION_ENDPOINT, CORE_V2_STATUS_ENDPOINT, CORE_V2_SUBMIT_ENDPOINT,
            OperationInfo,
        },
    },
    log::{LOGS_INSPECT_ENDPOINT, LOGS_RETRIEVE_ENDPOINT, LogsResBody},
    network::set_dns::{NETWORK_SET_DNS_ENDPOINT, NetworkSetDnsReq},
    status::{STATUS_ENDPOINT, StatusResBody},
};

/// One IPC operation: its wire address and its two body types.
pub trait IpcOperation {
    /// The HTTP method the operation is mounted with.
    const METHOD: Method;
    /// The endpoint path. Always one of the `*_ENDPOINT` constants.
    const PATH: &'static str;
    /// The JSON request body. `()` for the operations that send none — those
    /// are called with `None` and no body is written to the wire.
    type Req<'a>: Serialize;
    /// The payload carried in `R::data`.
    type Data: Serialize + DeserializeOwned + Debug;
}

/// The response envelope of `Op`, as it is decoded by the client.
pub type OpResponse<Op> = R<'static, <Op as IpcOperation>::Data>;

/// `GET /status`
pub struct Status;

impl IpcOperation for Status {
    const METHOD: Method = Method::GET;
    const PATH: &'static str = STATUS_ENDPOINT;
    type Req<'a> = ();
    type Data = StatusResBody<'static>;
}

/// `POST /core/start`
pub struct CoreStart;

impl IpcOperation for CoreStart {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_START_ENDPOINT;
    type Req<'a> = super::core::start::CoreStartReq<'a>;
    type Data = ();
}

/// `POST /core/stop`
pub struct CoreStop;

impl IpcOperation for CoreStop {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_STOP_ENDPOINT;
    type Req<'a> = ();
    type Data = ();
}

/// `POST /core/restart`
pub struct CoreRestart;

impl IpcOperation for CoreRestart {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_RESTART_ENDPOINT;
    type Req<'a> = ();
    type Data = ();
}

/// `POST /core/apply`
pub struct CoreApply;

impl IpcOperation for CoreApply {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_APPLY_ENDPOINT;
    type Req<'a> = super::core::apply::CoreApplyReq<'a>;
    type Data = CoreApplyData;
}

/// `POST /core/check`
pub struct CoreCheck;

impl IpcOperation for CoreCheck {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_CHECK_ENDPOINT;
    type Req<'a> = super::core::check::CoreCheckReq<'a>;
    type Data = ();
}

/// `POST /core/recover`
pub struct CoreRecover;

impl IpcOperation for CoreRecover {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_RECOVER_ENDPOINT;
    type Req<'a> = ();
    type Data = ();
}

/// `GET /logs/retrieve`
pub struct LogsRetrieve;

impl IpcOperation for LogsRetrieve {
    const METHOD: Method = Method::GET;
    const PATH: &'static str = LOGS_RETRIEVE_ENDPOINT;
    type Req<'a> = ();
    type Data = LogsResBody<'static>;
}

/// `GET /logs/inspect`
pub struct LogsInspect;

impl IpcOperation for LogsInspect {
    const METHOD: Method = Method::GET;
    const PATH: &'static str = LOGS_INSPECT_ENDPOINT;
    type Req<'a> = ();
    type Data = LogsResBody<'static>;
}

/// `POST /v2/core/submit`
pub struct CoreV2Submit;

impl IpcOperation for CoreV2Submit {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_V2_SUBMIT_ENDPOINT;
    type Req<'a> = super::core::v2::CoreSubmitReq<'a>;
    type Data = OperationInfo;
}

/// `POST /v2/core/operation`
pub struct CoreV2Operation;

impl IpcOperation for CoreV2Operation {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = CORE_V2_OPERATION_ENDPOINT;
    type Req<'a> = super::core::v2::CoreOperationReq<'a>;
    type Data = OperationInfo;
}

/// `GET /v2/core/status`
pub struct CoreV2Status;

impl IpcOperation for CoreV2Status {
    const METHOD: Method = Method::GET;
    const PATH: &'static str = CORE_V2_STATUS_ENDPOINT;
    type Req<'a> = ();
    type Data = super::status::CoreInfos;
}

/// `POST /network/set_dns`
pub struct NetworkSetDns;

impl IpcOperation for NetworkSetDns {
    const METHOD: Method = Method::POST;
    const PATH: &'static str = NETWORK_SET_DNS_ENDPOINT;
    type Req<'a> = NetworkSetDnsReq<'a>;
    type Data = ();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The contract must reproduce the pre-contract wire addresses exactly.
    /// A path or method typo here is a protocol break, so pin both literally
    /// rather than comparing a constant with itself.
    #[test]
    fn every_operation_keeps_its_legacy_path_and_method() {
        assert_eq!((Status::METHOD, Status::PATH), (Method::GET, "/status"));
        assert_eq!(
            (CoreStart::METHOD, CoreStart::PATH),
            (Method::POST, "/core/start")
        );
        assert_eq!(
            (CoreStop::METHOD, CoreStop::PATH),
            (Method::POST, "/core/stop")
        );
        assert_eq!(
            (CoreRestart::METHOD, CoreRestart::PATH),
            (Method::POST, "/core/restart")
        );
        assert_eq!(
            (LogsRetrieve::METHOD, LogsRetrieve::PATH),
            (Method::GET, "/logs/retrieve")
        );
        assert_eq!(
            (LogsInspect::METHOD, LogsInspect::PATH),
            (Method::GET, "/logs/inspect")
        );
        assert_eq!(
            (NetworkSetDns::METHOD, NetworkSetDns::PATH),
            (Method::POST, "/network/set_dns")
        );
    }

    /// The v2 control-plane endpoints, pinned for the same reason.
    #[test]
    fn every_v2_operation_keeps_its_declared_address() {
        assert_eq!(
            (CoreV2Submit::METHOD, CoreV2Submit::PATH),
            (Method::POST, "/v2/core/submit")
        );
        assert_eq!(
            (CoreV2Operation::METHOD, CoreV2Operation::PATH),
            (Method::POST, "/v2/core/operation")
        );
        assert_eq!(
            (CoreV2Status::METHOD, CoreV2Status::PATH),
            (Method::GET, "/v2/core/status")
        );
    }

    /// The S8 additions pin the report's addresses rather than legacy
    /// behaviour, but for the same reason: a path typo is a protocol break.
    #[test]
    fn every_s8_operation_is_addressed_as_the_report_says() {
        assert_eq!(
            (CoreApply::METHOD, CoreApply::PATH),
            (Method::POST, "/core/apply")
        );
        assert_eq!(
            (CoreCheck::METHOD, CoreCheck::PATH),
            (Method::POST, "/core/check")
        );
        assert_eq!(
            (CoreRecover::METHOD, CoreRecover::PATH),
            (Method::POST, "/core/recover")
        );
    }
}
