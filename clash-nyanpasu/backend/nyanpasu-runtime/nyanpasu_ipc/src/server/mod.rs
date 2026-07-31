use std::result::Result as StdResult;

use axum::Router;
use interprocess::local_socket::{
    GenericFilePath, ListenerNonblockingMode, ListenerOptions,
    tokio::{Listener, Stream as InterProcessStream, prelude::*},
};
#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
use interprocess::os::unix::local_socket::ListenerOptionsExt;
#[cfg(windows)]
use interprocess::os::windows::{
    local_socket::ListenerOptionsExt, security_descriptor::SecurityDescriptor,
};
use thiserror::Error;
use tracing_attributes::instrument;

type Result<T> = StdResult<T, ServerError>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Other error: {0}")]
    Other(#[from] anyhow::Error),
}

pub struct InterProcessListener(Listener, String);

fn configure_listener_mode<'n>(options: ListenerOptions<'n>) -> ListenerOptions<'n> {
    // Interprocess applies this mode with fchmod() before bind(). macOS does not support
    // fchmod() on socket file descriptors, so permissions are applied to the socket path below.
    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
    {
        return options.mode(0o664);
    }

    #[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")))]
    options
}

/// copy from axum::serve::listener.rs
async fn handle_accept_error(e: std::io::Error) {
    if is_connection_error(&e) {
        return;
    }

    // [From `hyper::Server` in 0.14](https://github.com/hyperium/hyper/blob/v0.14.27/src/server/tcp.rs#L186)
    //
    // > A possible scenario is that the process has hit the max open files
    // > allowed, and so trying to accept a new connection will fail with
    // > `EMFILE`. In some cases, it's preferable to just wait for some time, if
    // > the application will likely close some files (or connections), and try
    // > to accept the connection again. If this option is `true`, the error
    // > will be logged at the `error` level, since it is still a big deal,
    // > and then the listener will sleep for 1 second.
    //
    // hyper allowed customizing this but axum does not.
    tracing::error!("accept error: {e}");
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

fn is_connection_error(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::ConnectionReset
    )
}

impl axum::serve::Listener for InterProcessListener {
    type Io = InterProcessStream;
    // FIXME: it should be supported by upstream, or waiting for upstream got supported listener trait
    type Addr = String;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match self.0.accept().await {
                Ok(stream) => return (stream, self.1.clone()),
                Err(e) => handle_accept_error(e).await,
            }
        }
    }

    #[inline]
    fn local_addr(&self) -> tokio::io::Result<Self::Addr> {
        Ok(self.1.clone())
    }
}

#[instrument(skip(with_graceful_shutdown))]
pub async fn create_server(
    placeholder: &str,
    app: Router,
    with_graceful_shutdown: Option<impl Future<Output = ()> + Send + 'static>,
    #[cfg(windows)] sids: &[&str],
    #[cfg(not(windows))] sids: (),
) -> Result<()> {
    let name_str = crate::utils::get_name_string(placeholder);
    let name = name_str.as_str().to_fs_name::<GenericFilePath>()?;
    #[cfg(unix)]
    {
        crate::utils::remove_socket_if_exists(placeholder).await?;
    }
    tracing::debug!("socket name: {:?}", name);
    let options = ListenerOptions::new()
        .name(name)
        .nonblocking(ListenerNonblockingMode::Both);
    #[cfg(windows)]
    let options = {
        use anyhow::Context;
        use widestring::U16CString;
        let sdsf = crate::utils::acl::generate_windows_security_descriptor(sids, None, None)
            .context("failed to generate sdsf")?;
        let sdsf = U16CString::from_str(&sdsf).context("failed to convert sdsf to u16cstring")?;
        let sw = SecurityDescriptor::deserialize(&sdsf)?;
        options.security_descriptor(sw)
    };
    // Set the mode atomically on platforms that support fchmod() on socket descriptors.
    // Other Unix platforms use change_socket_mode() after the socket path is created.
    let options = configure_listener_mode(options);

    let listener = options.create_tokio()?;
    let listener = InterProcessListener(listener, name_str);
    // change the socket group
    tracing::debug!("changing socket group and permissions...");
    crate::utils::os::change_socket_group(placeholder)?;
    crate::utils::os::change_socket_mode(placeholder)?;

    tracing::debug!("mounting service...");
    let server = axum::serve(listener, app);
    match with_graceful_shutdown {
        Some(graceful_shutdown) => server.with_graceful_shutdown(graceful_shutdown).await?,
        None => server.await?,
    };
    Ok(())
}

/// Mount operation handlers from the shared contract.
///
/// The path and the method come from the `IpcOperation` impl, so a route
/// cannot drift from the client that calls it.
pub trait RegisterOperation<S> {
    /// Mount `handler` at `op`'s path and method.
    ///
    /// `op` is a value rather than a turbofish so the call sites read as
    /// `.register(CoreStart, start::start)`.
    fn register<Op, H, T>(self, op: Op, handler: H) -> Self
    where
        Op: crate::api::contract::IpcOperation,
        H: axum::handler::Handler<T, S>,
        T: 'static;
}

impl<S> RegisterOperation<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn register<Op, H, T>(self, _op: Op, handler: H) -> Self
    where
        Op: crate::api::contract::IpcOperation,
        H: axum::handler::Handler<T, S>,
        T: 'static,
    {
        let filter = axum::routing::MethodFilter::try_from(Op::METHOD)
            .expect("every IPC operation uses a method axum can filter on");
        self.route(Op::PATH, axum::routing::on(filter, handler))
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn listener_mode_configuration_is_supported_on_macos() {
        let socket_path = format!("/tmp/nyanpasu-ipc-test-{}.sock", std::process::id());
        let _ = std::fs::remove_file(&socket_path);
        let name = socket_path
            .as_str()
            .to_fs_name::<GenericFilePath>()
            .expect("temporary socket path should be valid");

        let listener = configure_listener_mode(
            ListenerOptions::new()
                .name(name)
                .nonblocking(ListenerNonblockingMode::Both),
        )
        .create_tokio()
        .expect("macOS should create the listener without an unsupported socket mode");

        assert!(std::path::Path::new(&socket_path).exists());
        drop(listener);
        let _ = std::fs::remove_file(socket_path);
    }
}
