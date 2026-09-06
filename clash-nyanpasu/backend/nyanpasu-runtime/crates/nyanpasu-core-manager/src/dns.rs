//! Host DNS override as an orchestrator-owned component (amendment A5 ③):
//! applied and restored at fixed phases of the core lifecycle transaction,
//! never by the app process.
//!
//! Invariants (audit §3):
//!
//! - The record is persisted *before* the side effect, so a crash between the
//!   two leaves an orphan record whose restore is a read-back no-op.
//! - Apply and restore both advance by read-back: an `Err` from the platform
//!   command never implies the side effect is absent.
//! - The manager converges DNS at the tail of every mutating transaction
//!   (apply when a runtime is up and wants an override, restore otherwise) and
//!   restores at the head of user-initiated stop/shutdown, so resolution never
//!   points at a core that is being torn down.

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Mapping;

use crate::{Epoch, runtime::BoxFuture};

/// The override one host should hold while a given effective config runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsIntent {
    /// Interface-scoped resolver addresses, in platform-native spelling.
    pub servers: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsOverrideState {
    /// Recorded, side effect in flight or in place.
    Applied,
    /// Restore started but not yet read back as clean.
    RestorePending,
}

/// The persisted proof-of-ownership for one applied override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsOverrideRecord {
    pub interface: String,
    /// Read-back before the apply; restore returns the interface here.
    pub previous: Vec<String>,
    pub applied: Vec<String>,
    pub runtime_epoch: u64,
    /// `ControllerGeneration` when a host layer supplies one; opaque here.
    pub owner_generation: Option<u64>,
    pub state: DnsOverrideState,
}

#[derive(Debug, thiserror::Error)]
pub enum DnsError {
    #[error("dns command failed: {0}")]
    Command(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Platform DNS mechanics behind one narrow boundary. A host without DNS
/// responsibility simply injects no controller.
///
/// **Contract limit (audit L8).** This shape supports *structurally owned*
/// overrides only: ones whose restore removes an artifact the controller owns
/// and therefore does not replay `record.previous`. The orchestrator persists
/// its pre-record before the first `apply`, and on that first call the record
/// has no baseline yet — nothing has read one back. A write-back controller
/// (`networksetup -setdnsservers` and friends), whose restore *is* replaying
/// the remembered value, would be unrecoverable in exactly that window: side
/// effect landed, `apply` never returned, nothing recorded to write back.
///
/// Adding one therefore requires splitting `apply` into prepare (read back the
/// baseline, hand it to the orchestrator to persist) and commit, which is the
/// L8 migration. Until then, do not implement this trait with a write-back
/// mechanism.
///
/// Each call is bounded by
/// [`ManagerOptions::dns_timeout`](crate::spec::ManagerOptions::dns_timeout).
/// An implementation must therefore be cancel-safe in the weak sense the
/// orchestrator relies on: a dropped future may leave the side effect in place,
/// and it is treated as uncertain rather than absent.
pub trait DnsController: Send + Sync {
    /// The override this host should hold while `effective` runs, or `None`.
    ///
    /// The derivation is controller policy, not orchestrator policy — the
    /// orchestrator only knows the fixed phases.
    fn desired(&self, effective: &Mapping) -> Option<DnsIntent>;

    /// Applies `intent` and reads the result back. Must be idempotent: the
    /// converge tail calls this after every mutating transaction. Returns the
    /// record to persist (with `previous` captured by read-back).
    fn apply<'a>(
        &'a self,
        intent: &'a DnsIntent,
        runtime_epoch: Epoch,
    ) -> BoxFuture<'a, Result<DnsOverrideRecord, DnsError>>;

    /// Restores `record.previous` and reads the result back. Also the orphan
    /// reconcile path at manager construction — restore of a record whose
    /// side effect never landed must read back as an immediate no-op.
    fn restore<'a>(&'a self, record: &'a DnsOverrideRecord) -> BoxFuture<'a, Result<(), DnsError>>;
}

/// macOS implementation sketch. **Unverified on this branch** — written on a
/// Windows host where `cfg(target_os = "macos")` code is never compiled or
/// run; the Phase-0 spike on real macOS decides the mechanism and hardens
/// this.
///
/// Mechanism choice (audit §3, design §8): a `scutil` dynamic-store `State:`
/// key expresses *structural ownership* — creating
/// `State:/Network/Service/<id>/DNS` overrides resolution, deleting the key
/// restores it, and the key cannot survive a reboot. That beats
/// `networksetup -setdnsservers`, whose only restore is writing back a
/// remembered value that may have changed underneath.
///
/// Phase-0 acceptance criteria (machine-checkable, run on macOS):
/// 1. after writing the key, `scutil --dns` lists the override as the first
///    resolver;
/// 2. after deleting the key, `scutil --dns` shows the pre-override resolver;
/// 3. after a reboot with the key left in place, the key does not exist.
#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;

