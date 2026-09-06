//! Immutable config snapshots and deterministic effective documents.
//!
//! This module owns the schema-agnostic pipeline — read, canonicalize, hash,
//! serialize. Every rule that names a YAML key lives in a per-core module:
//! [`clash`] for the controller vocabulary shared by all supported kinds, and
//! [`mihomo`] for the Mihomo-only capabilities layered on top.

mod clash;
mod diff;
pub(crate) mod mihomo;
pub mod runtime_store;

use camino::{Utf8Path, Utf8PathBuf};
use enumset::EnumSet;
use serde_yaml_ng::{Mapping, Value};

pub(crate) use clash::LOCAL_TRANSPORT_FEATURE;

use crate::{Epoch, capability::RuntimeFeature, error::Error, spec::ResolvedController};

#[derive(Debug, Clone)]
pub(crate) struct ConfigSnapshot {
    source_path: Utf8PathBuf,
    document: Mapping,
    source_hash: String,
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedConfig {
    pub bytes: Vec<u8>,
    pub document: Mapping,
    pub controller: ResolvedController,
    pub rewrote_controller: bool,
    pub source_hash: String,
    pub effective_hash: String,
}

#[derive(Debug)]
pub(crate) struct ConfigInfo {
    pub controller: Option<RawController>,
    pub secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawController {
    Pipe(String),
    #[cfg_attr(windows, allow(dead_code))]
    Unix(String),
    Http(String),
}

impl ConfigSnapshot {
    /// Reads and parses the user config exactly once for one manager operation.
    pub(crate) async fn load(source_path: &Utf8Path) -> Result<Self, Error> {
        let raw = tokio::fs::read(source_path).await?;
        Self::from_bytes(source_path.to_owned(), &raw)
    }

    fn from_bytes(source_path: Utf8PathBuf, raw: &[u8]) -> Result<Self, Error> {
        let value: Value = serde_yaml_ng::from_slice(raw)?;
        let Value::Mapping(document) = canonicalize(value)? else {
            return Err(Error::InvalidConfig(
                "top-level YAML document must be a mapping".into(),
            ));
        };
        let canonical = serialize_mapping(&document)?;
        Ok(Self {
            source_path,
            document,
            source_hash: semantic_hash(&canonical),
        })
    }

    pub(crate) fn source_path(&self) -> &Utf8Path {
        &self.source_path
    }

    pub(crate) fn document(&self) -> &Mapping {
        &self.document
    }

    #[cfg(test)]
    pub(crate) fn info(&self) -> ConfigInfo {
        clash::inspect(&self.document)
    }

    pub(crate) fn prepare_full(
        &self,
        controller_template: Option<&str>,
        runtime_dir: &Utf8Path,
        epoch: Epoch,
        runtime: EnumSet<RuntimeFeature>,
    ) -> Result<PreparedConfig, Error> {
        self.prepare(controller_template, runtime_dir, epoch, runtime, false)
    }

    pub(crate) fn prepare_bootstrap(
        &self,
        controller_template: Option<&str>,
        runtime_dir: &Utf8Path,
        epoch: Epoch,
        runtime: EnumSet<RuntimeFeature>,
    ) -> Result<PreparedConfig, Error> {
        self.prepare(controller_template, runtime_dir, epoch, runtime, true)
    }

