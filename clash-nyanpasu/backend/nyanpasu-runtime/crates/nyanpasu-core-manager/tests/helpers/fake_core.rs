//! Scripted mihomo simulator for nyanpasu-core-manager tests. Not production code.
//! CLI mirrors mihomo: `[-m] [-t] -d <dir> -f <config>`. See the implementation
//! plan for the `x-fake-core` behavior keys.
#![allow(dead_code)] // several Behavior fields are platform-conditional

use std::{
    process::exit,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use parking_lot::Mutex;
use serde_yaml_ng::{Mapping, Value};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct Behavior {
    external_controller: Option<String>,
    external_controller_pipe: Option<String>,
    external_controller_unix: Option<String>,
    secret: Option<String>,
    mixed_port: u16,
    allow_lan: bool,
    ready_delay_ms: u64,
    never_ready: bool,
    exit_code: Option<i32>,
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
    crash_after_ms: u64,
    crash_times: u64,
    state_file: Option<String>,
    launch_count_file: Option<String>,
    fail_after_launches: u64,
    fail_start_when_allow_lan: bool,
    patch_log: Option<String>,
    reject_patch: bool,
    patch_delay_ms: u64,
    patch_no_effect: bool,
    reject_put: bool,
    check_delay_ms: u64,
    check_started_file: Option<String>,
    check_fail: Option<String>,
}

fn s(doc: &Mapping, key: &str) -> Option<String> {
    doc.get(Value::String(key.into()))
        .and_then(Value::as_str)
        .map(str::to_owned)
}
fn u(doc: &Mapping, key: &str) -> u64 {
    doc.get(Value::String(key.into()))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}
fn b(doc: &Mapping, key: &str) -> bool {
    doc.get(Value::String(key.into()))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn lines(doc: &Mapping, key: &str) -> Vec<String> {
    doc.get(Value::String(key.into()))
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn parse(config: &str) -> Behavior {
    let doc: Mapping = serde_yaml_ng::from_str(config).expect("valid yaml");
    let x = doc
        .get(Value::String("x-fake-core".into()))
        .and_then(Value::as_mapping)
        .cloned()
        .unwrap_or_default();
    Behavior {
        external_controller: s(&doc, "external-controller"),
        external_controller_pipe: s(&doc, "external-controller-pipe"),
        external_controller_unix: s(&doc, "external-controller-unix"),
        secret: s(&doc, "secret"),
        mixed_port: u(&doc, "mixed-port") as u16,
        allow_lan: b(&doc, "allow-lan"),
        ready_delay_ms: u(&x, "ready-delay-ms"),
        never_ready: b(&x, "never-ready"),
        exit_code: x
            .get(Value::String("exit-code".into()))
            .and_then(Value::as_i64)
            .map(|c| c as i32),
        stdout_lines: lines(&x, "stdout-lines"),
        stderr_lines: lines(&x, "stderr-lines"),
        crash_after_ms: u(&x, "crash-after-ms"),
        crash_times: u(&x, "crash-times"),
        state_file: s(&x, "state-file"),
        launch_count_file: s(&x, "launch-count-file"),
        fail_after_launches: u(&x, "fail-after-launches"),
        fail_start_when_allow_lan: b(&x, "fail-start-when-allow-lan"),
        patch_log: s(&x, "patch-log"),
        reject_patch: b(&x, "reject-patch"),
        patch_delay_ms: u(&x, "patch-delay-ms"),
        patch_no_effect: b(&x, "patch-no-effect"),
        reject_put: b(&x, "reject-put"),
        check_delay_ms: u(&x, "check-delay-ms"),
        check_started_file: s(&x, "check-started-file"),
        check_fail: s(&x, "check-fail"),
    }
}

struct Ctx {
    ready: AtomicBool,
    behavior: Behavior,
    runtime: Mutex<Mapping>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-v") {
        if let Ok(binary) = std::env::current_exe()
            && binary
                .file_stem()
                .is_some_and(|name| name.to_string_lossy().starts_with("nyanpasu-version-probe"))
        {
            let counter = binary.with_extension("version-probes");
            let count = std::fs::read_to_string(&counter)
                .ok()
                .and_then(|value| value.trim().parse::<u64>().ok())
                .unwrap_or(0)
                + 1;
            std::fs::write(counter, count.to_string()).expect("write version probe counter");
        }
        println!("Mihomo Meta v1.18.9 linux test");
        return;
    }
    let check_mode = args.iter().any(|a| a == "-t");
    let config_path = args
        .iter()
        .position(|a| a == "-f")
        .and_then(|i| args.get(i + 1))
        .expect("-f <config> required");
    let config = std::fs::read_to_string(config_path).expect("readable config");
    let behavior = parse(&config);

    if check_mode {
        if let Some(path) = &behavior.check_started_file {
            std::fs::write(path, "started").expect("write check-started marker");
        }
        std::thread::sleep(Duration::from_millis(behavior.check_delay_ms));
        match &behavior.check_fail {
            Some(msg) => {
                println!("time=\"t\" level=error msg=\"{msg}\"");
                exit(1);
            }
            None => exit(0),
        }
    }

    for line in &behavior.stdout_lines {
        println!("{line}");
    }
    for line in &behavior.stderr_lines {
        eprintln!("{line}");
    }
    if behavior.fail_start_when_allow_lan && behavior.allow_lan {
        exit(23);
    }
    if let Some(counter_path) = &behavior.launch_count_file {
        let count: u64 = std::fs::read_to_string(counter_path)
            .ok()
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0)
            + 1;
        std::fs::write(counter_path, count.to_string()).expect("write launch counter");
        if count > behavior.fail_after_launches {
            exit(77);
        }
    }
    if let Some(code) = behavior.exit_code {
        exit(code);
    }

    // Crash script: only the first `crash-times` runs crash.
    if behavior.crash_after_ms > 0 {
        let state_file = behavior.state_file.clone().expect("crash needs state-file");
        let count: u64 = std::fs::read_to_string(&state_file)
            .ok()
            .and_then(|c| c.trim().parse().ok())
            .unwrap_or(0);
        if count < behavior.crash_times {
            std::fs::write(&state_file, (count + 1).to_string()).expect("write state file");
            let delay = behavior.crash_after_ms;
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(delay)).await;
                exit(1);
            });
        }
    }

    if behavior.mixed_port != 0 {
        let listener = TcpListener::bind(("127.0.0.1", behavior.mixed_port))
            .await
            .expect("bind mixed-port");
        hold_listener(listener);
    }

    let ctx = Arc::new(Ctx {
        ready: AtomicBool::new(false),
        behavior,
        runtime: Mutex::new(serde_yaml_ng::from_str(&config).expect("runtime mapping")),
    });
    if !ctx.behavior.never_ready {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ctx.behavior.ready_delay_ms)).await;
            ctx.ready.store(true, Ordering::SeqCst);
        });
    }

    let mut served = false;
    if let Some(addr) = ctx.behavior.external_controller.clone() {
        let listener = TcpListener::bind(&addr).await.expect("bind controller");
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(stream, ctx, true).await });
            }
        });
        served = true;
    }
    served |= serve_local_transports(&ctx);
    if !served {
        eprintln!("fake-core: no controller configured");
    }

    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

