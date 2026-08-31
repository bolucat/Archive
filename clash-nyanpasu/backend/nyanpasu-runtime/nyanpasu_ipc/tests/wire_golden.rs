//! Golden snapshots of the IPC wire format.
//!
//! Every literal here is shipped protocol: the GUI decodes these exact shapes.
//! A diff in this file is a breaking change to clash-nyanpasu, never a test
//! that needs updating — if one of these fails, revert the change that broke
//! it or ship a protocol version.

use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::Arc,
};

use nyanpasu_ipc::api::{
    CoreErrorKind, R, RBuilder, ResponseCode,
    core::{
        apply::{ApplyOutcomeKind, CoreApplyData, CoreApplyReq},
        check::CoreCheckReq,
        start::CoreStartReq,
    },
    log::LogsResBody,
    network::set_dns::NetworkSetDnsReq,
    status::{
        ConfigRevisionInfo, CoreControllerInfo, CoreHealthInfo, CoreHealthState, CoreInfos,
        CoreState, CoreStateDetail, LogPathsInfo, RevisionIdInfo, RuntimeInfos, StatusResBody,
    },
    ws::events::{
        ClashCoreKind, EVENT_URI, Event, LogField, LogFrame, LogLevel, LogStream, LogTimestamp,
    },
};
use nyanpasu_utils::core::{ClashCoreType, CoreType};

/// Frozen so the envelope's `ts` does not make the goldens time-dependent.
const TS: i64 = 1_700_000_000;

fn ok_envelope<T>(data: T) -> R<'static, T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let mut envelope: R<'static, T> = RBuilder::success(data);
    envelope.ts = TS;
    envelope
}

fn error_envelope<T>(msg: &'static str) -> R<'static, T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let mut envelope: R<'static, T> = RBuilder::other_error(Cow::Borrowed(msg));
    envelope.ts = TS;
    envelope
}

fn error_envelope_with_kind<T>(
    msg: &'static str,
    kind: CoreErrorKind,
    retryable: Option<bool>,
) -> R<'static, T>
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let mut envelope: R<'static, T> =
        RBuilder::other_error_with_kind(Cow::Borrowed(msg), Some(kind), retryable);
    envelope.ts = TS;
    envelope
}

/// The S7-enriched snapshot, the same value `the_enriched_status_response_is_pinned`
/// carries — so a diff between the two literals is a real drift, not a fixture
/// that fell behind.
fn enriched_core_infos() -> CoreInfos {
    CoreInfos {
        r#type: Some(CoreType::Clash(ClashCoreType::Mihomo)),
        state: CoreState::Running,
        state_changed_at: 42,
        config_path: Some(PathBuf::from("/etc/nyanpasu/config.yaml")),
        controller: Some(CoreControllerInfo::Http(
            "http://127.0.0.1:9090/".to_owned(),
        )),
        health: Some(CoreHealthInfo {
            state: CoreHealthState::Healthy,
            changed_at: 1_700_000_000_123,
            consecutive_failures: 0,
            last_error: None,
            last_success_at: Some(1_700_000_000_000),
        }),
        revision: Some(ConfigRevisionInfo {
            epoch: 3,
            generation: 7,
            source_hash: "0123456789abcdef".to_owned(),
            effective_hash: "fedcba9876543210".to_owned(),
        }),
        detail: Some(CoreStateDetail::Running {
            epoch: 3,
            pid: 4242,
        }),
    }
}

/// A never-started core: every S7 field absent, `detail` still populated.
fn minimal_core_infos() -> CoreInfos {
    CoreInfos {
        r#type: None,
        state: CoreState::Stopped(None),
        state_changed_at: 42,
        config_path: None,
        controller: None,
        health: None,
        revision: None,
        detail: Some(CoreStateDetail::Stopped { reason: None }),
    }
}

