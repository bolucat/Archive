//! The v2 control-plane wire: submit / operation query / status. Additive —
//! every v1 endpoint stays untouched until the app-side switch (the bridge
//! stage) deletes them.
//!
//! Deliberate absences:
//!
//! - No shutdown command: the daemon's own lifecycle belongs to the OS
//!   service layer, never to the core wire.
//! - No caller paths: the caller ships the config *bytes*; the daemon
//!   materializes them itself (design §16.2).
//! - No check command: the advisory check keeps its v1 endpoint until the
//!   bridge stage; it is not a mutating operation and gains nothing from the
//!   submit envelope.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use crate::api::{
    R,
    status::{ConfigRevisionInfo, RevisionIdInfo},
};

pub const CORE_V2_SUBMIT_ENDPOINT: &str = "/v2/core/submit";
pub const CORE_V2_OPERATION_ENDPOINT: &str = "/v2/core/operation";
pub const CORE_V2_STATUS_ENDPOINT: &str = "/v2/core/status";

/// Submit one mutating operation. The reply is the operation's registry
/// snapshot at admission time — usually `queued` — not its result; poll or
/// long-poll [`CoreOperationReq`] for that.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreSubmitReq<'n> {
    /// 32 lowercase hex characters: the idempotency identity. The same id
    /// with the same payload attaches to the original operation; the same id
    /// with a different payload fails with `error_kind =
    /// "operation_conflict"`.
    pub operation_id: Cow<'n, str>,
    pub command: CoreCommandInfo<'n>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CoreCommandInfo<'n> {
    /// The single mutating convergence command: make the runtime match this
    /// core + config. Start, patch, reload, restart, and switch are derived
    /// inside the transaction, never chosen by the caller — and the config
    /// check runs inside it too (a rejection is a clean abort with
    /// `error_kind = "invalid_config"` or `"config_check_failed"`).
    Reconcile {
        core_type: Cow<'n, nyanpasu_utils::core::CoreType>,
        /// The full config document, as text. Never a path.
        config: Cow<'n, str>,
        /// FNV-1a hex digest of `config` as the caller computed it; verified
        /// on receipt when present.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_digest: Option<Cow<'n, str>>,
        /// Compare-and-swap token: the revision the caller believes is
        /// applied. `None` reconciles unconditionally; a mismatch changes
        /// nothing and fails with `error_kind = "revision_conflict"`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_applied: Option<RevisionIdInfo>,
    },
    Stop,
    Recover,
}

/// Query one operation, optionally long-polling for its terminal state.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreOperationReq<'n> {
    pub operation_id: Cow<'n, str>,
    /// Long-poll bound in milliseconds: the reply returns the moment the
    /// operation reaches a terminal state, or with its current state when the
    /// bound (server-clamped) elapses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum OperationPhase {
    Queued,
    Running,
    Succeeded,
    Failed,
}

/// How a reconcile transaction carried the change. `RolledBack` is a
/// *successfully completed* transaction whose desired config did not take
/// effect; `revision` is then the one actually running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ReconcileOutcomeKind {
    /// Nothing was running; the desired runtime was started cold.
    Started,
    Noop,
    Patched,
    Reloaded,
    Restarted,
    Switched,
    RolledBack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct ReconcileOutcomeInfo {
    pub outcome: ReconcileOutcomeKind,
    /// The revision the core is running now.
    pub revision: ConfigRevisionInfo,
    /// Durability or degradation warnings the transaction survived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// Why the desired config was rejected; set only for `RolledBack`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failed_apply: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationOutputInfo {
    Reconciled(ReconcileOutcomeInfo),
    Stopped,
    Recovered,
    /// Never produced through this wire (there is no shutdown command); kept
    /// so a locally submitted shutdown observed through the registry decodes.
    ShutDown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct OperationErrorInfo {
    /// The wire spelling of the [`CoreErrorKind`](crate::api::CoreErrorKind)
    /// classification, absent when the failure is unclassified — never
    /// guessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<Cow<'static, str>>,
    pub message: String,
    /// Whether resubmitting the same envelope can plausibly succeed.
    pub retryable: bool,
}

/// One operation as the registry sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct OperationInfo {
    pub id: String,
    pub phase: OperationPhase,
    /// Present exactly when `phase == succeeded`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OperationOutputInfo>,
    /// Present exactly when `phase == failed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<OperationErrorInfo>,
}

pub type CoreSubmitRes<'a> = R<'a, OperationInfo>;
pub type CoreOperationRes<'a> = R<'a, OperationInfo>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reconcile_submit_roundtrips_with_every_optional_present() {
        let request = CoreSubmitReq {
            operation_id: Cow::Borrowed("00112233445566778899aabbccddeeff"),
            command: CoreCommandInfo::Reconcile {
                core_type: Cow::Owned(nyanpasu_utils::core::CoreType::Clash(
                    nyanpasu_utils::core::ClashCoreType::Mihomo,
                )),
                config: Cow::Borrowed("external-controller: 127.0.0.1:9090\n"),
                expected_digest: Some(Cow::Borrowed("cbf29ce484222325")),
                expected_applied: Some(RevisionIdInfo {
                    epoch: 3,
                    generation: 7,
                    effective_hash: "fedcba9876543210".into(),
                }),
            },
        };
        let encoded = serde_json::to_string(&request).unwrap();
        assert!(encoded.contains("\"type\":\"reconcile\""));
        let decoded: CoreSubmitReq<'_> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.operation_id, request.operation_id);
        assert!(matches!(decoded.command, CoreCommandInfo::Reconcile { .. }));
    }

    #[test]
    fn bare_commands_and_optionals_stay_off_the_wire() {
        let request = CoreSubmitReq {
            operation_id: Cow::Borrowed("00112233445566778899aabbccddeeff"),
            command: CoreCommandInfo::Stop,
        };
        let encoded = serde_json::to_string(&request).unwrap();
        assert!(encoded.contains("\"type\":\"stop\""));
        assert!(!encoded.contains("expected_digest"));

        let query = CoreOperationReq {
            operation_id: Cow::Borrowed("00112233445566778899aabbccddeeff"),
            wait_ms: None,
        };
        assert!(!serde_json::to_string(&query).unwrap().contains("wait_ms"));
    }

    #[test]
    fn operation_info_roundtrips_in_both_terminal_shapes() {
        let succeeded = OperationInfo {
            id: "00112233445566778899aabbccddeeff".into(),
            phase: OperationPhase::Succeeded,
            output: Some(OperationOutputInfo::Reconciled(ReconcileOutcomeInfo {
                outcome: ReconcileOutcomeKind::Started,
                revision: ConfigRevisionInfo {
                    epoch: 1,
                    generation: 1,
                    source_hash: "0123456789abcdef".into(),
                    effective_hash: "fedcba9876543210".into(),
                },
                warning: None,
                failed_apply: None,
            })),
            error: None,
        };
        let encoded = serde_json::to_string(&succeeded).unwrap();
        assert!(encoded.contains("\"outcome\":\"started\""));
        assert_eq!(
            serde_json::from_str::<OperationInfo>(&encoded).unwrap(),
            succeeded
        );

        let failed = OperationInfo {
            id: "00112233445566778899aabbccddeeff".into(),
            phase: OperationPhase::Failed,
            output: None,
            error: Some(OperationErrorInfo {
                kind: Some(Cow::Borrowed("quarantined")),
                message: "manager is quarantined".into(),
                retryable: false,
            }),
        };
        let encoded = serde_json::to_string(&failed).unwrap();
        assert_eq!(
            serde_json::from_str::<OperationInfo>(&encoded).unwrap(),
            failed
        );
    }
}
