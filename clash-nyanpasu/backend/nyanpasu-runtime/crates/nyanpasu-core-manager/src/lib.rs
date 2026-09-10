//! Clash core lifecycle management: epoch-based instances, health-probed
//! startup, crash recovery, and core switching.
//!
//! Design: docs/superpowers/specs/2026-07-18-nyanpasu-core-manager-design.md

mod capability;
pub mod controller_access;
pub use controller_access::ControllerAccess;
mod config;
pub mod control;
pub mod dns;
mod epoch;
mod error;
mod health;
pub mod instance;
pub mod kind;
mod log;
mod log_sink;
pub mod manager;
pub mod runtime;
pub mod snapshot;
pub mod spec;
pub use snapshot::{ConfigCommitSubscription, EffectiveConfigSnapshot};
pub mod state;

pub use capability::{Feature, RuntimeFeature};
pub use clash_api::Host;
pub use config::runtime_store;
pub use control::{
    CheckRequest, ConfigInput, ControlOptions, CoreCommand, CoreCommandEnvelope, CoreControl,
    CoreError, ExecutorExit, OperationHandle, OperationId, OperationOutput, OperationState,
    ReconcileRequest, payload_digest,
};
pub use dns::{DnsController, DnsError, DnsIntent, DnsOverrideRecord, DnsOverrideState};
pub use epoch::Epoch;
pub use error::{CoreErrorKind, Error};
pub use health::{HealthPolicy, HealthPolicySpec, HealthThresholds, probe};
pub use instance::{Instance, InstanceBuilder};
pub use kind::CoreKind;
pub use log::{LogField, LogFrame, LogLevel, LogStream, LogTimestamp};
pub use manager::{ApplyOutcome, CoreManager, CoreManagerBuilder, DegradeReason, SwitchOutcome};
pub use probe::{
    ControllerVersionProbe, HealthProbe, ProbeContext, ProbeFuture, ProbeHandle, ProbePhase,
    ProbeResult,
};
pub use runtime::{RuntimeBackend, RuntimeInstance, RuntimeLaunchRequest};
pub use runtime_store::{
    RuntimeCommitDurability, RuntimeConfigBackup, RuntimeConfigCommit, RuntimeConfigStore,
    StagedRuntimeConfig,
};
pub use spec::{
    ApiConnection, CoreSpec, InstanceOptions, InstanceSpec, LocalIpcPolicy, LocalIpcSettings,
    ManagerOptions, ResolvedController,
};
pub use state::{
    ConfigRevision, CoreState, CoreStatus, HealthState, HealthStatus, InstanceState,
    InstanceStatus, RevisionId, SpecSummary, StopReason,
};
