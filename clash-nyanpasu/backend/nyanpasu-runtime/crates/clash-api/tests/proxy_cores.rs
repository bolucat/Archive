//! Read-only group discovery against explicitly supplied local core binaries.
//! Run with MIHOMO_BIN, MEOW_BIN and CLASH_RS_BIN; no downloads or system proxy changes.
use clash_api::{Client, Host, ProxyName};
use std::{
    collections::BTreeSet,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Core {
    child: Child,
    _home: tempfile::TempDir,
}
impl Drop for Core {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
#[ignore = "requires MIHOMO_BIN, MEOW_BIN and CLASH_RS_BIN"]
async fn runtime_group_lists_and_proxy_inference_match_real_cores() {
    for (variable, lists_groups) in [
        ("MIHOMO_BIN", true),
        ("MEOW_BIN", true),
        ("CLASH_RS_BIN", false),
    ] {
        let binary = std::env::var_os(variable).expect(variable);
        let home = tempfile::tempdir().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        std::fs::write(
            home.path().join("provider.yaml"),
            "proxies:\n  - {name: provider-node, type: socks5, server: 127.0.0.1, port: 1}\n",
        )
        .unwrap();
        let provider = home
            .path()
            .join("provider.yaml")
            .to_string_lossy()
            .replace('\\', "/");
        let additional_groups = if variable == "MIHOMO_BIN" {
            "  - {name: Auto, type: url-test, proxies: [A, DIRECT], url: 'http://127.0.0.1:1/', interval: 0, lazy: true}\n  - {name: Backup, type: fallback, proxies: [A, DIRECT], url: 'http://127.0.0.1:1/', interval: 0, lazy: true}\n  - {name: Balance, type: load-balance, proxies: [A, DIRECT], url: 'http://127.0.0.1:1/', interval: 0, lazy: true, hidden: true}\n"
        } else {
            ""
        };
        let config = format!(
            r#"
port: 0
socks-port: 0
mixed-port: 0
allow-lan: false
bind-address: 127.0.0.1
external-controller: 127.0.0.1:{port}
secret: group-discovery-test
mode: rule
log-level: warning
ipv6: false
geo-auto-update: false
dns:
  enable: false
tun:
  enable: false
profile:
  store-selected: false
proxies:
  - {{name: A, type: socks5, server: 127.0.0.1, port: 1}}
proxy-providers:
  sample:
    type: file
    path: '{provider}'
    health-check:
      enable: false
      url: http://127.0.0.1:1/
      interval: 0
proxy-groups:
  - {{name: GLOBAL, type: select, proxies: [A, Foo]}}
  - {{name: Foo, type: select, proxies: [A, DIRECT]}}
  - {{name: Bar, type: select, proxies: [DIRECT], use: [sample]}}
  - {{name: group, type: select, proxies: [DIRECT]}}
{additional_groups}rules:
  - MATCH,Foo
"#
        );
        let config_path = home.path().join("config.yaml");
        std::fs::write(&config_path, config).unwrap();
        let mut command = Command::new(binary);
        command
            .arg("-d")
            .arg(home.path())
            .arg(if lists_groups { "-f" } else { "-c" })
            .arg(&config_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut core = Core {
            child: command.spawn().unwrap(),
            _home: home,
        };
        let client = Client::builder(Host::http(format!("127.0.0.1:{port}")).unwrap())
            .secret("group-discovery-test")
            .build()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                assert!(
                    core.child.try_wait().unwrap().is_none(),
                    "{variable} exited during startup"
                );
                if client.version().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("core startup");
        let proxies = client.proxies().await.unwrap();
        let providers = client.proxy_providers().await.unwrap();
        assert!(
            providers
                .values()
                .flat_map(|p| &p.proxies)
                .any(|p| p.name.as_str() == "provider-node")
        );
        let inferred: BTreeSet<_> = proxies
            .values()
            .filter(|p| p.all.is_some())
            .map(|p| p.name.as_str().to_owned())
            .collect();
        let mut expected: BTreeSet<_> = ["GLOBAL", "Foo", "Bar", "group"]
            .map(str::to_owned)
            .into_iter()
            .collect();
        if variable == "MIHOMO_BIN" {
            expected.extend(["Auto", "Backup", "Balance"].map(str::to_owned));
        }
        assert_eq!(inferred, expected);
        if lists_groups {
            let groups = client.groups().await.unwrap();
            let listed = groups
                .keys()
                .map(|name| name.as_str().to_owned())
                .collect::<BTreeSet<_>>();
            assert_eq!(listed, inferred, "{variable}");
            if variable == "MIHOMO_BIN" {
                let balance = &groups[&ProxyName::from("Balance")];
                assert!(balance.now.is_none());
                assert_eq!(balance.hidden, Some(true));
            }
            assert_eq!(
                client.group(&"Foo".into()).await.unwrap().name.as_str(),
                "Foo"
            );
            assert_eq!(
                proxies[&ProxyName::from("GLOBAL")]
                    .all
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|n| n.as_str())
                    .collect::<Vec<_>>(),
                ["A", "Foo"]
            );
        } else {
            assert_eq!(
                client
                    .groups()
                    .await
                    .unwrap_err()
                    .status()
                    .unwrap()
                    .as_u16(),
                404
            );
        }
        eprintln!(
            "{variable}: {} discovery passed",
            client.version().await.unwrap().version
        );
    }
}