    pub struct MacosDnsController {
        /// The dynamic-store key this controller owns, e.g.
        /// `State:/Network/Service/nyanpasu-dns/DNS`.
        store_key: String,
    }

    impl MacosDnsController {
        pub fn new(store_key: String) -> Self {
            Self { store_key }
        }

        async fn scutil(script: String) -> Result<String, DnsError> {
            use tokio::io::AsyncWriteExt;
            let mut child = tokio::process::Command::new("/usr/sbin/scutil")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                // The orchestrator bounds this call and drops the future on
                // timeout; without this the child would outlive it.
                .kill_on_drop(true)
                .spawn()?;
            child
                .stdin
                .as_mut()
                .expect("stdin was piped")
                .write_all(script.as_bytes())
                .await?;
            let output = child.wait_with_output().await?;
            if !output.status.success() {
                return Err(DnsError::Command(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ));
            }
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }

        /// Read-back: does the owned key currently exist with `servers`?
        async fn read_back(&self) -> Result<Option<Vec<String>>, DnsError> {
            let output = Self::scutil(format!("show {}\n", self.store_key)).await?;
            if output.contains("No such key") {
                return Ok(None);
            }
            let servers = output
                .lines()
                .filter_map(|line| {
                    let trimmed = line.trim();
                    trimmed
                        .split_once(':')
                        .filter(|(key, _)| key.trim().chars().all(|c| c.is_ascii_digit()))
                        .map(|(_, value)| value.trim().to_owned())
                })
                .collect();
            Ok(Some(servers))
        }
    }

    impl DnsController for MacosDnsController {
        fn desired(&self, effective: &Mapping) -> Option<DnsIntent> {
            // Placeholder product rule, finalized with the app wiring (PR-D):
            // only a DNS server the OS can actually reach on port 53 is worth
            // pointing the system at.
            let dns = effective.get("dns")?.as_mapping()?;
            if !dns.get("enable")?.as_bool()? {
                return None;
            }
            let listen = dns.get("listen")?.as_str()?;
            let port = listen.rsplit_once(':')?.1;
            (port == "53").then(|| DnsIntent {
                servers: vec!["127.0.0.1".to_owned()],
            })
        }

        fn apply<'a>(
            &'a self,
            intent: &'a DnsIntent,
            runtime_epoch: Epoch,
        ) -> BoxFuture<'a, Result<DnsOverrideRecord, DnsError>> {
            Box::pin(async move {
                let previous = self.read_back().await?.unwrap_or_default();
                let addresses = intent.servers.join(" ");
                Self::scutil(format!(
                    "d.init\nd.add ServerAddresses * {addresses}\nset {}\n",
                    self.store_key
                ))
                .await?;
                let observed = self.read_back().await?;
                if observed.as_deref() != Some(intent.servers.as_slice()) {
                    return Err(DnsError::Command(format!(
                        "read-back mismatch after apply: {observed:?}"
                    )));
                }
                Ok(DnsOverrideRecord {
                    interface: self.store_key.clone(),
                    previous,
                    applied: intent.servers.clone(),
                    runtime_epoch: runtime_epoch.get(),
                    owner_generation: None,
                    state: DnsOverrideState::Applied,
                })
            })
        }

        fn restore<'a>(
            &'a self,
            _record: &'a DnsOverrideRecord,
        ) -> BoxFuture<'a, Result<(), DnsError>> {
            Box::pin(async move {
                // Structural restore: delete the owned key. No remembered
                // value can be stale because nothing is written back.
                Self::scutil(format!("remove {}\n", self.store_key)).await?;
                if self.read_back().await?.is_some() {
                    return Err(DnsError::Command(
                        "read-back still shows the override after restore".into(),
                    ));
                }
                Ok(())
            })
        }
    }
}
