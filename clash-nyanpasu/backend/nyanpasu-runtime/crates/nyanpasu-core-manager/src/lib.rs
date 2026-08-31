//! Clash core lifecycle management: epoch-based instances, health-probed
//! startup, crash recovery, and core switching.
//!
//! Design: docs/superpowers/specs/2026-07-18-nyanpasu-core-manager-design.md

mod capability;
mod config;
pub mod control;
pub mod dns;
mod error;
mod health;
pub mod instance;
pub mod kind;
mod log;
mod log_sink;
pub mod manager;
pub mod runtime;
pub mod spec;
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
pub use error::{CoreErrorKind, Error};
pub use health::{HealthPolicy, probe};
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
    CoreSpec, InstanceOptions, InstanceSpec, LocalIpcPolicy, ManagerOptions, ResolvedController,
};
pub use state::{
    ConfigRevision, CoreState, CoreStatus, HealthState, HealthStatus, InstanceState,
    InstanceStatus, RevisionId, SpecSummary, StopReason,
};