/// Accept-and-hold so the port stays bound (simulates a proxy listener).
fn hold_listener(listener: TcpListener) {
    tokio::spawn(async move {
        loop {
            let _ = listener.accept().await;
        }
    });
}

fn serve_local_transports(ctx: &Arc<Ctx>) -> bool {
    let mut served = false;
    #[cfg(windows)]
    if let Some(path) = ctx.behavior.external_controller_pipe.clone() {
        let ctx = ctx.clone();
        tokio::spawn(async move {
            use tokio::net::windows::named_pipe::ServerOptions;
            let mut server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(&path)
                .expect("create pipe");
            loop {
                if server.connect().await.is_err() {
                    continue;
                }
                let conn = server;
                server = ServerOptions::new().create(&path).expect("recreate pipe");
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(conn, ctx, false).await });
            }
        });
        served = true;
    }
    #[cfg(unix)]
    if let Some(path) = ctx.behavior.external_controller_unix.clone() {
        let _ = std::fs::remove_file(&path);
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("bind unix socket");
        listener.set_nonblocking(true).expect("nonblocking");
        let listener = tokio::net::UnixListener::from_std(listener).expect("tokio listener");
        let ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let ctx = ctx.clone();
                tokio::spawn(async move { serve_conn(stream, ctx, false).await });
            }
        });
        served = true;
    }
    let _ = ctx;
    served
}

