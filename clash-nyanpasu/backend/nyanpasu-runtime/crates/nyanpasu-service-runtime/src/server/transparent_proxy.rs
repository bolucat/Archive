//! Service-owned Linux rules for Mihomo's transparent proxy ports.
//!
//! This module intentionally owns only the `nyanpasu_transparent` nft table and
//! its two policy rules/routes. It never flushes a host firewall ruleset.

use std::{
    io::Write,
    process::{Command, Output, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use nyanpasu_core_manager::{ConfigRevision, CoreState};
use nyanpasu_ipc::api::{
    network::transparent_proxy::{
        NetworkTransparentProxyAppliedScope, NetworkTransparentProxyBypass,
        NetworkTransparentProxyDiagnostics, NetworkTransparentProxyMarks,
        NetworkTransparentProxyMode as Mode, NetworkTransparentProxyObject,
        NetworkTransparentProxyObjectObservation, NetworkTransparentProxyObjectState,
        NetworkTransparentProxyProtocol, NetworkTransparentProxyRequest as Request,
        NetworkTransparentProxyRouting, NetworkTransparentProxyStatus as Status,
    },
    status::RevisionIdInfo,
};
use nyanpasu_utils::core::{ClashCoreType, CoreType};
use tokio::sync::Mutex;

use super::CoreManager;

const ROUTING_MARK: u32 = 0x20000;
const ROUTE_TABLE: &str = "233";
const RULE_PRIORITY: &str = "10000";

trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str], input: Option<&str>) -> Result<Output, String>;
}

struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str], input: Option<&str>) -> Result<Output, String> {
        let mut command = Command::new(program);
        command.env("LC_ALL", "C");
        command.args(args).stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                format!("{program} executable not found: {e}")
            } else {
                format!("could not start {program}: {e}")
            }
        })?;
        if let Some(input) = input {
            child
                .stdin
                .take()
                .ok_or("could not open command stdin")?
                .write_all(input.as_bytes())
                .map_err(|e| e.to_string())?;
        }
        child.wait_with_output().map_err(|e| e.to_string())
    }
}

#[derive(Clone)]
pub(crate) struct TransparentProxy {
    manager: CoreManager,
    inner: Arc<Mutex<Option<Applied>>>,
    runner: Arc<dyn CommandRunner>,
    operation: Arc<Mutex<()>>,
    last_error: Arc<Mutex<Option<String>>>,
    shutting_down: Arc<AtomicBool>,
    recovery_pending: Arc<AtomicBool>,
}

#[derive(Clone)]
struct Applied {
    request: Request,
    revision: RevisionIdInfo,
    cleanup_pending: bool,
}

#[derive(Debug)]
struct ApplyError {
    message: String,
    resources_cleaned: Option<bool>,
}

impl From<String> for ApplyError {
    fn from(message: String) -> Self {
        Self {
            message,
            resources_cleaned: None,
        }
    }
}