#[test]
fn the_status_event_is_pinned() {
    assert_eq!(
        serde_json::to_string(&Event::new_core_status_changed(enriched_core_infos())).unwrap(),
        concat!(
            r#"{"CoreStatusChanged":{"type":{"clash":"mihomo"},"state":"Running","#,
            r#""state_changed_at":42,"config_path":"/etc/nyanpasu/config.yaml","#,
            r#""controller":{"Http":"http://127.0.0.1:9090/"},"#,
            r#""health":{"state":"Healthy","changed_at":1700000000123,"#,
            r#""consecutive_failures":0,"last_error":null,"#,
            r#""last_success_at":1700000000000},"#,
            r#""revision":{"epoch":3,"generation":7,"#,
            r#""source_hash":"0123456789abcdef","#,
            r#""effective_hash":"fedcba9876543210"},"#,
            r#""detail":{"Running":{"epoch":3,"pid":4242}}}}"#
        )
    );
    assert_eq!(
        serde_json::to_string(&Event::new_core_status_changed(minimal_core_infos())).unwrap(),
        concat!(
            r#"{"CoreStatusChanged":{"type":null,"state":{"Stopped":null},"#,
            r#""state_changed_at":42,"config_path":null,"#,
            r#""detail":{"Stopped":{"reason":null}}}}"#
        )
    );
}

/// The defect the variant exists to fix (report §1.2): a crash loop reports
/// `Stopped(None)` on the lossy field and `Restarting` on the faithful one, in
/// same frame.
#[test]
fn the_status_payload_distinguishes_a_crash_loop_from_a_stop() {
    let mut infos = minimal_core_infos();
    infos.r#type = Some(CoreType::Clash(ClashCoreType::Mihomo));
    infos.detail = Some(CoreStateDetail::Restarting {
        epoch: 3,
        attempt: 2,
    });
    assert_eq!(
        serde_json::to_string(&Event::new_core_status_changed(infos)).unwrap(),
        concat!(
            r#"{"CoreStatusChanged":{"type":{"clash":"mihomo"},"#,
            r#""state":{"Stopped":null},"state_changed_at":42,"config_path":null,"#,
            r#""detail":{"Restarting":{"epoch":3,"attempt":2}}}}"#
        )
    );
}

