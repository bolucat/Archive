//! Cross-platform shutdown-signal handling.
//!
//! Long-running servers should drain gracefully on the signals an operator (or
//! a service manager / container runtime) actually sends. On Unix that means
//! both `SIGINT` (Ctrl-C) and `SIGTERM` — the latter is what `systemctl stop`,
//! `docker stop`, and Kubernetes pod termination deliver, and without it the
//! process is left to be hard-killed after the grace period. On Windows there
//! is no `SIGTERM`, so only Ctrl-C applies.

use std::future::pending;

use tracing::warn;

/// Resolve once the first OS shutdown signal arrives.
///
/// Awaits Ctrl-C on every platform and, additionally on Unix, `SIGTERM`.
/// Returns as soon as either fires. If a handler can't be installed (rare),
/// that arm parks forever instead of resolving so it never spuriously triggers
/// shutdown — the other signal can still win.
pub async fn shutdown_signal() {
	let ctrl_c = async {
		if let Err(err) = tokio::signal::ctrl_c().await {
			warn!("failed to install Ctrl-C handler: {err}");
			pending::<()>().await;
		}
	};

	#[cfg(unix)]
	let terminate = async {
		use tokio::signal::unix::{SignalKind, signal};
		match signal(SignalKind::terminate()) {
			Ok(mut sig) => {
				sig.recv().await;
			}
			Err(err) => {
				warn!("failed to install SIGTERM handler: {err}");
				pending::<()>().await;
			}
		}
	};

	#[cfg(not(unix))]
	let terminate = pending::<()>();

	tokio::select! {
		_ = ctrl_c => {}
		_ = terminate => {}
	}
}
