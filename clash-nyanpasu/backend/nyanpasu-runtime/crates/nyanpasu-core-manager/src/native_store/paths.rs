use crate::CoreKind;
use camino::Utf8Path;
use serde_yaml_ng::{Mapping, Value};

/// Rebase only schema-defined filesystem inputs. A proxy's transport `path`
/// is an HTTP path, and inline PEM values are not filesystem references.
pub fn rewrite_paths(kind: CoreKind, document: &mut Mapping, source: &Utf8Path) {
    for field in ["external-ui", "mmdb", "geosite", "asn-mmdb"] {
        rebase(document.get_mut(field), source);
    }
    for field in ["proxy-providers", "rule-providers"] {
        if let Some(Value::Mapping(providers)) = document.get_mut(field) {
            for provider in providers.values_mut().filter_map(Value::as_mapping_mut) {
                if provider.get("type").and_then(Value::as_str) != Some("http") {
                    rebase(provider.get_mut("path"), source);
                }
            }
        }
    }
    for field in ["tls", "tuic-server", "ss-config"] {
        if let Some(Value::Mapping(config)) = document.get_mut(field) {
            certificate_paths(config, source);
        }
    }
    if let Some(Value::Sequence(listeners)) = document.get_mut("listeners") {
        for listener in listeners.iter_mut().filter_map(Value::as_mapping_mut) {
            certificate_paths(listener, source);
        }
    }
    if let Some(Value::Sequence(proxies)) = document.get_mut("proxies") {
        for proxy in proxies.iter_mut().filter_map(Value::as_mapping_mut) {
            // WireGuard's private-key is inline data, unlike SSH/TLS keys.
            match proxy.get("type").and_then(Value::as_str) {
                Some("ssh") => rebase(proxy.get_mut("private-key"), source),
                Some("vmess" | "vless" | "trojan" | "hysteria2" | "tuic" | "anytls") => {
                    certificate_paths(proxy, source);
                    if let Some(realm) = proxy.get_mut("realm-opts").and_then(Value::as_mapping_mut)
                    {
                        certificate_paths(realm, source);
                    }
                    if let Some(download) = proxy
                        .get_mut("xhttp-opts")
                        .and_then(Value::as_mapping_mut)
                        .and_then(|opts| opts.get_mut("download-settings"))
                        .and_then(Value::as_mapping_mut)
                    {
                        certificate_paths(download, source);
                    }
                }
                Some("zerotier") => rebase(proxy.get_mut("planet"), source),
                _ => {}
            }
        }
    }
    if kind == CoreKind::ClashRust {
        if let Some(listen) = document
            .get_mut("dns")
            .and_then(Value::as_mapping_mut)
            .and_then(|dns| dns.get_mut("listen"))
            .and_then(Value::as_mapping_mut)
        {
            for kind in ["doh", "dot", "doh3"] {
                if let Some(config) = listen.get_mut(kind).and_then(Value::as_mapping_mut) {
                    for key in ["ca-cert", "ca-key"] {
                        rebase(config.get_mut(key), source);
                    }
                }
            }
        }
    }
}

fn certificate_paths(map: &mut Mapping, source: &Utf8Path) {
    for key in ["certificate", "private-key"] {
        rebase(map.get_mut(key), source);
    }
    match map.get_mut("client-auth-cert") {
        Some(Value::Sequence(certificates)) => {
            for certificate in certificates {
                rebase(Some(certificate), source);
            }
        }
        certificate => rebase(certificate, source),
    }
}

fn rebase(value: Option<&mut Value>, source: &Utf8Path) {
    let Some(Value::String(path)) = value else {
        return;
    };
    if !path.is_empty()
        && !path.contains('\n')
        && !path.starts_with("-----BEGIN ")
        && !path.contains("://")
        && Utf8Path::new(path).is_relative()
    {
        *path = source.join(&*path).into_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebases_file_inputs_without_rewriting_transport_paths_or_pem() {
        let mut doc: Mapping = serde_yaml_ng::from_str("external-ui: ui\nproxy-providers:\n  p: {type: http, path: providers/p.yaml, url: 'https://example.org/p'}\nproxies:\n  - {type: vmess, ws-opts: {path: /ws}}\ntls:\n  certificate: cert.pem\n  private-key: '-----BEGIN PRIVATE KEY-----'\n").unwrap();
        rewrite_paths(CoreKind::Mihomo, &mut doc, Utf8Path::new("/data"));
        assert_eq!(doc["external-ui"], "/data/ui");
        assert_eq!(doc["proxy-providers"]["p"]["path"], "providers/p.yaml");
        assert_eq!(doc["proxies"][0]["ws-opts"]["path"], "/ws");
        assert_eq!(doc["tls"]["certificate"], "/data/cert.pem");
        assert_eq!(doc["tls"]["private-key"], "-----BEGIN PRIVATE KEY-----");
        let before = doc.clone();
        rewrite_paths(CoreKind::Mihomo, &mut doc, Utf8Path::new("/data"));
        assert_eq!(before, doc);
    }

    #[test]
    fn preserves_file_provider_and_outbound_credentials_when_home_changes() {
        let mut doc: Mapping = serde_yaml_ng::from_str(
            r#"
proxy-providers:
  local: {type: file, path: providers/local.yaml}
tls: {client-auth-cert: client-ca.pem}
proxies:
  - type: vless
    certificate: client.pem
    private-key: client.key
    xhttp-opts:
      path: /tunnel
      download-settings: {certificate: download.pem, private-key: download.key}
  - {type: wireguard, private-key: 'CAsAAABhYmM='}
  - {type: ssh, private-key: ssh.key}
  - type: hysteria2
    realm-opts: {certificate: realm.pem, private-key: realm.key}
  - {type: zerotier, planet: planet.bin}
"#,
        )
        .unwrap();
        rewrite_paths(CoreKind::Mihomo, &mut doc, Utf8Path::new("/data"));
        assert_eq!(
            doc["proxy-providers"]["local"]["path"],
            "/data/providers/local.yaml"
        );
        assert_eq!(doc["tls"]["client-auth-cert"], "/data/client-ca.pem");
        let proxies = &doc["proxies"];
        assert_eq!(proxies[0]["certificate"], "/data/client.pem");
        assert_eq!(proxies[0]["private-key"], "/data/client.key");
        assert_eq!(proxies[0]["xhttp-opts"]["path"], "/tunnel");
        assert_eq!(
            proxies[0]["xhttp-opts"]["download-settings"]["private-key"],
            "/data/download.key"
        );
        assert_eq!(proxies[1]["private-key"], "CAsAAABhYmM=");
        assert_eq!(proxies[2]["private-key"], "/data/ssh.key");
        assert_eq!(proxies[3]["realm-opts"]["certificate"], "/data/realm.pem");
        assert_eq!(proxies[4]["planet"], "/data/planet.bin");
    }
}