impl TransparentProxy {
    pub(crate) fn new(manager: CoreManager) -> Self {
        Self {
            manager,
            inner: Arc::new(Mutex::new(None)),
            runner: Arc::new(SystemCommandRunner),
            operation: Arc::new(Mutex::new(())),
            last_error: Arc::new(Mutex::new(None)),
            shutting_down: Arc::new(AtomicBool::new(false)),
            recovery_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub(crate) async fn status(&self) -> Status {
        let _operation = self.operation.lock().await;
        self.status_locked().await
    }

    /// A read-only snapshot of the applied scope and the kernel objects this
    /// service owns or expects. Serialized with reconcile so the scope and the
    /// observation describe the same instant.
    pub(crate) async fn diagnostics(&self) -> NetworkTransparentProxyDiagnostics {
        let _operation = self.operation.lock().await;
        self.diagnostics_locked().await
    }

    async fn diagnostics_locked(&self) -> NetworkTransparentProxyDiagnostics {
        if !cfg!(target_os = "linux") {
            return empty_diagnostics(false);
        }

        let current = self.manager.runtime_status();
        let applied = self.inner.lock().await.clone();
        let applied_stale = applied
            .as_ref()
            .is_some_and(|state| is_stale(&self.manager, &current, state));
        let recovery_pending = self.recovery_pending.load(Ordering::Acquire);
        let active = applied.as_ref().filter(|_| !applied_stale);
        let scope = active.map(applied_scope);
        let nft = inspect_nft(self.runner.as_ref());
        let policy4 = inspect_policy(self.runner.as_ref(), false);
        let policy6 = inspect_policy(self.runner.as_ref(), true);
        let observed_owned = nft.is_present() || policy4.is_present() || policy6.is_present();
        let stale = applied_stale || recovery_pending || (active.is_none() && observed_owned);

        NetworkTransparentProxyDiagnostics {
            supported: true,
            applied: scope.clone(),
            stale,
            marks: NetworkTransparentProxyMarks {
                capture_mark: 0x10000,
                capture_mask: 0x10000,
                egress_mark: ROUTING_MARK,
            },
            routing: NetworkTransparentProxyRouting {
                rule_priority: RULE_PRIORITY
                    .parse()
                    .expect("rule priority is a compiled numeric literal"),
                table: ROUTE_TABLE
                    .parse()
                    .expect("route table is a compiled numeric literal"),
            },
            bypass: NetworkTransparentProxyBypass {
                root_uid: scope.as_ref().is_some_and(|scope| scope.local),
                local_destination: scope.is_some(),
                private_reserved: scope.is_some(),
                core_egress_mark: scope.is_some(),
            },
            objects: diagnostic_objects(
                scope.as_ref(),
                applied.as_ref().map(|state| state.request.mode),
                &nft,
                &policy4,
                &policy6,
            ),
        }
    }

    async fn status_locked(&self) -> Status {
        let current = self.manager.runtime_status();
        let applied = self.inner.lock().await.clone();
        let stale = applied
            .as_ref()
            .is_some_and(|state| is_stale(&self.manager, &current, state));
        let error = self.last_error.lock().await.clone().or_else(|| {
            stale
                .then(|| "core revision changed; stale transparent proxy cleanup is pending".into())
        });
        Status {
            supported: cfg!(target_os = "linux"),
            active: applied.is_some() && !stale,
            mode: applied
                .as_ref()
                .filter(|_| !stale)
                .map(|state| state.request.mode),
            revision: applied.filter(|_| !stale).map(|state| state.revision),
            error,
        }
    }

    pub(crate) async fn reconcile(&self, request: Request) -> Result<Status, String> {
        let operation = self.operation.clone();
        let _operation = operation.lock_owned().await;
        let result = self.reconcile_locked(request).await;
        match result {
            Ok(()) => {
                self.recovery_pending.store(false, Ordering::Release);
                *self.last_error.lock().await = None;
                Ok(self.status_locked().await)
            }
            Err(error) => {
                self.remember_error(error.clone()).await;
                Err(error)
            }
        }
    }

    async fn reconcile_locked(&self, request: Request) -> Result<(), String> {
        if request.mode == Mode::Disabled {
            if cfg!(target_os = "linux")
                && let Err(error) = cleanup(self.runner.as_ref())
                && !(error.starts_with("nft executable not found:")
                    && self.inner.lock().await.is_none())
            {
                self.mark_cleanup_pending().await;
                return Err(error);
            }
            *self.inner.lock().await = None;
            self.recovery_pending.store(false, Ordering::Release);
            return Ok(());
        }
        if !cfg!(target_os = "linux") {
            return Err("transparent proxy is supported only on Linux".into());
        }
        if self.shutting_down.load(Ordering::Acquire) {
            return Err("service is shutting down; transparent proxy cannot be enabled".into());
        }
        validate(&request)?;
        let runtime = self.manager.runtime_status();
        let revision = runtime
            .revision
            .as_ref()
            .ok_or("core has no applied revision")?;
        let CoreState::Running { pid, .. } = runtime.state else {
            return Err("Mihomo must be running before transparent proxy can be enabled".into());
        };
        if !matches!(
            self.manager.runtime_core_type(),
            Some(CoreType::Clash(ClashCoreType::Mihomo))
        ) {
            return Err("transparent proxy requires the Mihomo core".into());
        }
        if !same_revision(revision, &request.expected_revision) {
            return Err("core revision changed; refresh status and retry".into());
        }
        let effective = self
            .manager
            .effective_config()
            .await
            .map_err(|error| error.to_string())?
            .ok_or("effective core config is unavailable")?;
        if effective.revision.id() != request.expected_revision {
            return Err("effective config revision changed; refresh status and retry".into());
        }
        verify_ports(&effective.config, request.mode, request.port)?;
        verify_listener(
            self.runner.as_ref(),
            request.port,
            request.mode,
            pid,
            request.ipv6,
            !request.interfaces.is_empty(),
        )?;
        if let Err(error) = reconcile_linux(self.runner.as_ref(), &request) {
            if error.resources_cleaned == Some(true) {
                *self.inner.lock().await = None;
            } else if error.resources_cleaned == Some(false) {
                *self.inner.lock().await = Some(Applied {
                    revision: request.expected_revision.clone(),
                    request: request.clone(),
                    cleanup_pending: true,
                });
            }
            return Err(error.message);
        }
        let current = self.manager.runtime_status();
        if !matches!(current.state, CoreState::Running { .. })
            || self.shutting_down.load(Ordering::Acquire)
            || !matches!(
                self.manager.runtime_core_type(),
                Some(CoreType::Clash(ClashCoreType::Mihomo))
            )
            || current
                .revision
                .as_ref()
                .is_none_or(|r| !same_revision(r, &request.expected_revision))
        {
            match cleanup(self.runner.as_ref()) {
                Ok(()) => *self.inner.lock().await = None,
                Err(error) => {
                    *self.inner.lock().await = Some(Applied {
                        revision: request.expected_revision.clone(),
                        request: request.clone(),
                        cleanup_pending: true,
                    });
                    return Err(format!(
                        "core revision changed while applying transparent proxy rules; cleanup incomplete: {error}"
                    ));
                }
            }
            return Err(
                "core revision changed or shutdown began while applying transparent proxy rules"
                    .into(),
            );
        }
        let revision = request.expected_revision.clone();
        *self.inner.lock().await = Some(Applied {
            request,
            revision,
            cleanup_pending: false,
        });
        Ok(())
    }

    pub(crate) async fn cleanup(&self) -> Result<(), String> {
        let _operation = self.operation.lock().await;
        if !cfg!(target_os = "linux") {
            *self.inner.lock().await = None;
            return Ok(());
        }
        if let Err(error) = cleanup(self.runner.as_ref()) {
            if error.starts_with("nft executable not found:")
                && self.inner.lock().await.is_none()
                && !self.recovery_pending.load(Ordering::Acquire)
            {
                *self.last_error.lock().await = None;
                return Ok(());
            }
            self.mark_cleanup_pending().await;
            self.remember_error(error.clone()).await;
            return Err(error);
        }
        *self.inner.lock().await = None;
        *self.last_error.lock().await = None;
        self.recovery_pending.store(false, Ordering::Release);
        Ok(())
    }

    pub(crate) async fn recover(&self) -> Result<(), String> {
        if !cfg!(target_os = "linux") {
            return Ok(());
        }
        let result = match self.runner.run(
            "nft",
            &["list", "table", "inet", "nyanpasu_transparent"],
            None,
        ) {
            Ok(output) if !output.status.success() && missing_resource(&output.stderr) => Ok(()),
            Err(error) if error.starts_with("nft executable not found:") => Ok(()),
            Ok(output) if output.status.success() => self.cleanup().await,
            Ok(output) => Err(format!(
                "could not inspect the transparent proxy owner table: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )),
            Err(error) => Err(error),
        };
        if let Err(error) = &result {
            self.recovery_pending.store(true, Ordering::Release);
            self.remember_error(error.clone()).await;
        }
        result
    }

    pub(crate) async fn cleanup_if_stale(&self) -> Result<(), String> {
        let _operation = self.operation.lock().await;
        let current = self.manager.runtime_status();
        let applied = self.inner.lock().await.clone();
        if !self.recovery_pending.load(Ordering::Acquire)
            && !applied.is_some_and(|applied| is_stale(&self.manager, &current, &applied))
        {
            return Ok(());
        }
        if let Err(error) = cleanup(self.runner.as_ref()) {
            self.mark_cleanup_pending().await;
            self.remember_error(error.clone()).await;
            return Err(error);
        }
        *self.inner.lock().await = None;
        *self.last_error.lock().await = None;
        self.recovery_pending.store(false, Ordering::Release);
        Ok(())
    }

    pub(crate) fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
    }

    async fn mark_cleanup_pending(&self) {
        if let Some(applied) = self.inner.lock().await.as_mut() {
            applied.cleanup_pending = true;
        } else {
            self.recovery_pending.store(true, Ordering::Release);
        }
    }

    async fn remember_error(&self, error: String) {
        *self.last_error.lock().await = Some(error);
    }
}

fn empty_diagnostics(supported: bool) -> NetworkTransparentProxyDiagnostics {
    NetworkTransparentProxyDiagnostics {
        supported,
        applied: None,
        stale: false,
        marks: NetworkTransparentProxyMarks {
            capture_mark: 0x10000,
            capture_mask: 0x10000,
            egress_mark: ROUTING_MARK,
        },
        routing: NetworkTransparentProxyRouting {
            rule_priority: RULE_PRIORITY
                .parse()
                .expect("rule priority is a compiled numeric literal"),
            table: ROUTE_TABLE
                .parse()
                .expect("route table is a compiled numeric literal"),
        },
        bypass: NetworkTransparentProxyBypass {
            root_uid: false,
            local_destination: false,
            private_reserved: false,
            core_egress_mark: false,
        },
        objects: Vec::new(),
    }
}

fn applied_scope(applied: &Applied) -> NetworkTransparentProxyAppliedScope {
    let request = &applied.request;
    let mut protocols = vec![NetworkTransparentProxyProtocol::Tcp];
    if request.mode == Mode::Tproxy {
        protocols.push(NetworkTransparentProxyProtocol::Udp);
    }
    NetworkTransparentProxyAppliedScope {
        mode: request.mode,
        port: request.port,
        local: request.local,
        interfaces: request.interfaces.clone(),
        ipv6: request.ipv6,
        protocols,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    V4,
    V6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureAction {
    CaptureMark,
    Tproxy,
    Redirect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NftRule {
    chain: String,
    family: Option<Family>,
    action: Option<CaptureAction>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NftContent {
    chains: std::collections::HashSet<String>,
    rules: Vec<NftRule>,
}

impl NftContent {
    fn has_chain(&self, chain: &str) -> bool {
        self.chains.contains(chain)
    }

    fn has_capture(&self, chain: &str, family: Family, action: CaptureAction) -> bool {
        self.rules.iter().any(|rule| {
            rule.chain == chain && rule.family == Some(family) && rule.action == Some(action)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NftObservation {
    Absent,
    Owned(NftContent),
    Foreign,
    ReadError,
}

impl NftObservation {
    fn is_present(&self) -> bool {
        matches!(self, Self::Owned(_))
    }

    fn object_state(&self) -> NetworkTransparentProxyObjectState {
        match self {
            Self::Absent => NetworkTransparentProxyObjectState::Absent,
            Self::Owned(_) => NetworkTransparentProxyObjectState::Present,
            Self::Foreign => NetworkTransparentProxyObjectState::Foreign,
            Self::ReadError => NetworkTransparentProxyObjectState::ReadError,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyEntryObservation {
    Absent,
    Present,
    Foreign,
    ReadError,
}

impl PolicyEntryObservation {
    fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }

    fn object_state(self) -> NetworkTransparentProxyObjectState {
        match self {
            Self::Absent => NetworkTransparentProxyObjectState::Absent,
            Self::Present => NetworkTransparentProxyObjectState::Present,
            Self::Foreign => NetworkTransparentProxyObjectState::Foreign,
            Self::ReadError => NetworkTransparentProxyObjectState::ReadError,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PolicyFamilyObservation {
    rule: PolicyEntryObservation,
    route: PolicyEntryObservation,
}

impl PolicyFamilyObservation {
    fn is_present(self) -> bool {
        self.rule.is_present() || self.route.is_present()
    }
}

fn inspect_nft(runner: &dyn CommandRunner) -> NftObservation {
    let output = match runner.run(
        "nft",
        &["-j", "list", "table", "inet", "nyanpasu_transparent"],
        None,
    ) {
        Ok(output) => output,
        Err(_) => return NftObservation::ReadError,
    };
    if !output.status.success() {
        return if missing_resource(&output.stderr) {
            NftObservation::Absent
        } else {
            NftObservation::ReadError
        };
    }
    parse_nft_diagnostics(&output.stdout)
}

fn parse_nft_diagnostics(stdout: &[u8]) -> NftObservation {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(stdout) else {
        return NftObservation::ReadError;
    };
    let Some(items) = value.get("nftables").and_then(serde_json::Value::as_array) else {
        return NftObservation::ReadError;
    };
    let table = items.iter().find_map(|item| {
        let table = item.get("table")?;
        let family = table.get("family")?.as_str()?;
        let name = table.get("name")?.as_str()?;
        (family == "inet" && name == "nyanpasu_transparent").then_some(table)
    });
    let Some(table) = table else {
        return NftObservation::Foreign;
    };
    let owned = table
        .get("comment")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|comment| comment.contains("nyanpasu-service transparent-proxy"));
    if !owned {
        return NftObservation::Foreign;
    }

    let mut content = NftContent::default();
    for item in items {
        if let Some(chain) = item.get("chain")
            && let Some(name) = chain.get("name").and_then(serde_json::Value::as_str)
        {
            content.chains.insert(name.to_owned());
        }
        if let Some(rule) = item.get("rule") {
            let Some(chain) = rule.get("chain").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let Some(expr) = rule.get("expr").and_then(serde_json::Value::as_array) else {
                continue;
            };
            content.rules.push(NftRule {
                chain: chain.to_owned(),
                family: rule_family(expr),
                action: rule_action(expr),
            });
        }
    }
    NftObservation::Owned(content)
}

fn rule_family(expr: &[serde_json::Value]) -> Option<Family> {
    expr.iter().find_map(|item| {
        let matched = item.get("match")?;
        let key = matched
            .pointer("/left/meta/key")
            .and_then(serde_json::Value::as_str)?;
        if key != "nfproto" {
            return None;
        }
        match matched.get("right").and_then(serde_json::Value::as_str) {
            Some("ipv4") => Some(Family::V4),
            Some("ipv6") => Some(Family::V6),
            _ => None,
        }
    })
}

fn rule_action(expr: &[serde_json::Value]) -> Option<CaptureAction> {
    let mut capture_mark = false;
    for item in expr {
        if item.get("tproxy").is_some() {
            return Some(CaptureAction::Tproxy);
        }
        if item.get("redirect").is_some() {
            return Some(CaptureAction::Redirect);
        }
        if item.get("mangle").is_some()
            && serde_json::to_string(item).is_ok_and(|item| item.contains("65536"))
        {
            capture_mark = true;
        }
    }
    capture_mark.then_some(CaptureAction::CaptureMark)
}

fn inspect_policy(runner: &dyn CommandRunner, ipv6: bool) -> PolicyFamilyObservation {
    let family = ipv6.then_some("-6");
    let mut rule_args = vec!["-j"];
    rule_args.extend(family);
    rule_args.extend(["rule", "show"]);
    let rule = inspect_policy_rule(runner, &rule_args);

    let mut route_args = vec!["-j"];
    route_args.extend(family);
    route_args.extend(["route", "show", "table", ROUTE_TABLE]);
    let route = inspect_policy_route(runner, &route_args);

    PolicyFamilyObservation { rule, route }
}

fn inspect_policy_rule(runner: &dyn CommandRunner, args: &[&str]) -> PolicyEntryObservation {
    let output = match runner.run("ip", args, None) {
        Ok(output) => output,
        Err(_) => return PolicyEntryObservation::ReadError,
    };
    if !output.status.success() {
        return if missing_resource(&output.stderr) {
            PolicyEntryObservation::Absent
        } else {
            PolicyEntryObservation::ReadError
        };
    }
    let Ok(rules) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return PolicyEntryObservation::ReadError;
    };
    let Some(rules) = rules.as_array() else {
        return PolicyEntryObservation::ReadError;
    };
    let mut found = false;
    for rule in rules {
        if rule.get("priority").and_then(serde_json::Value::as_u64) != Some(10_000) {
            continue;
        }
        found = true;
        if !owned_policy_rule(rule) {
            return PolicyEntryObservation::Foreign;
        }
    }
    if found {
        PolicyEntryObservation::Present
    } else {
        PolicyEntryObservation::Absent
    }
}

fn owned_policy_rule(rule: &serde_json::Value) -> bool {
    rule.get("src").and_then(serde_json::Value::as_str) == Some("all")
        && table_value(rule.get("table")) == Some(ROUTE_TABLE.parse().expect("numeric literal"))
        && rule.get("fwmark").and_then(mark_value) == Some(0x10000)
        && rule.get("fwmask").and_then(mark_value) == Some(0x10000)
}

fn table_value(value: Option<&serde_json::Value>) -> Option<u32> {
    let value = value?;
    if let Some(value) = value.as_u64() {
        return u32::try_from(value).ok();
    }
    value.as_str()?.parse().ok()
}

fn mark_value(value: &serde_json::Value) -> Option<u32> {
    if let Some(value) = value.as_u64() {
        return u32::try_from(value).ok();
    }
    let value = value.as_str()?;
    if let Some(value) = value.strip_prefix("0x") {
        u32::from_str_radix(value, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn inspect_policy_route(runner: &dyn CommandRunner, args: &[&str]) -> PolicyEntryObservation {
    let output = match runner.run("ip", args, None) {
        Ok(output) => output,
        Err(_) => return PolicyEntryObservation::ReadError,
    };
    if !output.status.success() {
        return if missing_resource(&output.stderr) {
            PolicyEntryObservation::Absent
        } else {
            PolicyEntryObservation::ReadError
        };
    }
    let Ok(routes) = serde_json::from_slice::<serde_json::Value>(&output.stdout) else {
        return PolicyEntryObservation::ReadError;
    };
    let Some(routes) = routes.as_array() else {
        return PolicyEntryObservation::ReadError;
    };
    if routes.is_empty() {
        return PolicyEntryObservation::Absent;
    }
    if routes.iter().all(owned_policy_route) {
        PolicyEntryObservation::Present
    } else {
        PolicyEntryObservation::Foreign
    }
}

fn owned_policy_route(route: &serde_json::Value) -> bool {
    route.get("type").and_then(serde_json::Value::as_str) == Some("local")
        && route.get("dst").and_then(serde_json::Value::as_str) == Some("default")
        && route.get("dev").and_then(serde_json::Value::as_str) == Some("lo")
}

fn diagnostic_objects(
    scope: Option<&NetworkTransparentProxyAppliedScope>,
    applied_mode: Option<Mode>,
    nft: &NftObservation,
    policy4: &PolicyFamilyObservation,
    policy6: &PolicyFamilyObservation,
) -> Vec<NetworkTransparentProxyObjectObservation> {
    use NetworkTransparentProxyObject as Object;

    let mut objects = Vec::new();
    objects.push(object_observation(
        Object::NftTable,
        scope.is_some(),
        nft.object_state(),
    ));

    let expected_output = scope.is_some_and(|scope| scope.local);
    let expected_prerouting =
        scope.is_some_and(|scope| scope.mode == Mode::Tproxy || !scope.interfaces.is_empty());
    objects.push(object_observation(
        Object::NftOutputChain,
        expected_output,
        nft_chain_state(nft, "output"),
    ));
    objects.push(object_observation(
        Object::NftPreroutingChain,
        expected_prerouting,
        nft_chain_state(nft, "prerouting"),
    ));

    objects.push(capture_observation(
        Object::NftOutputIpv4Capture,
        scope,
        "output",
        Family::V4,
        true,
        nft,
    ));
    objects.push(capture_observation(
        Object::NftOutputIpv6Capture,
        scope,
        "output",
        Family::V6,
        true,
        nft,
    ));
    objects.push(capture_observation(
        Object::NftPreroutingIpv4Capture,
        scope,
        "prerouting",
        Family::V4,
        false,
        nft,
    ));
    objects.push(capture_observation(
        Object::NftPreroutingIpv6Capture,
        scope,
        "prerouting",
        Family::V6,
        false,
        nft,
    ));

    let tproxy = scope.is_some_and(|scope| scope.mode == Mode::Tproxy);
    let ipv6 = scope.is_some_and(|scope| scope.ipv6);
    let routes_not_applicable = applied_mode == Some(Mode::Redir);
    objects.push(policy_observation(
        Object::PolicyRuleIpv4,
        tproxy,
        policy4.rule,
        routes_not_applicable,
    ));
    objects.push(policy_observation(
        Object::PolicyRuleIpv6,
        tproxy && ipv6,
        policy6.rule,
        routes_not_applicable,
    ));
    objects.push(policy_observation(
        Object::PolicyRouteIpv4,
        tproxy,
        policy4.route,
        routes_not_applicable,
    ));
    objects.push(policy_observation(
        Object::PolicyRouteIpv6,
        tproxy && ipv6,
        policy6.route,
        routes_not_applicable,
    ));
    objects
}

fn object_observation(
    object: NetworkTransparentProxyObject,
    expected: bool,
    state: NetworkTransparentProxyObjectState,
) -> NetworkTransparentProxyObjectObservation {
    NetworkTransparentProxyObjectObservation {
        object,
        expected,
        state,
    }
}

fn nft_chain_state(nft: &NftObservation, chain: &str) -> NetworkTransparentProxyObjectState {
    match nft {
        NftObservation::Owned(content) if content.has_chain(chain) => {
            NetworkTransparentProxyObjectState::Present
        }
        NftObservation::Owned(_) => NetworkTransparentProxyObjectState::Absent,
        _ => nft.object_state(),
    }
}

fn capture_observation(
    object: NetworkTransparentProxyObject,
    scope: Option<&NetworkTransparentProxyAppliedScope>,
    chain: &str,
    family: Family,
    output: bool,
    nft: &NftObservation,
) -> NetworkTransparentProxyObjectObservation {
    let expected = scope.is_some_and(|scope| {
        if output && !scope.local {
            return false;
        }
        if !output && scope.mode == Mode::Redir && scope.interfaces.is_empty() {
            return false;
        }
        if family == Family::V6 && !scope.ipv6 {
            return false;
        }
        match scope.mode {
            Mode::Tproxy | Mode::Redir => true,
            Mode::Disabled => false,
        }
    });
    let state = match nft {
        NftObservation::Owned(content) => {
            let actions: &[CaptureAction] = match scope.map(|scope| scope.mode) {
                Some(Mode::Tproxy) if output => &[CaptureAction::CaptureMark],
                Some(Mode::Tproxy) => &[CaptureAction::Tproxy],
                Some(Mode::Redir) => &[CaptureAction::Redirect],
                Some(Mode::Disabled) | None => &[
                    CaptureAction::CaptureMark,
                    CaptureAction::Tproxy,
                    CaptureAction::Redirect,
                ],
            };
            if content.has_chain(chain)
                && actions
                    .iter()
                    .any(|action| content.has_capture(chain, family, *action))
            {
                NetworkTransparentProxyObjectState::Present
            } else {
                NetworkTransparentProxyObjectState::Absent
            }
        }
        _ => nft.object_state(),
    };
    object_observation(object, expected, state)
}

fn policy_observation(
    object: NetworkTransparentProxyObject,
    expected: bool,
    observed: PolicyEntryObservation,
    not_applicable: bool,
) -> NetworkTransparentProxyObjectObservation {
    let state = if not_applicable || (observed == PolicyEntryObservation::Absent && !expected) {
        NetworkTransparentProxyObjectState::NotApplicable
    } else {
        observed.object_state()
    };
    object_observation(object, expected, state)
}

fn is_stale(
    manager: &CoreManager,
    status: &nyanpasu_core_manager::CoreStatus,
    applied: &Applied,
) -> bool {
    applied.cleanup_pending
        || !matches!(status.state, CoreState::Running { .. })
        || !matches!(
            manager.runtime_core_type(),
            Some(CoreType::Clash(ClashCoreType::Mihomo))
        )
        || status
            .revision
            .as_ref()
            .is_none_or(|revision| !same_revision(revision, &applied.revision))
}

fn same_revision(revision: &ConfigRevision, expected: &RevisionIdInfo) -> bool {
    revision.epoch.get() == expected.epoch
        && revision.generation == expected.generation
        && revision.effective_hash == expected.effective_hash
}

fn validate(request: &Request) -> Result<(), String> {
    if request.port == 0 {
        return Err("transparent proxy port must be nonzero".into());
    }
    if !request.local && request.interfaces.is_empty() {
        return Err("gateway mode requires at least one interface".into());
    }
    if request.interfaces.iter().any(|iface| {
        iface.is_empty()
            || iface.len() > 15
            || !iface
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    }) {
        return Err("invalid network interface name".into());
    }
    if request.interfaces.len() > 16 {
        return Err("at most 16 network interfaces may be selected".into());
    }
    let mut seen = std::collections::HashSet::new();
    if request.interfaces.iter().any(|iface| !seen.insert(iface)) {
        return Err("network interface names must be unique".into());
    }
    Ok(())
}

fn verify_ports(config: &str, mode: Mode, port: u16) -> Result<(), String> {
    let value: yaml_serde::Value = yaml_serde::from_str(config).map_err(|e| e.to_string())?;
    if value
        .get("tun")
        .and_then(|tun| tun.get("enable"))
        .and_then(yaml_serde::Value::as_bool)
        == Some(true)
    {
        return Err("disable TUN before enabling transparent capture".into());
    }
    let key = match mode {
        Mode::Redir => "redir-port",
        Mode::Tproxy => "tproxy-port",
        Mode::Disabled => return Ok(()),
    };
    let actual = value
        .as_mapping()
        .and_then(|map| map.get(yaml_serde::Value::String(key.into())))
        .and_then(yaml_serde::Value::as_u64);
    if actual != Some(port as u64) {
        return Err(format!(
            "effective Mihomo {key} does not match requested port {port}"
        ));
    }
    for other in [
        "port",
        "socks-port",
        "mixed-port",
        "redir-port",
        "tproxy-port",
    ] {
        if other != key && value.get(other).and_then(yaml_serde::Value::as_u64) == Some(port as u64)
        {
            return Err(format!(
                "effective {other} conflicts with {key} on port {port}"
            ));
        }
    }
    if value
        .get("listeners")
        .and_then(yaml_serde::Value::as_sequence)
        .is_some_and(|listeners| {
            listeners.iter().any(|listener| {
                listener.get("port").and_then(yaml_serde::Value::as_u64) == Some(port as u64)
            })
        })
    {
        return Err(format!(
            "a named listener conflicts with {key} on port {port}"
        ));
    }
    let mark = value
        .as_mapping()
        .and_then(|map| map.get(yaml_serde::Value::String("routing-mark".into())))
        .and_then(yaml_serde::Value::as_u64);
    if mark != Some(ROUTING_MARK as u64) {
        return Err("effective Mihomo routing-mark must be 0x20000".into());
    }
    Ok(())
}

fn verify_listener(
    runner: &dyn CommandRunner,
    port: u16,
    mode: Mode,
    pid: u32,
    ipv6: bool,
    gateway: bool,
) -> Result<(), String> {
    let filter = format!("sport = :{port}");
    let tcp = runner.run("ss", &["-H", "-l", "-n", "-t", "-p", &filter], None)?;
    if !tcp.status.success() {
        return Err(format!(
            "could not verify the Mihomo TCP listener: {}",
            String::from_utf8_lossy(&tcp.stderr).trim()
        ));
    }
    verify_listener_addresses(&tcp.stdout, port, pid, ipv6, gateway)?;
    if mode == Mode::Tproxy {
        let udp = runner.run("ss", &["-H", "-l", "-n", "-u", "-p", &filter], None)?;
        if !udp.status.success() {
            return Err(format!(
                "could not verify the Mihomo UDP listener: {}",
                String::from_utf8_lossy(&udp.stderr).trim()
            ));
        }
        verify_listener_addresses(&udp.stdout, port, pid, ipv6, gateway)?;
    }
    Ok(())
}

fn verify_listener_addresses(
    output: &[u8],
    port: u16,
    pid: u32,
    ipv6: bool,
    gateway: bool,
) -> Result<(), String> {
    let output = String::from_utf8_lossy(output);
    let expected_pid = format!("pid={pid},");
    let addresses: Vec<_> = output
        .lines()
        .filter(|line| line.contains(&expected_pid))
        .filter_map(|line| line.split_whitespace().nth(3))
        .collect();
    for (family, wildcard, loopback) in
        [("IPv4", "0.0.0.0", "127.0.0.1"), ("IPv6", "[::]", "[::1]")]
            .into_iter()
            .take(if ipv6 { 2 } else { 1 })
    {
        let any = format!("*:{port}");
        let wildcard = format!("{wildcard}:{port}");
        let loopback = format!("{loopback}:{port}");
        if !addresses.iter().any(|address| {
            *address == any || *address == wildcard || (!gateway && *address == loopback)
        }) {
            return Err(format!(
                "Mihomo process {pid} needs a {family} {} listener on port {port}; check allow-lan and bind-address",
                if gateway {
                    "wildcard"
                } else {
                    "wildcard or loopback"
                }
            ));
        }
    }
    Ok(())
}

fn run(
    runner: &dyn CommandRunner,
    program: &str,
    args: &[&str],
    input: Option<&str>,
) -> Result<(), String> {
    let output = runner.run(program, args, input)?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn reconcile_linux(runner: &dyn CommandRunner, request: &Request) -> Result<(), ApplyError> {
    // Use a private table and replace only that table. On failure the best-effort
    // cleanup removes our routes and table without touching other firewall owners.
    let existing = runner.run(
        "nft",
        &["list", "table", "inet", "nyanpasu_transparent"],
        None,
    )?;
    let table_exists = existing.status.success();
    if !table_exists && !missing_resource(&existing.stderr) {
        return Err(format!(
            "could not inspect the owned nft table: {}",
            String::from_utf8_lossy(&existing.stderr).trim()
        )
        .into());
    }
    if table_exists
        && !String::from_utf8_lossy(&existing.stdout).contains("nyanpasu-service transparent-proxy")
    {
        return Err(
            "nft table inet nyanpasu_transparent already exists and is not service-owned"
                .to_owned()
                .into(),
        );
    }
    for iface in &request.interfaces {
        let link = runner.run("ip", &["link", "show", "dev", iface], None)?;
        if !link.status.success() {
            return Err(
                format!("network interface {iface} does not exist or is unavailable").into(),
            );
        }
    }
    check_routing_ownership(runner, false, table_exists)?;
    check_routing_ownership(runner, true, table_exists)?;
    if table_exists {
        cleanup(runner).map_err(|message| ApplyError {
            message,
            resources_cleaned: Some(false),
        })?;
    }
    run(
        runner,
        "nft",
        &["-f", "-"],
        Some(
            "table inet nyanpasu_transparent { comment \"nyanpasu-service transparent-proxy\"; }\n",
        ),
    )
    .map_err(|message| ApplyError {
        message,
        resources_cleaned: Some(true),
    })?;
    let script = format!(
        "delete table inet nyanpasu_transparent\n{}",
        nft_script(request)
    );
    let result = (|| -> Result<(), String> {
        if request.mode == Mode::Tproxy {
            run(
                runner,
                "ip",
                &[
                    "rule",
                    "add",
                    "fwmark",
                    "0x10000/0x10000",
                    "priority",
                    RULE_PRIORITY,
                    "lookup",
                    ROUTE_TABLE,
                ],
                None,
            )?;
            run(
                runner,
                "ip",
                &[
                    "route",
                    "replace",
                    "local",
                    "default",
                    "dev",
                    "lo",
                    "table",
                    ROUTE_TABLE,
                ],
                None,
            )?;
            if request.ipv6 {
                run(
                    runner,
                    "ip",
                    &[
                        "-6",
                        "rule",
                        "add",
                        "fwmark",
                        "0x10000/0x10000",
                        "priority",
                        RULE_PRIORITY,
                        "lookup",
                        ROUTE_TABLE,
                    ],
                    None,
                )?;
                run(
                    runner,
                    "ip",
                    &[
                        "-6",
                        "route",
                        "replace",
                        "local",
                        "default",
                        "dev",
                        "lo",
                        "table",
                        ROUTE_TABLE,
                    ],
                    None,
                )?;
            }
        }
        run(runner, "nft", &["-f", "-"], Some(&script))
    })();
    if let Err(error) = result {
        let rollback = cleanup(runner);
        return Err(ApplyError {
            message: match &rollback {
                Ok(()) => error,
                Err(cleanup_error) => format!("{error}; cleanup incomplete: {cleanup_error}"),
            },
            resources_cleaned: Some(rollback.is_ok()),
        });
    }
    Ok(())
}

fn nft_script(request: &Request) -> String {
    let mut s = String::from(
        "table inet nyanpasu_transparent { comment \"nyanpasu-service transparent-proxy\"\n",
    );
    if request.local && request.mode == Mode::Tproxy {
        s.push_str(" chain output { type route hook output priority mangle; meta skuid 0 return; meta mark & 0x20000 != 0 return; fib daddr type local return; ");
        s.push_str(&tproxy_rules(request, true));
        s.push_str("}\n");
    }
    if request.mode == Mode::Tproxy {
        s.push_str(" chain prerouting { type filter hook prerouting priority mangle; meta mark & 0x20000 != 0 return; fib daddr type local return; ");
        s.push_str(&tproxy_rules(request, false));
        s.push_str("}\n");
    } else {
        if request.local {
            s.push_str(" chain output { type nat hook output priority dstnat; meta skuid 0 return; meta mark & 0x20000 != 0 return; fib daddr type local return; ");
            s.push_str(&redir_rules(request, true));
            s.push_str("}\n");
        }
        if !request.interfaces.is_empty() {
            s.push_str(" chain prerouting { type nat hook prerouting priority dstnat; meta mark & 0x20000 != 0 return; fib daddr type local return; ");
            s.push_str(&redir_rules(request, false));
            s.push_str("}\n");
        }
    }
    s.push_str("}\n");
    s
}

fn interfaces_expr(request: &Request) -> String {
    format!(
        "iifname {{ {} }}",
        request
            .interfaces
            .iter()
            .map(|i| format!("\"{i}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn ipv4_bypass() -> &'static str {
    "daddr != { 0.0.0.0/8, 10.0.0.0/8, 100.64.0.0/10, 127.0.0.0/8, 169.254.0.0/16, 172.16.0.0/12, 192.0.0.0/24, 192.0.2.0/24, 192.168.0.0/16, 198.19.0.0/16, 198.51.100.0/24, 203.0.113.0/24, 224.0.0.0/4, 240.0.0.0/4 }"
}

fn ipv6_bypass() -> &'static str {
    "daddr != { ::, ::1, ::ffff:0:0/96, 2001:db8::/32, fc00::/7, fe80::/10, ff00::/8 }"
}

fn tproxy_rules(request: &Request, output: bool) -> String {
    let mut rules = String::new();
    for family in ["ipv4", "ipv6"]
        .into_iter()
        .take(if request.ipv6 { 2 } else { 1 })
    {
        let (header, bypass) = if family == "ipv4" {
            ("ip", ipv4_bypass())
        } else {
            ("ip6", ipv6_bypass())
        };
        let prefix = format!("meta nfproto {family} {header} {bypass} ");
        if output {
            rules.push_str(&format!(
                "{prefix}meta l4proto {{ tcp, udp }} meta mark set meta mark | 0x10000; "
            ));
            continue;
        }
        if request.local {
            rules.push_str(&format!("{prefix}iifname \"lo\" meta mark & 0x10000 != 0 meta l4proto {{ tcp, udp }} tproxy {header} to :{} accept; ", request.port));
        }
        if !request.interfaces.is_empty() {
            rules.push_str(&format!("{prefix}{} meta l4proto {{ tcp, udp }} meta mark set meta mark | 0x10000 tproxy {header} to :{} accept; ", interfaces_expr(request), request.port));
        }
    }
    rules
}

fn redir_rules(request: &Request, output: bool) -> String {
    let mut rules = String::new();
    for family in ["ipv4", "ipv6"]
        .into_iter()
        .take(if request.ipv6 { 2 } else { 1 })
    {
        let (header, bypass) = if family == "ipv4" {
            ("ip", ipv4_bypass())
        } else {
            ("ip6", ipv6_bypass())
        };
        let iface = if output || request.interfaces.is_empty() {
            String::new()
        } else {
            format!("{} ", interfaces_expr(request))
        };
        rules.push_str(&format!(
            "meta nfproto {family} {header} {bypass} {iface}meta l4proto tcp redirect to :{}; ",
            request.port
        ));
    }
    rules
}

fn cleanup(runner: &dyn CommandRunner) -> Result<(), String> {
    let table = match runner.run(
        "nft",
        &["list", "table", "inet", "nyanpasu_transparent"],
        None,
    ) {
        Ok(table) => table,
        Err(error) if error.starts_with("nft executable not found:") => return Err(error),
        Err(error) => return Err(error),
    };
    if !table.status.success() && !missing_resource(&table.stderr) {
        return Err(format!(
            "could not inspect the owned nft table: {}",
            String::from_utf8_lossy(&table.stderr).trim()
        ));
    }
    if table.status.success() {
        if !String::from_utf8_lossy(&table.stdout).contains("nyanpasu-service transparent-proxy") {
            return Err("refusing to remove an nft table not owned by the service".into());
        }
        check_routing_ownership(runner, false, true)?;
        check_routing_ownership(runner, true, true)?;
        run(
            runner,
            "nft",
            &["flush", "table", "inet", "nyanpasu_transparent"],
            None,
        )?;
        cleanup_routing(runner, false)?;
        cleanup_routing(runner, true)?;
        run(
            runner,
            "nft",
            &["delete", "table", "inet", "nyanpasu_transparent"],
            None,
        )?;
        return Ok(());
    }
    Ok(())
}

fn check_routing_ownership(
    runner: &dyn CommandRunner,
    ipv6: bool,
    table_owned: bool,
) -> Result<(), String> {
    let family = if ipv6 { vec!["-6"] } else { vec![] };
    let mut args = vec!["-N"];
    args.extend(family.clone());
    args.extend(["rule", "show"]);
    let all_rules = runner.run("ip", &args, None)?;
    if !all_rules.status.success() {
        return Err("could not inspect policy routing rules".into());
    }
    for line in String::from_utf8_lossy(&all_rules.stdout).lines() {
        if owned_rule(line) {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        let references_table = words.windows(2).any(|pair| pair == ["lookup", ROUTE_TABLE]);
        let mark_conflict = words.windows(2).filter(|pair| pair[0] == "fwmark").any(|pair| {
            let (value, mask) = pair[1].split_once('/').unwrap_or((pair[1], "0xffffffff"));
            let number = |text: &str| u32::from_str_radix(text.trim_start_matches("0x"), if text.starts_with("0x") { 16 } else { 10 }).ok();
            matches!((number(value), number(mask)), (Some(value), Some(mask)) if value & mask & 0x30000 != 0)
        });
        if references_table || mark_conflict {
            return Err(
                "existing policy routing uses the transparent proxy table or mark bits".into(),
            );
        }
    }
    args.extend(["priority", RULE_PRIORITY]);
    let rules = runner.run("ip", &args, None)?;
    if !rules.status.success() {
        return Err(format!(
            "could not inspect policy rules: {}",
            String::from_utf8_lossy(&rules.stderr).trim()
        ));
    }
    let rules = String::from_utf8_lossy(&rules.stdout);
    if !rules.trim().is_empty() && (!table_owned || !rules.lines().all(owned_rule)) {
        return Err(format!(
            "policy routing priority {RULE_PRIORITY} is already in use"
        ));
    }
    let mut args = vec!["-N"];
    args.extend(family);
    args.extend(["route", "show", "table", ROUTE_TABLE]);
    let routes = runner.run("ip", &args, None)?;
    if !routes.status.success() && !missing_resource(&routes.stderr) {
        return Err(format!(
            "could not inspect routing table {ROUTE_TABLE}: {}",
            String::from_utf8_lossy(&routes.stderr).trim()
        ));
    }
    let routes = String::from_utf8_lossy(&routes.stdout);
    if !routes.trim().is_empty() && (!table_owned || !routes.lines().all(owned_local_route)) {
        return Err(format!(
            "policy routing table {ROUTE_TABLE} already contains foreign routes"
        ));
    }
    Ok(())
}

fn owned_local_route(line: &str) -> bool {
    let mut words = line.split_whitespace();
    if !matches!(
        (words.next(), words.next(), words.next(), words.next()),
        (
            Some("local" | "2"),
            Some("default"),
            Some("dev"),
            Some("lo")
        )
    ) {
        return false;
    }
    let tail: Vec<_> = words.collect();
    tail.is_empty()
        || tail == ["scope", "host"]
        || tail == ["scope", "254"]
        || tail == ["metric", "1024", "pref", "medium"]
}

fn owned_rule(line: &str) -> bool {
    let fields: Vec<_> = line.split_whitespace().collect();
    fields.as_slice()
        == [
            "10000:",
            "from",
            "all",
            "fwmark",
            "0x10000/0x10000",
            "lookup",
            "233",
        ]
}

fn missing_resource(stderr: &[u8]) -> bool {
    let error = String::from_utf8_lossy(stderr).to_ascii_lowercase();
    error.contains("no such file or directory")
        || error.contains("does not exist")
        || error.contains("fib table does not exist")
}

fn cleanup_routing(runner: &dyn CommandRunner, ipv6: bool) -> Result<(), String> {
    let family = if ipv6 { vec!["-6"] } else { vec![] };
    let mut args = vec!["-N"];
    args.extend(family.clone());
    args.extend(["route", "show", "table", ROUTE_TABLE]);
    let routes = runner.run("ip", &args, None)?;
    if !routes.status.success() && !missing_resource(&routes.stderr) {
        return Err(format!(
            "could not inspect routing table {ROUTE_TABLE}: {}",
            String::from_utf8_lossy(&routes.stderr).trim()
        ));
    }
    if String::from_utf8_lossy(&routes.stdout)
        .lines()
        .any(owned_local_route)
    {
        let mut args = family.clone();
        args.extend([
            "route",
            "del",
            "local",
            "default",
            "dev",
            "lo",
            "table",
            ROUTE_TABLE,
        ]);
        run(runner, "ip", &args, None)?;
    }
    let mut args = vec!["-N"];
    args.extend(family.clone());
    args.extend(["rule", "show", "priority", RULE_PRIORITY]);
    let rules = runner.run("ip", &args, None)?;
    if !rules.status.success() {
        return Err(format!(
            "could not inspect policy rules: {}",
            String::from_utf8_lossy(&rules.stderr).trim()
        ));
    }
    let rules = String::from_utf8_lossy(&rules.stdout);
    if !rules.trim().is_empty() && !rules.lines().all(owned_rule) {
        return Err(format!(
            "refusing to remove foreign policy rules at priority {RULE_PRIORITY}"
        ));
    }
    if rules.lines().any(owned_rule) {
        let mut args = family;
        args.extend([
            "rule",
            "del",
            "fwmark",
            "0x10000/0x10000",
            "priority",
            RULE_PRIORITY,
            "lookup",
            ROUTE_TABLE,
        ]);
        run(runner, "ip", &args, None)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{os::unix::process::ExitStatusExt, sync::Mutex as StdMutex};

    #[derive(Default)]
    struct FakeRunner {
        calls: StdMutex<Vec<String>>,
        state: StdMutex<FakeState>,
        conflict: bool,
        fail_nft_apply: bool,
        fail_cleanup: bool,
    }

    #[derive(Default)]
    struct FakeState {
        table: bool,
        route4: bool,
        route6: bool,
        rule4: bool,
        rule6: bool,
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str], input: Option<&str>) -> Result<Output, String> {
            self.calls.lock().unwrap().push(format!(
                "{program} {} {}",
                args.join(" "),
                input.unwrap_or("")
            ));
            let mut state = self.state.lock().unwrap();
            let is_nft_list = program == "nft" && args.first() == Some(&"list");
            let is_rule_list = program == "ip" && args.contains(&"rule") && args.contains(&"show");
            let ipv6 = args.contains(&"-6");
            let fail_apply = self.fail_nft_apply
                && program == "nft"
                && args.first() == Some(&"-f")
                && input.is_some_and(|text| text.contains("chain prerouting"));
            let stdout = if is_nft_list && state.table {
                b"table inet nyanpasu_transparent { comment \"nyanpasu-service transparent-proxy\" }".to_vec()
            } else if is_rule_list && self.conflict {
                b"10000: from all lookup main\n".to_vec()
            } else if is_rule_list && (if ipv6 { state.rule6 } else { state.rule4 }) {
                b"10000: from all fwmark 0x10000/0x10000 lookup 233\n".to_vec()
            } else if program == "ip"
                && args.contains(&"table")
                && (if ipv6 { state.route6 } else { state.route4 })
            {
                b"local default dev lo scope host\n".to_vec()
            } else {
                Vec::new()
            };
            if program == "nft"
                && args.first() == Some(&"-f")
                && !fail_apply
                && input.is_some_and(|text| text.contains("nyanpasu_transparent"))
            {
                state.table = true;
            }
            if program == "nft" && args.first() == Some(&"delete") {
                state.table = false;
            }
            if program == "ip" && args.contains(&"rule") {
                if args.contains(&"add") {
                    if ipv6 {
                        state.rule6 = true
                    } else {
                        state.rule4 = true
                    }
                }
                if args.contains(&"del") {
                    if ipv6 {
                        state.rule6 = false
                    } else {
                        state.rule4 = false
                    }
                }
            }
            if program == "ip" && args.contains(&"route") {
                if args.contains(&"replace") {
                    if ipv6 {
                        state.route6 = true
                    } else {
                        state.route4 = true
                    }
                }
                if args.contains(&"del") {
                    if ipv6 {
                        state.route6 = false
                    } else {
                        state.route4 = false
                    }
                }
            }
            let fail_cleanup =
                self.fail_cleanup && program == "nft" && args.first() == Some(&"flush");
            let fail = (is_nft_list && !state.table) || fail_apply || fail_cleanup;
            let stderr = if fail_apply || fail_cleanup {
                b"synthetic failure".to_vec()
            } else if is_nft_list && !state.table {
                b"No such file or directory".to_vec()
            } else {
                Vec::new()
            };
            Ok(Output {
                status: std::process::ExitStatus::from_raw(if fail { 1 << 8 } else { 0 }),
                stdout,
                stderr,
            })
        }
    }

    fn request(mode: Mode, local: bool, interfaces: Vec<String>, ipv6: bool) -> Request {
        Request {
            mode,
            port: 7894,
            local,
            interfaces,
            ipv6,
            expected_revision: RevisionIdInfo {
                epoch: 1,
                generation: 7,
                effective_hash: "hash".into(),
            },
        }
    }

    fn state_of(
        objects: &[NetworkTransparentProxyObjectObservation],
        object: NetworkTransparentProxyObject,
    ) -> NetworkTransparentProxyObjectObservation {
        *objects
            .iter()
            .find(|observation| observation.object == object)
            .expect("diagnostics must report every managed object")
    }

    fn owned_nft() -> NftObservation {
        NftObservation::Owned(NftContent {
            chains: ["output", "prerouting"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            rules: vec![
                NftRule {
                    chain: "output".into(),
                    family: Some(Family::V4),
                    action: Some(CaptureAction::CaptureMark),
                },
                NftRule {
                    chain: "output".into(),
                    family: Some(Family::V6),
                    action: Some(CaptureAction::CaptureMark),
                },
                NftRule {
                    chain: "prerouting".into(),
                    family: Some(Family::V4),
                    action: Some(CaptureAction::Tproxy),
                },
                NftRule {
                    chain: "prerouting".into(),
                    family: Some(Family::V6),
                    action: Some(CaptureAction::Tproxy),
                },
            ],
        })
    }

    #[test]
    fn diagnostics_matching_scope_reports_every_expected_object_present() {
        let applied = Applied {
            request: request(Mode::Tproxy, true, vec!["eth0".into()], true),
            revision: RevisionIdInfo {
                epoch: 1,
                generation: 7,
                effective_hash: "hash".into(),
            },
            cleanup_pending: false,
        };
        let scope = applied_scope(&applied);
        let policy = PolicyFamilyObservation {
            rule: PolicyEntryObservation::Present,
            route: PolicyEntryObservation::Present,
        };
        let objects = diagnostic_objects(
            Some(&scope),
            Some(Mode::Tproxy),
            &owned_nft(),
            &policy,
            &policy,
        );

        assert!(
            objects
                .iter()
                .filter(|object| object.expected)
                .all(|object| { object.state == NetworkTransparentProxyObjectState::Present })
        );
        assert_eq!(
            scope.protocols,
            [
                NetworkTransparentProxyProtocol::Tcp,
                NetworkTransparentProxyProtocol::Udp
            ]
        );
    }

    #[test]
    fn diagnostics_absent_scope_reports_absent_and_not_applicable() {
        let policy = PolicyFamilyObservation {
            rule: PolicyEntryObservation::Absent,
            route: PolicyEntryObservation::Absent,
        };
        let objects = diagnostic_objects(None, None, &NftObservation::Absent, &policy, &policy);

        assert!(!objects.iter().any(|object| object.expected));
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::NftTable).state,
            NetworkTransparentProxyObjectState::Absent
        );
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::PolicyRuleIpv4).state,
            NetworkTransparentProxyObjectState::NotApplicable
        );
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::PolicyRouteIpv6).state,
            NetworkTransparentProxyObjectState::NotApplicable
        );
    }

    #[test]
    fn diagnostics_foreign_objects_are_not_reported_as_owned() {
        let applied = Applied {
            request: request(Mode::Tproxy, true, vec!["eth0".into()], false),
            revision: RevisionIdInfo {
                epoch: 1,
                generation: 7,
                effective_hash: "hash".into(),
            },
            cleanup_pending: false,
        };
        let scope = applied_scope(&applied);
        let policy = PolicyFamilyObservation {
            rule: PolicyEntryObservation::Foreign,
            route: PolicyEntryObservation::Foreign,
        };
        let objects = diagnostic_objects(
            Some(&scope),
            Some(Mode::Tproxy),
            &NftObservation::Foreign,
            &policy,
            &policy,
        );

        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::NftTable).state,
            NetworkTransparentProxyObjectState::Foreign
        );
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::NftOutputChain).state,
            NetworkTransparentProxyObjectState::Foreign
        );
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::PolicyRuleIpv4).state,
            NetworkTransparentProxyObjectState::Foreign
        );
    }

    #[test]
    fn diagnostics_partial_scope_reports_missing_ipv6_and_policy_entries() {
        let applied = Applied {
            request: request(Mode::Tproxy, true, vec!["eth0".into()], true),
            revision: RevisionIdInfo {
                epoch: 1,
                generation: 7,
                effective_hash: "hash".into(),
            },
            cleanup_pending: false,
        };
        let scope = applied_scope(&applied);
        let partial_nft = NftObservation::Owned(NftContent {
            chains: ["output", "prerouting"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            rules: vec![
                NftRule {
                    chain: "output".into(),
                    family: Some(Family::V4),
                    action: Some(CaptureAction::CaptureMark),
                },
                NftRule {
                    chain: "prerouting".into(),
                    family: Some(Family::V4),
                    action: Some(CaptureAction::Tproxy),
                },
            ],
        });
        let policy4 = PolicyFamilyObservation {
            rule: PolicyEntryObservation::Present,
            route: PolicyEntryObservation::Absent,
        };
        let policy6 = PolicyFamilyObservation {
            rule: PolicyEntryObservation::Absent,
            route: PolicyEntryObservation::Absent,
        };
        let objects = diagnostic_objects(
            Some(&scope),
            Some(Mode::Tproxy),
            &partial_nft,
            &policy4,
            &policy6,
        );

        let output_v6 = state_of(
            &objects,
            NetworkTransparentProxyObject::NftOutputIpv6Capture,
        );
        assert!(output_v6.expected);
        assert_eq!(output_v6.state, NetworkTransparentProxyObjectState::Absent);
        let route4 = state_of(&objects, NetworkTransparentProxyObject::PolicyRouteIpv4);
        assert!(route4.expected);
        assert_eq!(route4.state, NetworkTransparentProxyObjectState::Absent);
        let rule6 = state_of(&objects, NetworkTransparentProxyObject::PolicyRuleIpv6);
        assert!(rule6.expected);
        assert_eq!(rule6.state, NetworkTransparentProxyObjectState::Absent);
    }

    #[test]
    fn diagnostics_read_errors_are_distinct_from_absent_resources() {
        let applied = Applied {
            request: request(Mode::Redir, false, vec!["br-lan".into()], false),
            revision: RevisionIdInfo {
                epoch: 1,
                generation: 7,
                effective_hash: "hash".into(),
            },
            cleanup_pending: false,
        };
        let scope = applied_scope(&applied);
        let policy = PolicyFamilyObservation {
            rule: PolicyEntryObservation::ReadError,
            route: PolicyEntryObservation::ReadError,
        };
        let objects = diagnostic_objects(
            Some(&scope),
            Some(Mode::Redir),
            &NftObservation::ReadError,
            &policy,
            &policy,
        );

        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::NftTable).state,
            NetworkTransparentProxyObjectState::ReadError
        );
        assert_eq!(
            state_of(
                &objects,
                NetworkTransparentProxyObject::NftPreroutingIpv4Capture
            )
            .state,
            NetworkTransparentProxyObjectState::ReadError
        );
        assert_eq!(
            state_of(&objects, NetworkTransparentProxyObject::PolicyRuleIpv4).state,
            NetworkTransparentProxyObjectState::NotApplicable,
            "REDIRECT routing stays N/A even when the probe failed"
        );
    }

    #[test]
    fn diagnostics_parse_nft_json_family_and_actions() {
        let output = br#"{"nftables":[{"metainfo":{"json_schema_version":1}},{"table":{"family":"inet","name":"nyanpasu_transparent","comment":"nyanpasu-service transparent-proxy"}},{"chain":{"family":"inet","table":"nyanpasu_transparent","name":"output","type":"route","hook":"output","prio":-150}},{"rule":{"family":"inet","table":"nyanpasu_transparent","chain":"output","expr":[{"match":{"op":"==","left":{"meta":{"key":"nfproto"}},"right":"ipv4"}},{"match":{"op":"==","left":{"meta":{"key":"l4proto"}},"right":{"set":["tcp","udp"]}}},{"mangle":{"key":{"meta":{"key":"mark"}},"value":{"|":[{"meta":{"key":"mark"}},65536]}}}]}},{"chain":{"family":"inet","table":"nyanpasu_transparent","name":"prerouting","type":"nat","hook":"prerouting","prio":-100}},{"rule":{"family":"inet","table":"nyanpasu_transparent","chain":"prerouting","expr":[{"match":{"op":"==","left":{"meta":{"key":"nfproto"}},"right":"ipv6"}},{"match":{"op":"==","left":{"meta":{"key":"l4proto"}},"right":"tcp"}},{"redirect":{"port":7894}}]}}]}"#;

        let nft = parse_nft_diagnostics(output);
        let NftObservation::Owned(content) = nft else {
            panic!("owned service table must parse");
        };
        assert!(content.has_chain("output"));
        assert!(content.has_chain("prerouting"));
        assert!(content.has_capture("output", Family::V4, CaptureAction::CaptureMark));
        assert!(content.has_capture("prerouting", Family::V6, CaptureAction::Redirect));
    }

    /// Run only in a disposable network namespace with nftables and iproute2.
    #[test]
    #[ignore = "requires a disposable privileged network namespace"]
    #[cfg(target_os = "linux")]
    fn linux_kernel_installs_and_removes_capture_rules() {
        let runner = SystemCommandRunner;
        run(&runner, "ip", &["link", "set", "lo", "up"], None).unwrap();
        for (mode, ipv6) in [
            (Mode::Redir, false),
            (Mode::Tproxy, false),
            (Mode::Redir, true),
            (Mode::Tproxy, true),
        ] {
            let desired = request(mode, true, Vec::new(), ipv6);
            reconcile_linux(&runner, &desired).expect("install capture rules");
            let table = runner
                .run(
                    "nft",
                    &["list", "table", "inet", "nyanpasu_transparent"],
                    None,
                )
                .unwrap();
            assert!(table.status.success());
            cleanup(&runner).expect("remove capture rules");
            let table = runner
                .run(
                    "nft",
                    &["list", "table", "inet", "nyanpasu_transparent"],
                    None,
                )
                .unwrap();
            assert!(!table.status.success());
        }
    }

    #[test]
    fn nft_plan_combines_local_and_gateway_capture_and_gates_ipv6() {
        let script = nft_script(&request(Mode::Tproxy, true, vec!["eth0".into()], false));
        assert!(script.contains("iifname \"lo\" meta mark & 0x10000"));
        assert!(script.contains("iifname { \"eth0\" }"));
        assert!(script.contains("tproxy ip to :7894"));
        assert!(!script.contains("tproxy ip6 to"));
        assert!(script.contains("198.19.0.0/16"));
        assert!(!script.contains("198.18.0.0/16"));
    }

    #[test]
    fn redir_plan_captures_local_output_and_selected_interfaces() {
        let script = nft_script(&request(Mode::Redir, true, vec!["br-lan".into()], true));
        assert!(script.contains("chain output { type nat hook output"));
        assert!(script.contains("iifname { \"br-lan\" }"));
        assert!(script.contains("meta nfproto ipv4"));
        assert!(script.contains("meta nfproto ipv6"));
    }

    #[test]
    fn conflicting_priority_is_rejected_before_any_mutation() {
        let runner = FakeRunner {
            conflict: true,
            ..FakeRunner::default()
        };
        let result = reconcile_linux(&runner, &request(Mode::Tproxy, true, vec![], false));
        assert!(result.unwrap_err().message.contains("priority"));
        let calls = runner.calls.lock().unwrap();
        assert!(
            !calls
                .iter()
                .any(|call| call.contains("rule add") || call.contains("-f -"))
        );
    }

    #[test]
    fn failed_nft_apply_runs_compensating_cleanup() {
        let runner = FakeRunner {
            fail_nft_apply: true,
            ..FakeRunner::default()
        };
        assert!(reconcile_linux(&runner, &request(Mode::Tproxy, true, vec![], false)).is_err());
        let calls = runner.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call.contains("rule del")));
        assert!(calls.iter().any(|call| call.contains("route del")));
    }
    #[test]
    fn matching_foreign_policy_rule_requires_an_ownership_marker() {
        let runner = FakeRunner::default();
        runner.state.lock().unwrap().rule4 = true;
        let error =
            reconcile_linux(&runner, &request(Mode::Tproxy, true, vec![], false)).unwrap_err();
        assert!(error.message.contains("priority"));
        assert!(runner.state.lock().unwrap().rule4);
        assert!(
            !runner
                .calls
                .lock()
                .unwrap()
                .iter()
                .any(|call| call.contains("rule del"))
        );
    }

    #[test]
    fn cleanup_without_marker_preserves_foreign_resources() {
        let runner = FakeRunner::default();
        {
            let mut state = runner.state.lock().unwrap();
            state.route4 = true;
            state.rule4 = true;
        }
        cleanup(&runner).unwrap();
        let state = runner.state.lock().unwrap();
        assert!(state.route4 && state.rule4);
    }

    #[test]
    fn failed_compensation_retains_owner_marker_and_diagnostic() {
        let runner = FakeRunner {
            fail_nft_apply: true,
            fail_cleanup: true,
            ..Default::default()
        };
        let error =
            reconcile_linux(&runner, &request(Mode::Tproxy, true, vec![], true)).unwrap_err();
        assert_eq!(error.resources_cleaned, Some(false));
        assert!(error.message.contains("cleanup incomplete"));
        assert!(runner.state.lock().unwrap().table);
    }

    #[test]
    fn replacing_dual_stack_tproxy_with_redir_removes_policy_routes() {
        let runner = FakeRunner::default();
        reconcile_linux(&runner, &request(Mode::Tproxy, true, vec![], true)).unwrap();
        {
            let state = runner.state.lock().unwrap();
            assert!(state.rule4 && state.rule6 && state.route4 && state.route6);
        }
        reconcile_linux(&runner, &request(Mode::Redir, true, vec![], false)).unwrap();
        let state = runner.state.lock().unwrap();
        assert!(state.table);
        assert!(!state.rule4 && !state.rule6 && !state.route4 && !state.route6);
    }

    #[test]
    fn gateway_requires_wildcard_and_ipv6_requires_a_matching_family() {
        let loopback = b"LISTEN 0 128 127.0.0.1:7894 0.0.0.0:* users:((\"mihomo\",pid=42,fd=3))";
        assert!(verify_listener_addresses(loopback, 7894, 42, false, false).is_ok());
        assert!(verify_listener_addresses(loopback, 7894, 42, false, true).is_err());
        assert!(verify_listener_addresses(loopback, 7894, 42, true, false).is_err());
        let dual = b"LISTEN 0 128 *:7894 *:* users:((\"mihomo\",pid=42,fd=3))";
        assert!(verify_listener_addresses(dual, 7894, 42, true, true).is_ok());
        assert!(verify_listener_addresses(dual, 7894, 41, true, true).is_err());
    }

    #[test]
    fn effective_configuration_must_match_listener_mark_and_tun_policy() {
        assert!(
            verify_ports(
                "tproxy-port: 7894\nrouting-mark: 131072\n",
                Mode::Tproxy,
                7894
            )
            .is_ok()
        );
        assert!(
            verify_ports(
                "redir-port: 7894\nrouting-mark: 131072\n",
                Mode::Redir,
                7894
            )
            .is_ok()
        );
        assert!(
            verify_ports(
                "tproxy-port: 7894\nrouting-mark: 131072\ntun: { enable: true }\n",
                Mode::Tproxy,
                7894
            )
            .is_err()
        );
        assert!(
            verify_ports(
                "tproxy-port: 7893\nrouting-mark: 131072\n",
                Mode::Tproxy,
                7894
            )
            .is_err()
        );
        assert!(verify_ports("tproxy-port: 7894\n", Mode::Tproxy, 7894).is_err());
        assert!(
            verify_ports(
                "redir-port: 7894\ntproxy-port: 7894\nrouting-mark: 131072\n",
                Mode::Redir,
                7894
            )
            .is_err()
        );
        assert!(verify_ports("tproxy-port: 7894\nrouting-mark: 131072\nlisteners: [{ name: other, type: tproxy, port: 7894 }]\n", Mode::Tproxy, 7894).is_err());
    }
}
