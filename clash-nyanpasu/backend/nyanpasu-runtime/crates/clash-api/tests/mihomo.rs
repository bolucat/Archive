//! End-to-end compatibility tests against a real mihomo process.
//!
//! The transport matrix is platform-aware: HTTP runs everywhere, Windows also
//! runs named pipes, and Unix also runs Unix domain sockets. Prepare the binary
//! with `deno run -A scripts/prepare-mihomo.ts`, then run this ignored test with
//! `cargo test -p clash-api --test mihomo -- --ignored --nocapture`.

use std::{
    convert::Infallible,
    fs,
    future::pending,
    net::{SocketAddr, TcpListener},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, SystemTime},
};

use axum::{
    Router,
    body::{Body, Bytes as BodyBytes},
    http::{Response, StatusCode},
    routing::get,
};
use clash_api::{
    Client, ConfigPatch, Connection, ConnectionStreamQuery, DelayQuery, DnsQuery, DnsRecordType,
    ExpectedStatus, Host, LogEntry, LogLevel, LogQuery, Memory, ProviderName, ProxyName, RulePatch,
    RuleProviderName, StorageKey, StructuredLogEntry, Traffic, TunnelMode, UpdateConfigOptions,
    UpdateConfigRequest, UpgradeOptions,
};
use futures_util::{StreamExt, stream};
use reqwest_websocket::Message;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::{sync::oneshot, task::JoinHandle, time::timeout};

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
    },
    core::PCWSTR,
};

const SECRET: &str = "mihomo-integration-secret";
const GROUP: &str = "fixture group 日本";
const AUTOMATIC_GROUP: &str = "fixture automatic 日本";
const DIRECT: &str = "fixture direct 日本";
const REJECT: &str = "fixture reject 日本";
const PROXY_PROVIDER: &str = "fixture provider 日本";
const PROVIDER_PROXY: &str = "provider direct 日本";
const RULE_PROVIDER: &str = "fixture rules 日本";

struct Mihomo {
    child: Child,
    home: PathBuf,
    config: PathBuf,
    executable: PathBuf,
    http_address: String,
    local_controller: PathBuf,
    #[cfg(windows)]
    job: Option<WindowsJob>,
}

#[cfg(windows)]
struct WindowsJob(HANDLE);

#[cfg(windows)]
impl WindowsJob {
    fn assign(child: &Child) -> Self {
        unsafe {
            let job = CreateJobObjectW(None, PCWSTR::null()).unwrap();
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .unwrap();
            AssignProcessToJobObject(job, HANDLE(child.as_raw_handle())).unwrap();
            Self(job)
        }
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        unsafe {
            let _ = TerminateJobObject(self.0, 1);
            let _ = CloseHandle(self.0);
        }
    }
}

impl Mihomo {
    async fn start(healthcheck_url: &str, dns_nameserver: &str, ui_url: &str) -> Self {
        let workspace = workspace_root();
        let binary = std::env::var_os("MIHOMO_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                workspace
                    .join("tests/bin")
                    .join(format!("mihomo{}", std::env::consts::EXE_SUFFIX))
            });
        assert!(
            binary.is_file(),
            "mihomo was not found at {}; run `deno run -A scripts/prepare-mihomo.ts` or set MIHOMO_BIN",
            binary.display()
        );

        let home = unique_temp_dir();
        fs::create_dir(&home).unwrap();
        let executable = home.join(format!("mihomo{}", std::env::consts::EXE_SUFFIX));
        fs::copy(&binary, &executable).unwrap();
        let fixture_dir = workspace.join("tests/fixtures/mihomo");
        let config = home.join("config.yaml");
        let config_contents = fs::read_to_string(fixture_dir.join("config.yaml"))
            .unwrap()
            .replace("http://127.0.0.1:1/generate_204", healthcheck_url)
            .replace("http://127.0.0.1:1/ui.zip", ui_url)
            .replace("rcode://success", dns_nameserver);
        fs::write(&config, config_contents).unwrap();
        fs::copy(
            fixture_dir.join("proxy-provider.yaml"),
            home.join("proxy-provider.yaml"),
        )
        .unwrap();
        fs::copy(
            fixture_dir.join("rule-provider.yaml"),
            home.join("rule-provider.yaml"),
        )
        .unwrap();

        let http_address = format!("127.0.0.1:{}", unused_tcp_port());
        #[cfg(windows)]
        let local_controller = PathBuf::from(format!(
            r"\\.\pipe\clash-api-mihomo-{}-{}",
            std::process::id(),
            unique_nonce()
        ));
        #[cfg(unix)]
        let local_controller = home.join("controller.sock");