async fn serve_conn<S>(mut stream: S, ctx: Arc<Ctx>, http_transport: bool)
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buf = Vec::new();
    let header_end = loop {
        let mut chunk = [0u8; 1024];
        let Ok(n) = stream.read(&mut chunk).await else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
        if buf.len() > 64 * 1024 {
            return;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).into_owned();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_owned();

    let mut content_length = 0usize;
    let mut authorization = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        match name.trim().to_ascii_lowercase().as_str() {
            "content-length" => content_length = value.trim().parse().unwrap_or(0),
            "authorization" => authorization = Some(value.trim().to_owned()),
            _ => {}
        }
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let mut chunk = [0u8; 1024];
        let Ok(n) = stream.read(&mut chunk).await else {
            return;
        };
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let body = String::from_utf8_lossy(&body).into_owned();

    if http_transport
        && let Some(secret) = &ctx.behavior.secret
        && authorization.as_deref() != Some(&format!("Bearer {secret}"))
    {
        respond(&mut stream, 401, r#"{"message":"Unauthorized"}"#).await;
        return;
    }

    match (method.as_str(), path.as_str()) {
        ("GET", "/version") => {
            if ctx.ready.load(Ordering::SeqCst) {
                respond(&mut stream, 200, r#"{"meta":true,"version":"fake-core"}"#).await;
            } else {
                respond(&mut stream, 503, r#"{"message":"starting"}"#).await;
            }
        }
        ("GET", "/configs") => {
            let body = runtime_config_json(&ctx.runtime.lock());
            respond(&mut stream, 200, &body).await;
        }
        ("PUT", "/configs") => {
            if ctx.behavior.reject_put {
                respond(&mut stream, 500, r#"{"message":"reload rejected"}"#).await;
                return;
            }
            let request: Mapping = serde_yaml_ng::from_str(&body).expect("PUT JSON mapping");
            let path = request
                .get(Value::String("path".into()))
                .and_then(Value::as_str)
                .expect("PUT path");
            let desired = std::fs::read_to_string(path).expect("read PUT path");
            *ctx.runtime.lock() = serde_yaml_ng::from_str(&desired).expect("PUT runtime mapping");
            respond(&mut stream, 204, "").await;
        }
        ("PATCH", "/configs") => {
            if ctx.behavior.reject_patch {
                respond(&mut stream, 500, r#"{"message":"patch rejected"}"#).await;
                return;
            }
            if let Some(log) = &ctx.behavior.patch_log {
                let mut existing = std::fs::read_to_string(log).unwrap_or_default();
                existing.push_str(&body);
                existing.push('\n');
                let _ = std::fs::write(log, existing);
            }
            if let Some(port) = extract_mixed_port(&body) {
                match TcpListener::bind(("127.0.0.1", port)).await {
                    Ok(listener) => hold_listener(listener),
                    Err(_) => {
                        respond(&mut stream, 500, r#"{"message":"bind failed"}"#).await;
                        return;
                    }
                }
            }
            if !ctx.behavior.patch_no_effect {
                let patch: Mapping = serde_yaml_ng::from_str(&body).expect("PATCH JSON mapping");
                merge_mapping(&mut ctx.runtime.lock(), &patch);
            }
            if ctx.behavior.patch_delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(ctx.behavior.patch_delay_ms)).await;
            }
            respond(&mut stream, 204, "").await;
        }
        _ => respond(&mut stream, 404, r#"{"message":"not found"}"#).await,
    }
}

fn merge_mapping(target: &mut Mapping, patch: &Mapping) {
    for (key, value) in patch {
        if let (Some(target), Some(patch)) = (
            target.get_mut(key).and_then(Value::as_mapping_mut),
            value.as_mapping(),
        ) {
            merge_mapping(target, patch);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn runtime_config_json(document: &Mapping) -> String {
    let integer = |key: &str| {
        document
            .get(Value::String(key.into()))
            .and_then(Value::as_i64)
            .unwrap_or(0)
    };
    let boolean = |key: &str| {
        document
            .get(Value::String(key.into()))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    };
    let string = |key: &str, default: &str| {
        document
            .get(Value::String(key.into()))
            .and_then(Value::as_str)
            .unwrap_or(default)
            .to_owned()
    };
    let nested = |key: &str| {
        document
            .get(Value::String(key.into()))
            .map(value_to_json)
            .unwrap_or_else(|| "{}".into())
    };
    let optional_string = |key: &str| {
        document
            .get(Value::String(key.into()))
            .and_then(Value::as_str)
            .map(json_string)
            .unwrap_or_else(|| "null".into())
    };
    let optional_value = |key: &str| {
        document
            .get(Value::String(key.into()))
            .map(value_to_json)
            .unwrap_or_else(|| "null".into())
    };
    format!(
        concat!(
            "{{\"port\":{},\"socks-port\":{},\"redir-port\":{},\"tproxy-port\":{},",
            "\"mixed-port\":{},\"tun\":{},\"tuic-server\":{},\"ss-config\":{},",
            "\"vmess-config\":{},\"tcptun-config\":{},\"udptun-config\":{},",
            "\"authentication\":null,\"skip-auth-prefixes\":{},\"lan-allowed-ips\":{},",
            "\"lan-disallowed-ips\":{},\"allow-lan\":{},\"bind-address\":{},",
            "\"inbound-tfo\":false,\"inbound-mptcp\":false,\"mode\":{},",
            "\"unified-delay\":false,\"log-level\":{},\"ipv6\":{},\"interface-name\":{},",
            "\"routing-mark\":0,\"geox-url\":{{}},\"geo-auto-update\":false,",
            "\"geo-update-interval\":0,\"geodata-mode\":false,\"geodata-loader\":\"\",",
            "\"geosite-matcher\":\"\",\"tcp-concurrent\":{},\"find-process-mode\":{},",
            "\"sniffing\":{},\"global-ua\":\"\",\"etag-support\":false,",
            "\"keep-alive-idle\":0,\"keep-alive-interval\":0,\"disable-keep-alive\":false}}"
        ),
        integer("port"),
        integer("socks-port"),
        integer("redir-port"),
        integer("tproxy-port"),
        integer("mixed-port"),
        nested("tun"),
        nested("tuic-server"),
        json_string(&string("ss-config", "")),
        json_string(&string("vmess-config", "")),
        optional_string("tcptun-config"),
        optional_string("udptun-config"),
        optional_value("skip-auth-prefixes"),
        optional_value("lan-allowed-ips"),
        optional_value("lan-disallowed-ips"),
        boolean("allow-lan"),
        json_string(&string("bind-address", "*")),
        json_string(&string("mode", "rule")),
        json_string(&string("log-level", "info")),
        boolean("ipv6"),
        json_string(&string("interface-name", "")),
        boolean("tcp-concurrent"),
        json_string(&string("find-process-mode", "off")),
        boolean("sniffing"),
    )
}

fn value_to_json(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => json_string(value),
        Value::Sequence(values) => format!(
            "[{}]",
            values
                .iter()
                .map(value_to_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Mapping(mapping) => format!(
            "{{{}}}",
            mapping
                .iter()
                .filter_map(|(key, value)| key
                    .as_str()
                    .map(|key| { format!("{}:{}", json_string(key), value_to_json(value)) }))
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Tagged(tagged) => value_to_json(&tagged.value),
    }
}

fn json_string(value: &str) -> String {
    format!("{value:?}")
}

fn extract_mixed_port(body: &str) -> Option<u16> {
    let idx = body.find("\"mixed-port\":")?;
    let digits: String = body[idx + 13..]
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

async fn respond<S: AsyncWriteExt + Unpin>(stream: &mut S, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        401 => "Unauthorized",
        503 => "Service Unavailable",
        500 => "Internal Server Error",
        _ => "Not Found",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;
}