    fn prepare(
        &self,
        controller_template: Option<&str>,
        runtime_dir: &Utf8Path,
        epoch: Epoch,
        runtime: EnumSet<RuntimeFeature>,
        zero_inbounds: bool,
    ) -> Result<PreparedConfig, Error> {
        let mut document = self.document.clone();

        if zero_inbounds {
            mihomo::zero_inbounds(&mut document);
        }

        // `RuntimeFeature::LocalIpc` is the single source of truth: the policy
        // was already weighed in `capability::resolve_runtime`. Without it the
        // source config's controller stays authoritative, and only
        // `external-controller` is read — a user-declared pipe or socket path
        // is not a channel this manager can own across epochs.
        let rewrote_controller = runtime.contains(RuntimeFeature::LocalIpc);
        if rewrote_controller {
            let endpoint = managed_endpoint_path(runtime_dir, controller_template, epoch)?;
            clash::rewrite_managed_controller(&mut document, endpoint);
        }

        let Value::Mapping(document) = canonicalize(Value::Mapping(document))? else {
            unreachable!("canonical mapping remains a mapping")
        };
        let info = if rewrote_controller {
            clash::inspect(&document)
        } else {
            clash::inspect_http(&document)
        };
        let controller = resolve_controller(&info)?;
        let bytes = serialize_mapping(&document)?;
        Ok(PreparedConfig {
            effective_hash: semantic_hash(&bytes),
            source_hash: self.source_hash.clone(),
            bytes,
            document,
            controller,
            rewrote_controller,
        })
    }
}

fn canonicalize(value: Value) -> Result<Value, Error> {
    match value {
        Value::Mapping(mapping) => {
            let mut entries = Vec::with_capacity(mapping.len());
            for (key, value) in mapping {
                let Value::String(key) = key else {
                    return Err(Error::InvalidConfig(
                        "all YAML mapping keys must be strings".into(),
                    ));
                };
                entries.push((key, canonicalize(value)?));
            }
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = Mapping::new();
            for (key, value) in entries {
                canonical.insert(Value::String(key), value);
            }
            Ok(Value::Mapping(canonical))
        }
        Value::Sequence(sequence) => sequence
            .into_iter()
            .map(canonicalize)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Sequence),
        Value::Tagged(_) => Err(Error::InvalidConfig(
            "tagged YAML values are not supported in runtime configs".into(),
        )),
        scalar => Ok(scalar),
    }
}

fn serialize_mapping(document: &Mapping) -> Result<Vec<u8>, Error> {
    Ok(serde_yaml_ng::to_string(document)?.into_bytes())
}

fn semantic_hash(bytes: &[u8]) -> String {
    // Stable FNV-1a over canonical YAML; this is a change identity, not a
    // cryptographic integrity primitive.
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn str_value(doc: &Mapping, key: &str) -> Option<String> {
    doc.get(Value::String(key.to_owned()))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .filter(|value| !value.is_empty())
}

pub(crate) fn resolve_controller(info: &ConfigInfo) -> Result<ResolvedController, Error> {
    let raw = info.controller.as_ref().ok_or(Error::ControllerMissing)?;
    let host = match raw {
        RawController::Pipe(path) => clash_api::Host::named_pipe(path),
        RawController::Unix(path) => clash_api::Host::unix_socket(path),
        RawController::Http(address) => clash_api::Host::http(probe_address(address))?,
    };
    Ok(ResolvedController {
        host,
        secret: info.secret.clone(),
    })
}

fn probe_address(address: &str) -> String {
    match address.rsplit_once(':') {
        Some(("0.0.0.0" | "::" | "[::]" | "", port)) => format!("127.0.0.1:{port}"),
        _ => address.to_owned(),
    }
}

pub(crate) fn validate_controller_template(template: Option<&str>) -> Result<(), Error> {
    if template.is_some_and(|value| !value.contains("{epoch}")) {
        return Err(Error::InvalidManagerOptions(
            "managed controller_template must contain `{epoch}`".into(),
        ));
    }
    Ok(())
}

/// Manager construction validates the controller template before any epoch
/// exists. A template may put `{epoch}` in a parent component, and the
/// rendered parent is canonicalized and checked for containment, so which
/// number renders is observable: `runtime/0/` and `runtime/1/` need not both
/// exist, nor resolve through the same symlink. Zero therefore stays the probe
/// rendering it has always been — a number, never promoted to an `Epoch`.
const TEMPLATE_PROBE_RENDERING: u64 = 0;

pub(crate) fn validate_managed_endpoint(
    runtime_dir: &Utf8Path,
    template: Option<&str>,
) -> Result<(), Error> {
    render_endpoint_path(runtime_dir, template, TEMPLATE_PROBE_RENDERING).map(|_| ())
}

pub(crate) fn managed_endpoint_path(
    runtime_dir: &Utf8Path,
    template: Option<&str>,
    epoch: Epoch,
) -> Result<String, Error> {
    render_endpoint_path(runtime_dir, template, epoch.get())
}

fn render_endpoint_path(
    runtime_dir: &Utf8Path,
    template: Option<&str>,
    epoch: u64,
) -> Result<String, Error> {
    validate_controller_template(template)?;
    if let Some(template) = template {
        let endpoint = template.replace("{epoch}", &epoch.to_string());
        #[cfg(windows)]
        return Ok(endpoint);
        #[cfg(unix)]
        return managed_unix_endpoint(runtime_dir, &endpoint);
    }
    #[cfg(windows)]
    {
        let _ = runtime_dir;
        Ok(format!(r"\\.\pipe\nyanpasu\core-{epoch}"))
    }
    #[cfg(not(windows))]
    {
        Ok(runtime_dir.join(format!("core-{epoch}.sock")).to_string())
    }
}

#[cfg(unix)]
fn managed_unix_endpoint(runtime_dir: &Utf8Path, endpoint: &str) -> Result<String, Error> {
    let endpoint = Utf8Path::new(endpoint);
    let candidate = if endpoint.is_absolute() {
        endpoint.to_owned()
    } else {
        runtime_dir.join(endpoint)
    };
    let parent = candidate.parent().ok_or_else(|| {
        Error::InvalidManagerOptions("managed Unix controller has no parent directory".into())
    })?;
    let canonical_parent = std::fs::canonicalize(parent).map_err(|error| {
        Error::InvalidManagerOptions(format!(
            "managed Unix controller parent `{parent}` cannot be canonicalized: {error}"
        ))
    })?;
    let canonical_parent = Utf8PathBuf::from_path_buf(canonical_parent).map_err(|_| {
        Error::InvalidManagerOptions("managed Unix controller path is not UTF-8".into())
    })?;
    if !canonical_parent.starts_with(runtime_dir) {
        return Err(Error::InvalidManagerOptions(format!(
            "managed Unix controller `{candidate}` escapes runtime directory `{runtime_dir}`"
        )));
    }
    let file_name = candidate.file_name().ok_or_else(|| {
        Error::InvalidManagerOptions("managed Unix controller must name a socket file".into())
    })?;
    Ok(canonical_parent.join(file_name).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::epoch::epoch;

    fn snapshot(yaml: &str) -> ConfigSnapshot {
        ConfigSnapshot::from_bytes(Utf8PathBuf::from("config.yaml"), yaml.as_bytes()).unwrap()
    }

    #[test]
    fn extracts_http_controller_and_secret() {
        let info = snapshot("external-controller: 127.0.0.1:9090\nsecret: s3cret\n").info();
        assert_eq!(
            info.controller,
            Some(RawController::Http("127.0.0.1:9090".into()))
        );
        assert_eq!(info.secret.as_deref(), Some("s3cret"));
    }

    #[test]
    fn the_http_path_ignores_a_configured_local_controller() {
        #[cfg(windows)]
        let source = snapshot(r"external-controller-pipe: \\.\pipe\source");
        #[cfg(not(windows))]
        let source = snapshot("external-controller-unix: /tmp/source.sock");

        let error = source
            .prepare_full(None, Utf8Path::new("runtime"), epoch(1), EnumSet::new())
            .unwrap_err();

        assert!(matches!(error, Error::ControllerMissing));
    }

    #[test]
    fn semantic_hash_ignores_mapping_order_and_whitespace() {
        let first = snapshot("mode: rule\ndns:\n  enable: true\n  listen: ''\n");
        let second = snapshot("dns: { listen: '', enable: true }\n\nmode: rule\n");
        assert_eq!(first.source_hash, second.source_hash);
    }

    #[test]
    fn non_string_mapping_keys_are_rejected_recursively() {
        let error = ConfigSnapshot::from_bytes(
            Utf8PathBuf::from("config.yaml"),
            b"rules:\n  nested:\n    1: invalid\n",
        )
        .unwrap_err();
        assert!(matches!(error, Error::InvalidConfig(_)));
    }

    #[test]
    fn managed_bootstrap_zeroes_listeners_and_keeps_the_source_snapshot() {
        let source = snapshot(
            "mixed-port: 7890\nexternal-controller: 127.0.0.1:9090\nsecret: sc\ntun:\n  enable: true\n",
        );
        let prepared = source
            .prepare_bootstrap(
                Some(r"\\.\pipe\ny-{epoch}"),
                Utf8Path::new("runtime"),
                epoch(7),
                EnumSet::only(RuntimeFeature::LocalIpc),
            )
            .unwrap();
        assert_eq!(
            prepared
                .document
                .get(Value::String("mixed-port".into()))
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            prepared
                .document
                .get(Value::String("tun".into()))
                .and_then(Value::as_mapping)
                .and_then(|tun| tun.get(Value::String("enable".into())))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(prepared.controller.secret.as_deref(), Some("sc"));
        assert_eq!(
            source.info().controller,
            Some(RawController::Http("127.0.0.1:9090".into()))
        );
    }

    #[test]
    fn endpoint_template_requires_and_substitutes_epoch() {
        let dir = Utf8Path::new("/tmp/x");
        assert!(managed_endpoint_path(dir, Some("fixed"), epoch(1)).is_err());
        #[cfg(windows)]
        assert_eq!(
            managed_endpoint_path(dir, Some(r"\\.\pipe\ny-{epoch}"), epoch(42)).unwrap(),
            r"\\.\pipe\ny-42"
        );
        assert!(
            managed_endpoint_path(dir, None, epoch(42))
                .unwrap()
                .contains("42")
        );
    }

    #[cfg(unix)]
    #[test]
    fn managed_unix_template_must_stay_inside_runtime_directory() {
        let root = tempfile::tempdir().unwrap();
        let runtime = Utf8PathBuf::from_path_buf(root.path().join("runtime")).unwrap();
        std::fs::create_dir(&runtime).unwrap();
        assert!(managed_endpoint_path(&runtime, Some("core-{epoch}.sock"), epoch(4)).is_ok());
        let outside = root.path().join("escaped-{epoch}.sock");
        let outside = outside.to_str().unwrap();
        let error = managed_endpoint_path(&runtime, Some(outside), epoch(4)).unwrap_err();
        assert!(error.to_string().contains("escapes runtime directory"));
    }

    #[cfg(unix)]
    #[test]
    fn managed_unix_template_rejects_escaping_parent_symlink() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let runtime = root.path().join("runtime");
        let outside = root.path().join("outside");
        std::fs::create_dir(&runtime).unwrap();
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, runtime.join("link")).unwrap();
        let runtime = Utf8PathBuf::from_path_buf(runtime.canonicalize().unwrap()).unwrap();
        let template = runtime.join("link/core-{epoch}.sock");

        let error = managed_endpoint_path(&runtime, Some(template.as_str()), epoch(5)).unwrap_err();
        assert!(error.to_string().contains("escapes runtime directory"));
    }
}