        let log = fs::File::create(home.join("mihomo.log")).unwrap();
        let mut command = Command::new(&executable);
        command
            .arg("-d")
            .arg(&home)
            .arg("-f")
            .arg(&config)
            .arg("-ext-ctl")
            .arg(&http_address)
            .arg("-secret")
            .arg(SECRET)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log.try_clone().unwrap()))
            .stderr(Stdio::from(log));
        for variable in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "NO_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
            "no_proxy",
        ] {
            command.env_remove(variable);
        }
        #[cfg(windows)]
        command.arg("-ext-ctl-pipe").arg(&local_controller);
        #[cfg(unix)]
        command.arg("-ext-ctl-unix").arg(&local_controller);

        let child = command.spawn().unwrap();
        #[cfg(windows)]
        let job = Some(WindowsJob::assign(&child));
        let mut mihomo = Self {
            child,
            home,
            config,
            executable,
            http_address,
            local_controller,
            #[cfg(windows)]
            job,
        };
        mihomo.wait_until_ready().await;
        mihomo
    }

    fn http_client(&self) -> Client {
        Client::builder(Host::http(&self.http_address).unwrap())
            .secret(SECRET)
            .build()
            .unwrap()
    }

    fn local_client(&self) -> Client {
        #[cfg(windows)]
        let host = Host::named_pipe(&self.local_controller);
        #[cfg(unix)]
        let host = Host::unix_socket(&self.local_controller);

        Client::builder(host)
            .secret("local-transports-must-ignore-this")
            .build()
            .unwrap()
    }

    async fn wait_until_ready(&mut self) {
        let client = self.http_client();
        let ready = timeout(Duration::from_secs(10), async {
            loop {
                if client.version().await.is_ok() {
                    break;
                }
                if let Some(status) = self.child.try_wait().unwrap() {
                    panic!(
                        "mihomo exited during startup with {status}:\n{}",
                        self.logs()
                    );
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await;
        assert!(
            ready.is_ok(),
            "mihomo did not expose its controller in time:\n{}",
            self.logs()
        );
    }

    fn logs(&self) -> String {
        fs::read_to_string(self.home.join("mihomo.log")).unwrap_or_default()
    }

    fn hide_executable(&self) -> PathBuf {
        let hidden = self.home.join("mihomo.hidden");
        #[cfg(windows)]
        fs::rename(&self.executable, &hidden).unwrap();
        #[cfg(unix)]
        {
            fs::copy(&self.executable, &hidden).unwrap();
            fs::remove_file(&self.executable).unwrap();
        }
        hidden
    }

    fn restore_executable(&self, hidden: &Path) {
        fs::rename(hidden, &self.executable).unwrap();
    }

    async fn restart_and_wait(&mut self, client: &Client) {
        assert_eq!(client.restart().await.unwrap().status, "ok");

        #[cfg(windows)]
        timeout(Duration::from_secs(10), async {
            loop {
                if self.child.try_wait().unwrap().is_some() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        })
        .await
        .expect("the original mihomo process did not exit during restart");

        tokio::time::sleep(Duration::from_millis(200)).await;
        timeout(Duration::from_secs(10), async {
            loop {
                if client.version().await.is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("mihomo did not return after restart:\n{}", self.logs()));
    }
}

impl Drop for Mihomo {
    fn drop(&mut self) {
        #[cfg(windows)]
        drop(self.job.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        for _ in 0..20 {
            if fs::remove_dir_all(&self.home).is_ok() || !self.home.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
struct StoredFixture {
    enabled: bool,
    label: String,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires the platform mihomo binary in tests/bin"]
async fn real_mihomo_api_and_transport_matrix() {
    let (target_address, target_server) = spawn_target_server().await;
    let (dns_address, dns_server) = spawn_dns_server().await;
    let healthcheck_url = format!("http://{target_address}/generate_204");
    let ui_url = format!("http://{target_address}/ui.zip");
    let dns_nameserver = format!("udp://{dns_address}");
    let ui_fixture = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(&ui_url)
        .send()
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(ui_fixture.starts_with(&[0x50, 0x4b, 0x03, 0x04]));
    let mut mihomo = Mihomo::start(&healthcheck_url, &dns_nameserver, &ui_url).await;
    let client = mihomo.http_client();

    let unauthenticated = Client::new_http(&mihomo.http_address).unwrap();
    assert_eq!(
        unauthenticated.version().await.unwrap_err().status(),
        Some(StatusCode::UNAUTHORIZED)
    );

    assert_transport("HTTP", &client).await;
    let local_client = mihomo.local_client();
    #[cfg(windows)]
    assert_transport("Windows named pipe", &local_client).await;
    #[cfg(unix)]
    assert_transport("Unix domain socket", &local_client).await;

    client
        .update_config(
            &UpdateConfigRequest::from_path(mihomo.config.to_string_lossy()),
            UpdateConfigOptions { force: true },
        )
        .await
        .unwrap();

    let config = client.configs().await.unwrap();
    assert_eq!(config.mode, TunnelMode::Rule);
    assert_eq!(config.log_level, LogLevel::Debug);
    assert!(!config.allow_lan);

    client
        .patch_config(&ConfigPatch {
            allow_lan: Some(true),
            ..ConfigPatch::default()
        })
        .await
        .unwrap();
    assert!(client.configs().await.unwrap().allow_lan);

    assert_proxy_and_rule_apis(&client, &healthcheck_url).await;
    assert_dns_and_storage_apis(&client).await;

    client.flush_fake_ip_cache().await.unwrap();
    client.flush_dns_cache().await.unwrap();
    client.collect_garbage().await.unwrap();
    assert_real_update_apis(&client, &mihomo).await;

    let mixed_port = unused_tcp_port();
    client
        .patch_config(&ConfigPatch {
            allow_lan: Some(false),
            mixed_port: Some(i64::from(mixed_port)),
            ..ConfigPatch::default()
        })
        .await
        .unwrap();
    let proxied = proxied_client(mixed_port);

    assert_streaming_apis(&client, &proxied, target_address).await;
    assert_connection_apis(&client, &proxied, target_address).await;

    assert!(
        mihomo.child.try_wait().unwrap().is_none(),
        "{}",
        mihomo.logs()
    );
    mihomo.restart_and_wait(&client).await;
    assert_transport("HTTP after restart", &client).await;
    target_server.abort();
    dns_server.abort();
}

async fn assert_transport(name: &str, client: &Client) {
    let hello = client
        .hello()
        .await
        .unwrap_or_else(|error| panic!("{name} REST failed: {error}"));
    assert!(!hello.hello.is_empty());
    let version = client.version().await.unwrap();
    assert!(version.meta);
    assert!(!version.version.is_empty());

    let mut websocket = client
        .traffic_ws()
        .await
        .unwrap_or_else(|error| panic!("{name} WebSocket failed: {error}"));
    let _: Traffic = next_ws_json(&mut websocket).await;
}

async fn assert_real_update_apis(client: &Client, mihomo: &Mihomo) {
    client.update_geo_databases().await.unwrap();
    client.upgrade_geo_databases().await.unwrap();

    assert_eq!(
        client
            .upgrade_ui()
            .await
            .unwrap_or_else(|error| panic!("upgrade_ui failed: {error}\n{}", mihomo.logs()))
            .status,
        "ok"
    );
    assert_eq!(
        fs::read_to_string(mihomo.home.join("ui/index.html")).unwrap(),
        "mihomo integration fixture"
    );

    // The core updater has hard-coded GitHub URLs. Make os.Stat fail before it
    // reaches the network or can replace the executable; this still exercises
    // the real route and the client's error decoding against mihomo.
    let hidden = mihomo.hide_executable();
    let result = client
        .upgrade(&UpgradeOptions {
            channel: Some("release".to_owned()),
            force: true,
        })
        .await;
    mihomo.restore_executable(&hidden);
    let error = result.unwrap_err();
    assert_eq!(error.status(), Some(StatusCode::INTERNAL_SERVER_ERROR));
    assert!(
        error
            .error_body()
            .and_then(|body| body.message())
            .is_some_and(|message| message.contains("check currentExePath"))
    );
}

async fn assert_proxy_and_rule_apis(client: &Client, healthcheck_url: &str) {
    let groups = client.groups().await.unwrap();
    assert!(groups.iter().any(|proxy| proxy.name.as_str() == GROUP));
    assert!(
        groups
            .iter()
            .any(|proxy| proxy.name.as_str() == AUTOMATIC_GROUP)
    );
    assert_eq!(
        client.group(&ProxyName::from(GROUP)).await.unwrap().name,
        ProxyName::from(GROUP)
    );

    let proxies = client.proxies().await.unwrap();
    assert!(proxies.contains_key(&ProxyName::from(DIRECT)));
    assert_eq!(
        client.proxy(&ProxyName::from(DIRECT)).await.unwrap().name,
        ProxyName::from(DIRECT)
    );

    client
        .select_proxy(&ProxyName::from(GROUP), &ProxyName::from(REJECT))
        .await
        .unwrap();
    assert_eq!(
        client.group(&ProxyName::from(GROUP)).await.unwrap().now,
        Some(ProxyName::from(REJECT))
    );
    client
        .select_proxy(&ProxyName::from(GROUP), &ProxyName::from(DIRECT))
        .await
        .unwrap();

    let delay_query = DelayQuery::new(healthcheck_url.parse().unwrap(), Duration::from_secs(2))
        .unwrap()
        .with_expected(ExpectedStatus::new("204").unwrap());
    let group_delay = client
        .group_delay(&ProxyName::from(GROUP), &delay_query)
        .await
        .unwrap();
    assert!(
        group_delay
            .get(&ProxyName::from(DIRECT))
            .is_some_and(|delay| *delay > 0)
    );
    assert!(
        client
            .proxy_delay(&ProxyName::from(DIRECT), &delay_query)
            .await
            .unwrap()
            .delay
            < 2_000
    );
    client
        .clear_proxy_selection(&ProxyName::from(AUTOMATIC_GROUP))
        .await
        .unwrap();

    let provider_name = ProviderName::from(PROXY_PROVIDER);
    let provider_proxy = ProxyName::from(PROVIDER_PROXY);
    let providers = client.proxy_providers().await.unwrap();
    assert!(providers.contains_key(&provider_name));
    assert_eq!(
        client.proxy_provider(&provider_name).await.unwrap().name,
        provider_name
    );
    client.update_proxy_provider(&provider_name).await.unwrap();
    client
        .healthcheck_proxy_provider(&provider_name)
        .await
        .unwrap();
    assert_eq!(
        client
            .provider_proxy(&provider_name, &provider_proxy)
            .await
            .unwrap()
            .name,
        provider_proxy
    );
    assert!(
        client
            .provider_proxy_delay(&provider_name, &provider_proxy, &delay_query)
            .await
            .unwrap()
            .delay
            < 2_000
    );

    let rules = client.rules().await.unwrap();
    assert!(rules.iter().any(|rule| rule.payload == "fixture.test"));
    let rule_index = rules
        .iter()
        .find(|rule| rule.payload == "fixture.test")
        .unwrap()
        .index as usize;
    let mut patch = RulePatch::new();
    patch.set_disabled(rule_index, true);
    client.patch_rules(&patch).await.unwrap();
    assert!(
        client.rules().await.unwrap()[rule_index]
            .extra
            .as_ref()
            .is_some_and(|extra| extra.disabled)
    );
    patch.set_disabled(rule_index, false);
    client.patch_rules(&patch).await.unwrap();

    let rule_provider = RuleProviderName::from(RULE_PROVIDER);
    assert!(
        client
            .rule_providers()
            .await
            .unwrap()
            .contains_key(&rule_provider)
    );
    client.update_rule_provider(&rule_provider).await.unwrap();
}

async fn assert_dns_and_storage_apis(client: &Client) {
    let dns = client
        .dns_query(&DnsQuery::new("fixture.test", DnsRecordType::a()).unwrap())
        .await
        .unwrap();
    assert_eq!(dns.status, 0);
    assert!(
        dns.answer
            .as_ref()
            .is_some_and(|records| records.iter().any(|record| record.data == "198.18.0.1"))
    );

    let key = StorageKey::new("fixture storage 日本").unwrap();
    client.storage_delete(&key).await.unwrap();
    let missing: Option<StoredFixture> = client.storage_get(&key).await.unwrap();
    assert_eq!(missing, None);
    let value = StoredFixture {
        enabled: true,
        label: "round trip".to_owned(),
    };
    client.storage_put(&key, &value).await.unwrap();
    assert_eq!(client.storage_get(&key).await.unwrap(), Some(value));
    client.storage_delete(&key).await.unwrap();
}

async fn assert_streaming_apis(client: &Client, proxied: &reqwest::Client, target: SocketAddr) {
    let mut traffic = client.traffic().await.unwrap();
    let _: Traffic = next_http_item(&mut traffic).await;
    let mut memory = client.memory().await.unwrap();
    let _: Memory = next_http_item(&mut memory).await;

    let mut connections = client
        .connections_ws(ConnectionStreamQuery::new(Duration::from_millis(100)).unwrap())
        .await
        .unwrap();
    let _: clash_api::ConnectionsSnapshot = next_ws_json(&mut connections).await;

    let mut memory_ws = client.memory_ws().await.unwrap();
    let _: Memory = next_ws_json(&mut memory_ws).await;

    let (logs, _) = timeout(Duration::from_secs(5), async {
        tokio::join!(client.logs(LogQuery::new(LogLevel::Debug)), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            make_proxied_request(proxied, target).await;
        })
    })
    .await
    .unwrap();
    let _: LogEntry = next_http_item(&mut logs.unwrap()).await;

    let (structured_logs, _) = timeout(Duration::from_secs(5), async {
        tokio::join!(
            client.structured_logs(LogQuery::new(LogLevel::Debug)),
            async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                make_proxied_request(proxied, target).await;
            }
        )
    })
    .await
    .unwrap();
    let _: StructuredLogEntry = next_http_item(&mut structured_logs.unwrap()).await;

    let mut logs_ws = client
        .logs_ws(LogQuery::new(LogLevel::Debug))
        .await
        .unwrap();
    make_proxied_request(proxied, target).await;
    let _: LogEntry = next_ws_json(&mut logs_ws).await;

    let mut structured_logs_ws = client
        .structured_logs_ws(LogQuery::new(LogLevel::Debug))
        .await
        .unwrap();
    make_proxied_request(proxied, target).await;
    let _: StructuredLogEntry = next_ws_json(&mut structured_logs_ws).await;
}

async fn assert_connection_apis(client: &Client, proxied: &reqwest::Client, target: SocketAddr) {
    assert!(client.connections().await.unwrap().connections.is_none());

    let first = open_held_connection(proxied.clone(), target).await;
    let connection = wait_for_connection(client).await;
    assert!(connection.metadata.is_some());
    client.close_connection(connection.id).await.unwrap();
    wait_for_no_connections(client).await;
    first.abort();

    let second = open_held_connection(proxied.clone(), target).await;
    let _ = wait_for_connection(client).await;
    client.close_all_connections().await.unwrap();
    wait_for_no_connections(client).await;
    second.abort();
}

async fn next_http_item<T>(stream: &mut clash_api::HttpStream<T>) -> T
where
    T: DeserializeOwned + Send + 'static,
{
    timeout(Duration::from_secs(4), stream.next())
        .await
        .expect("mihomo HTTP stream did not produce a sample")
        .expect("mihomo HTTP stream ended")
        .unwrap()
}

async fn next_ws_json<T>(websocket: &mut reqwest_websocket::WebSocket) -> T
where
    T: DeserializeOwned,
{
    timeout(Duration::from_secs(4), async {
        loop {
            match websocket
                .next()
                .await
                .expect("mihomo closed WebSocket")
                .unwrap()
            {
                Message::Text(text) => return serde_json::from_str(&text).unwrap(),
                Message::Binary(bytes) => return serde_json::from_slice(&bytes).unwrap(),
                _ => {}
            }
        }
    })
    .await
    .expect("mihomo WebSocket did not produce a JSON frame")
}

async fn wait_for_connection(client: &Client) -> Connection {
    timeout(Duration::from_secs(4), async {
        loop {
            if let Some(connection) = client
                .connections()
                .await
                .unwrap()
                .connections
                .and_then(|connections| connections.into_iter().next())
            {
                return connection;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("proxied request did not appear in /connections")
}

async fn wait_for_no_connections(client: &Client) {
    timeout(Duration::from_secs(4), async {
        loop {
            if client
                .connections()
                .await
                .unwrap()
                .connections
                .is_none_or(|connections| connections.is_empty())
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("connection remained after the close API call");
}

async fn open_held_connection(client: reqwest::Client, target: SocketAddr) -> JoinHandle<()> {
    let (ready_tx, ready_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        let response = client.get(format!("http://{target}/hold")).send().await;
        let result = response
            .as_ref()
            .map(|response| response.status())
            .map_err(ToString::to_string);
        let _ = ready_tx.send(result);
        if response.is_ok() {
            pending::<()>().await;
        }
    });
    let status = timeout(Duration::from_secs(4), ready_rx)
        .await
        .expect("held request did not receive response headers")
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::OK);
    task
}

async fn make_proxied_request(client: &reqwest::Client, target: SocketAddr) {
    let status = client
        .get(format!("http://{target}/generate_204"))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, StatusCode::NO_CONTENT);
}

fn proxied_client(port: u16) -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}")).unwrap())
        .build()
        .unwrap()
}

async fn spawn_target_server() -> (SocketAddr, JoinHandle<()>) {
    async fn no_content() -> StatusCode {
        tokio::time::sleep(Duration::from_millis(10)).await;
        StatusCode::NO_CONTENT
    }

    async fn hold() -> Response<Body> {
        let chunks = stream::once(async {
            Ok::<_, Infallible>(BodyBytes::from_static(b"connection is held open"))
        })
        .chain(stream::pending());
        Response::builder()
            .status(StatusCode::OK)
            .body(Body::from_stream(chunks))
            .unwrap()
    }

    async fn ui() -> Response<Body> {
        let archive = ui_fixture_zip();
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/zip")
            .header("content-length", archive.len())
            .header("connection", "close")
            .body(Body::from(archive))
            .unwrap()
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/generate_204", get(no_content))
        .route("/hold", get(hold))
        .route("/ui.zip", get(ui));
    let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (address, task)
}

fn ui_fixture_zip() -> Vec<u8> {
    const NAME: &[u8] = b"index.html";
    const CONTENT: &[u8] = b"mihomo integration fixture";

    let crc = crc32(CONTENT);
    let mut archive = Vec::new();
    push_u32(&mut archive, 0x0403_4b50);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, crc);
    push_u32(&mut archive, CONTENT.len() as u32);
    push_u32(&mut archive, CONTENT.len() as u32);
    push_u16(&mut archive, NAME.len() as u16);
    push_u16(&mut archive, 0);
    archive.extend_from_slice(NAME);
    archive.extend_from_slice(CONTENT);

    let central_offset = archive.len() as u32;
    push_u32(&mut archive, 0x0201_4b50);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 20);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, crc);
    push_u32(&mut archive, CONTENT.len() as u32);
    push_u32(&mut archive, CONTENT.len() as u32);
    push_u16(&mut archive, NAME.len() as u16);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u32(&mut archive, 0);
    push_u32(&mut archive, 0);
    archive.extend_from_slice(NAME);

    let central_size = archive.len() as u32 - central_offset;
    push_u32(&mut archive, 0x0605_4b50);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 1);
    push_u16(&mut archive, 1);
    push_u32(&mut archive, central_size);
    push_u32(&mut archive, central_offset);
    push_u16(&mut archive, 0);
    archive
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}

fn push_u16(buffer: &mut Vec<u8>, value: u16) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(buffer: &mut Vec<u8>, value: u32) {
    buffer.extend_from_slice(&value.to_le_bytes());
}

async fn spawn_dns_server() -> (SocketAddr, JoinHandle<()>) {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let address = socket.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut request = [0_u8; 512];
        loop {
            let (length, peer) = socket.recv_from(&mut request).await.unwrap();
            let Some(question_end) = dns_question_end(&request[..length]) else {
                continue;
            };
            let mut response = request[..question_end].to_vec();
            response[2] = 0x81;
            response[3] = 0x80;
            response[6..8].copy_from_slice(&1_u16.to_be_bytes());
            response[8..12].fill(0);
            response.extend_from_slice(&[
                0xc0, 0x0c, // compressed fixture.test name
                0x00, 0x01, // A
                0x00, 0x01, // IN
                0x00, 0x00, 0x00, 0x3c, // 60 second TTL
                0x00, 0x04, // four address bytes
                198, 18, 0, 1,
            ]);
            socket.send_to(&response, peer).await.unwrap();
        }
    });
    (address, task)
}

fn dns_question_end(packet: &[u8]) -> Option<usize> {
    let mut cursor = 12;
    loop {
        let label_length = *packet.get(cursor)? as usize;
        cursor += 1;
        if label_length == 0 {
            break;
        }
        cursor = cursor.checked_add(label_length)?;
        if cursor > packet.len() {
            return None;
        }
    }
    cursor.checked_add(4).filter(|end| *end <= packet.len())
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn unused_tcp_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn unique_temp_dir() -> PathBuf {
    // Keep the path short: on macOS the runner's $TMPDIR is already ~50 chars
    // and `home/controller.sock` must fit the 104-byte unix socket sun_path.
    std::env::temp_dir().join(format!("mihomo-{}-{}", std::process::id(), unique_nonce()))
}

fn unique_nonce() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}