/// Push *is* snapshot: the frame's payload is the same object `/status` puts in
/// `core_infos`, byte for byte. A client decodes one with the other's decoder.
#[test]
fn the_status_payload_is_the_status_core_infos() {
    let infos = enriched_core_infos();
    let payload = serde_json::to_string(&infos).unwrap();
    let frame = serde_json::to_string(&Event::new_core_status_changed(infos)).unwrap();
    assert_eq!(frame, format!(r#"{{"CoreStatusChanged":{payload}}}"#));
}

/// The endpoint string is protocol: the GUI builds the URL from it. There is
/// no version parameter left to pin — the stream is unversioned, and the
/// service ignores whatever query it is handed.
#[test]
fn the_event_endpoint_is_pinned() {
    assert_eq!(EVENT_URI, "/ws/events");
}

/// The other half: a status frame decodes back, and a frame written by a
/// *newer* service — one extra field in the payload — still decodes instead of
/// killing the stream.
#[test]
fn a_status_frame_decodes_back() {
    let frame =
        serde_json::to_string(&Event::new_core_status_changed(enriched_core_infos())).unwrap();
    match serde_json::from_str::<Event>(&frame).unwrap() {
        Event::CoreStatusChanged(infos) => {
            assert!(matches!(infos.state, CoreState::Running));
            assert_eq!(
                infos.detail,
                Some(CoreStateDetail::Running {
                    epoch: 3,
                    pid: 4242
                })
            );
            assert_eq!(infos.revision.unwrap().generation, 7);
        }
        other => panic!("expected a status snapshot, got: {other:?}"),
    }
    let forward = concat!(
        r#"{"CoreStatusChanged":{"type":null,"state":"Running","#,
        r#""state_changed_at":1,"config_path":null,"future_field":7}}"#
    );
    assert!(serde_json::from_str::<Event>(forward).is_ok());
}

#[test]
fn response_codes_and_their_messages_are_pinned() {
    assert_eq!(serde_json::to_string(&ResponseCode::Ok).unwrap(), r#""Ok""#);
    assert_eq!(
        serde_json::to_string(&ResponseCode::OtherError).unwrap(),
        r#""OtherError""#
    );
    assert_eq!(ResponseCode::Ok.msg(), "ok");
    assert_eq!(ResponseCode::OtherError.msg(), "other error");
}

#[test]
fn the_unit_response_envelope_is_pinned() {
    let mut built: R<'static, ()> = RBuilder::success(());
    built.ts = TS;
    assert_eq!(
        serde_json::to_string(&built).unwrap(),
        r#"{"code":"Ok","msg":"ok","data":null,"ts":1700000000}"#
    );
}

/// The three legacy core error strings are protocol, not diagnostics: the GUI
/// branches on them.
#[test]
fn the_legacy_core_error_envelopes_are_pinned() {
    for msg in [
        "core is already running",
        "core is already stopped",
        "core have not been started yet",
    ] {
        let envelope: R<'static, ()> = error_envelope(msg);
        assert_eq!(
            serde_json::to_string(&envelope).unwrap(),
            format!(r#"{{"code":"OtherError","msg":"{msg}","data":null,"ts":1700000000}}"#)
        );
    }
}

#[test]
fn the_core_start_request_is_pinned() {
    let request = CoreStartReq {
        core_type: Cow::Owned(CoreType::Clash(ClashCoreType::Mihomo)),
        config_file: Cow::Owned(PathBuf::from("/etc/nyanpasu/config.yaml")),
    };
    assert_eq!(
        serde_json::to_string(&request).unwrap(),
        r#"{"core_type":{"clash":"mihomo"},"config_file":"/etc/nyanpasu/config.yaml"}"#
    );
}

#[test]
fn every_core_type_tag_is_pinned() {
    let cases = [
        (ClashCoreType::Mihomo, r#"{"clash":"mihomo"}"#),
        (ClashCoreType::MihomoAlpha, r#"{"clash":"mihomo-alpha"}"#),
        (ClashCoreType::ClashRust, r#"{"clash":"clash-rs"}"#),
        (
            ClashCoreType::ClashRustAlpha,
            r#"{"clash":"clash-rs-alpha"}"#,
        ),
        (ClashCoreType::ClashPremium, r#"{"clash":"clash"}"#),
        (ClashCoreType::Meow, r#"{"clash":"meow"}"#),
    ];
    for (core, expected) in cases {
        assert_eq!(
            serde_json::to_string(&CoreType::Clash(core)).unwrap(),
            expected
        );
    }
    assert_eq!(
        serde_json::to_string(&CoreType::SingBox).unwrap(),
        r#""singbox""#
    );
}

#[test]
fn the_set_dns_request_is_pinned() {
    let with_servers = NetworkSetDnsReq {
        dns_servers: Some(vec![
            Cow::Owned(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
            Cow::Owned(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
        ]),
    };
    assert_eq!(
        serde_json::to_string(&with_servers).unwrap(),
        r#"{"dns_servers":["1.1.1.1","8.8.8.8"]}"#
    );
    assert_eq!(
        serde_json::to_string(&NetworkSetDnsReq { dns_servers: None }).unwrap(),
        r#"{"dns_servers":null}"#
    );
}

#[test]
fn the_logs_response_is_pinned() {
    let body = LogsResBody {
        logs: vec![Cow::Borrowed("first"), Cow::Borrowed("second")],
    };
    assert_eq!(
        serde_json::to_string(&ok_envelope(body)).unwrap(),
        r#"{"code":"Ok","msg":"ok","data":{"logs":["first","second"]},"ts":1700000000}"#
    );
}

/// The absent S7 fields keep this pre-S7 JSON byte-identical.
#[test]
fn the_status_response_is_pinned() {
    let body = StatusResBody {
        version: Cow::Borrowed("9.9.9-golden"),
        core_infos: CoreInfos {
            r#type: Some(CoreType::Clash(ClashCoreType::Mihomo)),
            state: CoreState::Running,
            state_changed_at: 42,
            config_path: Some(PathBuf::from("/etc/nyanpasu/config.yaml")),
            controller: None,
            health: None,
            revision: None,
            detail: None,
        },
        runtime_infos: RuntimeInfos {
            service_data_dir: Cow::Owned(PathBuf::from("/srv/data")),
            service_config_dir: Cow::Owned(PathBuf::from("/srv/config")),
            nyanpasu_config_dir: Cow::Owned(PathBuf::from("/home/config")),
            nyanpasu_data_dir: Cow::Owned(PathBuf::from("/home/data")),
        },
        logs: None,
    };
    assert_eq!(
        serde_json::to_string(&ok_envelope(body)).unwrap(),
        concat!(
            r#"{"code":"Ok","msg":"ok","data":{"version":"9.9.9-golden","#,
            r#""core_infos":{"type":{"clash":"mihomo"},"state":"Running","#,
            r#""state_changed_at":42,"config_path":"/etc/nyanpasu/config.yaml"},"#,
            r#""runtime_infos":{"service_data_dir":"/srv/data","#,
            r#""service_config_dir":"/srv/config","#,
            r#""nyanpasu_config_dir":"/home/config","#,
            r#""nyanpasu_data_dir":"/home/data"}},"ts":1700000000}"#
        )
    );
}

#[test]
fn the_core_states_are_pinned() {
    assert_eq!(
        serde_json::to_string(&CoreState::Running).unwrap(),
        r#""Running""#
    );
    assert_eq!(
        serde_json::to_string(&CoreState::Stopped(None)).unwrap(),
        r#"{"Stopped":null}"#
    );
    assert_eq!(
        serde_json::to_string(&CoreState::Stopped(Some("boom".to_owned()))).unwrap(),
        r#"{"Stopped":"boom"}"#
    );
}

/// The legacy state frame, unchanged since before S7. A service-log frame was
/// pinned here too until L3, when the service stopped pushing its own logs.
/// (Deliberately not naming the removed variant: §7 step 2's `rg` gate covers
/// this file.)
#[test]
fn the_ws_events_are_pinned() {
    assert_eq!(
        serde_json::to_string(&Event::new_core_state_changed(CoreState::Running)).unwrap(),
        r#"{"CoreStateChanged":"Running"}"#
    );
}

/// The fixture both serializer pins use. `crates/nyanpasu-service-runtime`'s
/// `ws_core_log_frames_are_pinned` builds the identical value and asserts the
/// identical literal: the service writes with `simd_json` and the client reads
/// with `serde_json`, so a divergence must show up as a diff between two
/// otherwise identical strings.
fn pinned_core_log() -> Event {
    Event::new_core_log(Arc::new(LogFrame {
        at: 1_700_000_000_000,
        epoch: 1,
        kind: ClashCoreKind::Mihomo,
        stream: LogStream::Stdout,
        level: LogLevel::Info,
        timestamp: Some(LogTimestamp {
            raw: "2026-07-29T00:16:22.646059400+08:00".to_owned(),
            unix_ms: Some(1_753_719_382_646),
            inferred: false,
        }),
        target: None,
        message: "hello core".to_owned(),
        fields: vec![LogField {
            key: "request".to_owned(),
            value: "7".to_owned(),
        }],
        raw: "time=\"2026-07-29T00:16:22.646059400+08:00\" level=info msg=\"hello core\" request=7"
            .to_owned(),
        truncated: false,
    }))
}

#[test]
fn the_core_log_event_is_pinned() {
    assert_eq!(
        serde_json::to_string(&pinned_core_log()).unwrap(),
        concat!(
            r#"{"CoreLog":{"at":1700000000000,"epoch":1,"kind":"mihomo","stream":"stdout","#,
            r#""level":"info","timestamp":{"raw":"2026-07-29T00:16:22.646059400+08:00","#,
            r#""unix_ms":1753719382646,"inferred":false},"target":null,"#,
            r#""message":"hello core","fields":[{"key":"request","value":"7"}],"#,
            r#""raw":"time=\"2026-07-29T00:16:22.646059400+08:00\" level=info "#,
            r#"msg=\"hello core\" request=7","truncated":false}}"#
        )
    );
}

/// The degraded shape, which is the common one: a line whose header did not
/// parse has no clock, no target and no fields — and `at` is why the record is
/// still sortable.
#[test]
fn a_degraded_core_log_event_is_pinned() {
    let event = Event::new_core_log(Arc::new(LogFrame {
        at: 1_700_000_000_001,
        epoch: 2,
        kind: ClashCoreKind::ClashRust,
        stream: LogStream::Stderr,
        level: LogLevel::Warning,
        timestamp: None,
        target: None,
        message: "unparsed line".to_owned(),
        fields: Vec::new(),
        raw: "unparsed line".to_owned(),
        truncated: true,
    }));
    assert_eq!(
        serde_json::to_string(&event).unwrap(),
        concat!(
            r#"{"CoreLog":{"at":1700000000001,"epoch":2,"kind":"clash-rs","stream":"stderr","#,
            r#""level":"warning","timestamp":null,"target":null,"#,
            r#""message":"unparsed line","fields":[],"raw":"unparsed line","#,
            r#""truncated":true}}"#
        )
    );
}

#[test]
fn the_error_envelope_decodes_back() {
    let encoded = serde_json::to_string(&error_envelope::<()>("core is already stopped")).unwrap();
    let decoded: R<'static, ()> = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.code, ResponseCode::OtherError);
    assert_eq!(decoded.msg, "core is already stopped");
    assert!(decoded.data.is_none());
    assert_eq!(decoded.ts, TS);
}

#[test]
fn the_controller_infos_are_pinned() {
    // JSON escapes every backslash, so each one in a named pipe path doubles.
    assert_eq!(
        serde_json::to_string(&CoreControllerInfo::NamedPipe(PathBuf::from(
            r"\\.\pipe\nyanpasu\core-1"
        )))
        .unwrap(),
        r#"{"NamedPipe":"\\\\.\\pipe\\nyanpasu\\core-1"}"#
    );
    assert_eq!(
        serde_json::to_string(&CoreControllerInfo::UnixSocket(PathBuf::from(
            "/run/nyanpasu/core-1.sock"
        )))
        .unwrap(),
        r#"{"UnixSocket":"/run/nyanpasu/core-1.sock"}"#
    );
    assert_eq!(
        serde_json::to_string(&CoreControllerInfo::Http(
            "http://127.0.0.1:9090/".to_owned()
        ))
        .unwrap(),
        r#"{"Http":"http://127.0.0.1:9090/"}"#
    );
}

#[test]
fn the_core_state_details_are_pinned() {
    for (value, expected) in [
        (
            CoreStateDetail::Stopped { reason: None },
            r#"{"Stopped":{"reason":null}}"#,
        ),
        (
            CoreStateDetail::Stopped {
                reason: Some("boom".to_owned()),
            },
            r#"{"Stopped":{"reason":"boom"}}"#,
        ),
        (
            CoreStateDetail::Starting { epoch: 3 },
            r#"{"Starting":{"epoch":3}}"#,
        ),
        (
            CoreStateDetail::Running {
                epoch: 3,
                pid: 4242,
            },
            r#"{"Running":{"epoch":3,"pid":4242}}"#,
        ),
        (
            CoreStateDetail::Restarting {
                epoch: 3,
                attempt: 2,
            },
            r#"{"Restarting":{"epoch":3,"attempt":2}}"#,
        ),
        (
            CoreStateDetail::Switching {
                from: Some(2),
                to: 3,
            },
            r#"{"Switching":{"from":2,"to":3}}"#,
        ),
        (
            CoreStateDetail::Switching { from: None, to: 1 },
            r#"{"Switching":{"from":null,"to":1}}"#,
        ),
        (
            CoreStateDetail::Stopping { epoch: 3 },
            r#"{"Stopping":{"epoch":3}}"#,
        ),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    }
}

#[test]
fn the_core_health_infos_are_pinned() {
    for (value, expected) in [
        (CoreHealthState::Starting, r#""Starting""#),
        (CoreHealthState::Healthy, r#""Healthy""#),
        (CoreHealthState::Unhealthy, r#""Unhealthy""#),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    }
    assert_eq!(
        serde_json::to_string(&CoreHealthInfo {
            state: CoreHealthState::Unhealthy,
            changed_at: 1_700_000_000_123,
            consecutive_failures: 3,
            last_error: Some("probe timed out".to_owned()),
            last_success_at: Some(1_700_000_000_000),
        })
        .unwrap(),
        concat!(
            r#"{"state":"Unhealthy","changed_at":1700000000123,"#,
            r#""consecutive_failures":3,"last_error":"probe timed out","#,
            r#""last_success_at":1700000000000}"#
        )
    );
}

#[test]
fn the_config_revision_info_is_pinned() {
    assert_eq!(
        serde_json::to_string(&ConfigRevisionInfo {
            epoch: 3,
            generation: 7,
            source_hash: "0123456789abcdef".to_owned(),
            effective_hash: "fedcba9876543210".to_owned(),
        })
        .unwrap(),
        concat!(
            r#"{"epoch":3,"generation":7,"source_hash":"0123456789abcdef","#,
            r#""effective_hash":"fedcba9876543210"}"#
        )
    );
}

/// The S7 fields, all populated, inside the production envelope.
#[test]
fn the_enriched_status_response_is_pinned() {
    let body = StatusResBody {
        version: Cow::Borrowed("9.9.9-golden"),
        core_infos: CoreInfos {
            r#type: Some(CoreType::Clash(ClashCoreType::Mihomo)),
            state: CoreState::Running,
            state_changed_at: 42,
            config_path: Some(PathBuf::from("/etc/nyanpasu/config.yaml")),
            controller: Some(CoreControllerInfo::Http(
                "http://127.0.0.1:9090/".to_owned(),
            )),
            health: Some(CoreHealthInfo {
                state: CoreHealthState::Healthy,
                changed_at: 1_700_000_000_123,
                consecutive_failures: 0,
                last_error: None,
                last_success_at: Some(1_700_000_000_000),
            }),
            revision: Some(ConfigRevisionInfo {
                epoch: 3,
                generation: 7,
                source_hash: "0123456789abcdef".to_owned(),
                effective_hash: "fedcba9876543210".to_owned(),
            }),
            detail: Some(CoreStateDetail::Running {
                epoch: 3,
                pid: 4242,
            }),
        },
        runtime_infos: RuntimeInfos {
            service_data_dir: Cow::Owned(PathBuf::from("/srv/data")),
            service_config_dir: Cow::Owned(PathBuf::from("/srv/config")),
            nyanpasu_config_dir: Cow::Owned(PathBuf::from("/home/config")),
            nyanpasu_data_dir: Cow::Owned(PathBuf::from("/home/data")),
        },
        logs: None,
    };
    assert_eq!(
        serde_json::to_string(&ok_envelope(body)).unwrap(),
        concat!(
            r#"{"code":"Ok","msg":"ok","data":{"version":"9.9.9-golden","#,
            r#""core_infos":{"type":{"clash":"mihomo"},"state":"Running","#,
            r#""state_changed_at":42,"config_path":"/etc/nyanpasu/config.yaml","#,
            r#""controller":{"Http":"http://127.0.0.1:9090/"},"#,
            r#""health":{"state":"Healthy","changed_at":1700000000123,"#,
            r#""consecutive_failures":0,"last_error":null,"#,
            r#""last_success_at":1700000000000},"#,
            r#""revision":{"epoch":3,"generation":7,"#,
            r#""source_hash":"0123456789abcdef","#,
            r#""effective_hash":"fedcba9876543210"},"#,
            r#""detail":{"Running":{"epoch":3,"pid":4242}}},"#,
            r#""runtime_infos":{"service_data_dir":"/srv/data","#,
            r#""service_config_dir":"/srv/config","#,
            r#""nyanpasu_config_dir":"/home/config","#,
            r#""nyanpasu_data_dir":"/home/data"}},"ts":1700000000}"#
        )
    );
}

/// The L3 addition. The field is `Option` only so a payload without it matches
/// the pre-L3 bytes exactly — which the two literals above still assert — but
/// the service always sends it.
#[test]
fn the_status_log_paths_are_pinned() {
    assert_eq!(
        serde_json::to_string(&LogPathsInfo {
            service_dir: PathBuf::from("/var/lib/nyanpasu-service/logs"),
            core_dir: Some(PathBuf::from("/var/lib/nyanpasu-service/core-runtime/logs")),
        })
        .unwrap(),
        concat!(
            r#"{"service_dir":"/var/lib/nyanpasu-service/logs","#,
            r#""core_dir":"/var/lib/nyanpasu-service/core-runtime/logs"}"#
        )
    );
    // Sink disabled: the key is omitted, not null.
    assert_eq!(
        serde_json::to_string(&LogPathsInfo {
            service_dir: PathBuf::from("/var/lib/nyanpasu-service/logs"),
            core_dir: None,
        })
        .unwrap(),
        r#"{"service_dir":"/var/lib/nyanpasu-service/logs"}"#
    );
}

/// The other half of the compatibility gate: a payload written by a pre-S7
/// service must still decode, with the new fields absent rather than an error.
#[test]
fn a_pre_s7_status_payload_still_decodes() {
    let legacy = concat!(
        r#"{"code":"Ok","msg":"ok","data":{"version":"1.4.5","#,
        r#""core_infos":{"type":{"clash":"mihomo"},"state":"Running","#,
        r#""state_changed_at":42,"config_path":"/etc/nyanpasu/config.yaml"},"#,
        r#""runtime_infos":{"service_data_dir":"/srv/data","#,
        r#""service_config_dir":"/srv/config","#,
        r#""nyanpasu_config_dir":"/home/config","#,
        r#""nyanpasu_data_dir":"/home/data"}},"ts":1700000000}"#
    );
    let decoded: R<'static, StatusResBody<'static>> = serde_json::from_str(legacy).unwrap();
    let core_infos = decoded.data.unwrap().core_infos;
    assert!(core_infos.controller.is_none());
    assert!(core_infos.health.is_none());
    assert!(core_infos.revision.is_none());
    assert!(core_infos.detail.is_none());
}

#[test]
fn the_apply_outcome_kinds_are_pinned() {
    for (value, expected) in [
        (ApplyOutcomeKind::Noop, r#""noop""#),
        (ApplyOutcomeKind::Patched, r#""patched""#),
        (ApplyOutcomeKind::Reloaded, r#""reloaded""#),
        (ApplyOutcomeKind::Restarted, r#""restarted""#),
        // Produced since S10 by the manager's core-switch path; the spelling was
        // pinned a stage before it was reachable.
        (ApplyOutcomeKind::Switched, r#""switched""#),
        (ApplyOutcomeKind::RolledBack, r#""rolled_back""#),
    ] {
        assert_eq!(serde_json::to_string(&value).unwrap(), expected);
    }
}

#[test]
fn the_error_kind_strings_are_pinned() {
    // These are protocol: a caller branches on them. The enum lives in
    // nyanpasu-core-metadata now, so this pins what actually reaches the wire.
    for (kind, expected) in [
        (CoreErrorKind::NotStarted, r#""not_started""#),
        (CoreErrorKind::AlreadyRunning, r#""already_running""#),
        (CoreErrorKind::RevisionConflict, r#""revision_conflict""#),
        (CoreErrorKind::Quarantined, r#""quarantined""#),
        (CoreErrorKind::ConfigCheckFailed, r#""config_check_failed""#),
        (CoreErrorKind::ConfigNotFound, r#""config_not_found""#),
        (CoreErrorKind::BinaryNotFound, r#""binary_not_found""#),
        (CoreErrorKind::InvalidConfig, r#""invalid_config""#),
        (CoreErrorKind::ControllerMissing, r#""controller_missing""#),
        (CoreErrorKind::ApplyFailed, r#""apply_failed""#),
        (
            CoreErrorKind::ApplyRollbackFailed,
            r#""apply_rollback_failed""#,
        ),
        (CoreErrorKind::StopUnconfirmed, r#""stop_unconfirmed""#),
        // The control-plane admission and routing kinds (PR-A).
        (CoreErrorKind::ShuttingDown, r#""shutting_down""#),
        (CoreErrorKind::QueueFull, r#""queue_full""#),
        (CoreErrorKind::OperationConflict, r#""operation_conflict""#),
        (
            CoreErrorKind::BackendUnavailable,
            r#""backend_unavailable""#,
        ),
        (CoreErrorKind::Internal, r#""internal""#),
    ] {
        assert_eq!(serde_json::to_string(&kind).unwrap(), expected);
    }
    // Every kind is covered above; a new one must be added here too.
    assert_eq!(CoreErrorKind::ALL.len(), 17);
}

/// The new field is appended, so no existing key moves; the absent case is
/// pinned by every other envelope golden in this file staying unchanged.
#[test]
fn an_error_envelope_carries_its_kind() {
    let envelope: R<'static, ()> = error_envelope_with_kind(
        "config revision conflict",
        CoreErrorKind::RevisionConflict,
        None,
    );
    assert_eq!(
        serde_json::to_string(&envelope).unwrap(),
        concat!(
            r#"{"code":"OtherError","msg":"config revision conflict","data":null,"#,
            r#""ts":1700000000,"error_kind":"revision_conflict"}"#
        )
    );
}

/// `retryable` is appended after `error_kind`, and an unanswered one is still
/// omitted — the literal above is the proof that adding it moved no key.
#[test]
fn an_error_envelope_carries_its_retryability() {
    let envelope: R<'static, ()> = error_envelope_with_kind(
        "the operation queue is full",
        CoreErrorKind::QueueFull,
        Some(true),
    );
    assert_eq!(
        serde_json::to_string(&envelope).unwrap(),
        concat!(
            r#"{"code":"OtherError","msg":"the operation queue is full","data":null,"#,
            r#""ts":1700000000,"error_kind":"queue_full","retryable":true}"#
        )
    );
}

/// The other half of the compatibility gate: an envelope written by a pre-S8
/// service has no `error_kind` key and must still decode.
#[test]
fn a_pre_s8_envelope_still_decodes() {
    let legacy = r#"{"code":"OtherError","msg":"boom","data":null,"ts":1700000000}"#;
    let decoded: R<'static, ()> = serde_json::from_str(legacy).unwrap();
    assert_eq!(decoded.code, ResponseCode::OtherError);
    assert!(decoded.error_kind.is_none());
    assert!(decoded.retryable.is_none());
}

#[test]
fn the_revision_id_info_is_pinned() {
    let revision = ConfigRevisionInfo {
        epoch: 3,
        generation: 7,
        source_hash: "0123456789abcdef".to_owned(),
        effective_hash: "fedcba9876543210".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&revision.id()).unwrap(),
        r#"{"epoch":3,"generation":7,"effective_hash":"fedcba9876543210"}"#
    );
    // The CAS token is a strict subset: `source_hash` takes no part in it.
    assert_eq!(
        revision.id(),
        RevisionIdInfo {
            epoch: 3,
            generation: 7,
            effective_hash: "fedcba9876543210".to_owned(),
        }
    );
}

#[test]
fn the_core_apply_request_is_pinned() {
    let without = CoreApplyReq {
        core_type: Cow::Owned(CoreType::Clash(ClashCoreType::Mihomo)),
        config_file: Cow::Owned(PathBuf::from("/etc/nyanpasu/config.yaml")),
        expected_revision: None,
    };
    // No CAS token: the key is omitted, not sent as null.
    assert_eq!(
        serde_json::to_string(&without).unwrap(),
        r#"{"core_type":{"clash":"mihomo"},"config_file":"/etc/nyanpasu/config.yaml"}"#
    );
    let with = CoreApplyReq {
        expected_revision: Some(RevisionIdInfo {
            epoch: 3,
            generation: 7,
            effective_hash: "fedcba9876543210".to_owned(),
        }),
        ..without
    };
    assert_eq!(
        serde_json::to_string(&with).unwrap(),
        concat!(
            r#"{"core_type":{"clash":"mihomo"},"#,
            r#""config_file":"/etc/nyanpasu/config.yaml","#,
            r#""expected_revision":{"epoch":3,"generation":7,"#,
            r#""effective_hash":"fedcba9876543210"}}"#
        )
    );
}

#[test]
fn the_core_check_request_is_pinned() {
    let request = CoreCheckReq {
        core_type: Cow::Owned(CoreType::Clash(ClashCoreType::Mihomo)),
        config_file: Cow::Owned(PathBuf::from("/etc/nyanpasu/config.yaml")),
    };
    assert_eq!(
        serde_json::to_string(&request).unwrap(),
        r#"{"core_type":{"clash":"mihomo"},"config_file":"/etc/nyanpasu/config.yaml"}"#
    );
}

#[test]
fn the_core_apply_response_is_pinned() {
    let revision = ConfigRevisionInfo {
        epoch: 3,
        generation: 7,
        source_hash: "0123456789abcdef".to_owned(),
        effective_hash: "fedcba9876543210".to_owned(),
    };
    let clean = CoreApplyData {
        outcome: ApplyOutcomeKind::Patched,
        revision: revision.clone(),
        warning: None,
        failed_apply: None,
    };
    assert_eq!(
        serde_json::to_string(&ok_envelope(clean)).unwrap(),
        concat!(
            r#"{"code":"Ok","msg":"ok","data":{"outcome":"patched","#,
            r#""revision":{"epoch":3,"generation":7,"#,
            r#""source_hash":"0123456789abcdef","#,
            r#""effective_hash":"fedcba9876543210"}},"ts":1700000000}"#
        )
    );
    // A rollback is a *successful* call reporting that the old config runs.
    let rolled_back = CoreApplyData {
        outcome: ApplyOutcomeKind::RolledBack,
        revision,
        warning: Some("runtime directory sync failed".to_owned()),
        failed_apply: Some("core failed to start".to_owned()),
    };
    assert_eq!(
        serde_json::to_string(&ok_envelope(rolled_back)).unwrap(),
        concat!(
            r#"{"code":"Ok","msg":"ok","data":{"outcome":"rolled_back","#,
            r#""revision":{"epoch":3,"generation":7,"#,
            r#""source_hash":"0123456789abcdef","#,
            r#""effective_hash":"fedcba9876543210"},"#,
            r#""warning":"runtime directory sync failed","#,
            r#""failed_apply":"core failed to start"},"ts":1700000000}"#
        )
    );
}
