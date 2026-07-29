//! The cloneable [`QuicConnection`] handle for the quiche backend.
//!
//! All methods translate into messages to the connection's
//! [`BridgeDriver`](crate::quiche::driver::BridgeDriver): commands over an
//! unbounded channel, stream/datagram accepts over receive channels (guarded by
//! async mutexes because the trait methods take `&self` on a `Clone` handle).

use std::{
	net::SocketAddr,
	sync::{Arc, atomic::Ordering},
};

use bytes::Bytes;
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::{
	error::QuicError,
	quiche::{
		driver::{CmdTx, DriverCommand, Shared},
		stream::{QuicheRecv, QuicheSend},
	},
	traits::QuicConnection,
};

struct Inner {
	cmd_tx: CmdTx,
	accept_bi: Mutex<mpsc::UnboundedReceiver<(QuicheSend, QuicheRecv)>>,
	accept_uni: Mutex<mpsc::UnboundedReceiver<QuicheRecv>>,
	dgram_in: Mutex<mpsc::UnboundedReceiver<Bytes>>,
	shared: Arc<Shared>,
	peer_addr: SocketAddr,
}

/// A [`QuicConnection`] backed by quiche / tokio-quiche.
#[derive(Clone)]
pub struct QuicheConnection(Arc<Inner>);

impl QuicheConnection {
	pub(crate) fn new(
		cmd_tx: CmdTx,
		accept_bi: mpsc::UnboundedReceiver<(QuicheSend, QuicheRecv)>,
		accept_uni: mpsc::UnboundedReceiver<QuicheRecv>,
		dgram_in: mpsc::UnboundedReceiver<Bytes>,
		shared: Arc<Shared>,
		peer_addr: SocketAddr,
	) -> Self {
		Self(Arc::new(Inner {
			cmd_tx,
			accept_bi: Mutex::new(accept_bi),
			accept_uni: Mutex::new(accept_uni),
			dgram_in: Mutex::new(dgram_in),
			shared,
			peer_addr,
		}))
	}
}

impl QuicConnection for QuicheConnection {
	type RecvStream = QuicheRecv;
	type SendStream = QuicheSend;

	async fn open_bi(&self) -> Result<(QuicheSend, QuicheRecv), QuicError> {
		let (tx, rx) = oneshot::channel();
		self.0
			.cmd_tx
			.send(DriverCommand::OpenBi(tx))
			.map_err(|_| QuicError::LocallyClosed)?;
		rx.await.map_err(|_| QuicError::ConnectionLost("driver closed".into()))
	}

	async fn accept_bi(&self) -> Result<(QuicheSend, QuicheRecv), QuicError> {
		let mut guard = self.0.accept_bi.lock().await;
		guard.recv().await.ok_or(QuicError::LocallyClosed)
	}

	async fn open_uni(&self) -> Result<QuicheSend, QuicError> {
		let (tx, rx) = oneshot::channel();
		self.0
			.cmd_tx
			.send(DriverCommand::OpenUni(tx))
			.map_err(|_| QuicError::LocallyClosed)?;
		rx.await.map_err(|_| QuicError::ConnectionLost("driver closed".into()))
	}

	async fn accept_uni(&self) -> Result<QuicheRecv, QuicError> {
		let mut guard = self.0.accept_uni.lock().await;
		guard.recv().await.ok_or(QuicError::LocallyClosed)
	}

	fn send_datagram(&self, data: Bytes) -> Result<(), QuicError> {
		if let Some(max) = self.max_datagram_size()
			&& data.len() > max
		{
			return Err(QuicError::Datagram(format!("datagram {} bytes > max {}", data.len(), max)));
		}
		self.0
			.cmd_tx
			.send(DriverCommand::SendDatagram(data))
			.map_err(|_| QuicError::LocallyClosed)
	}

	async fn read_datagram(&self) -> Result<Bytes, QuicError> {
		let mut guard = self.0.dgram_in.lock().await;
		guard.recv().await.ok_or(QuicError::LocallyClosed)
	}

	fn max_datagram_size(&self) -> Option<usize> {
		match self.0.shared.max_dgram.load(Ordering::Relaxed) {
			0 => None,
			n => Some(n),
		}
	}

	async fn export_keying_material<'a>(
		&'a self,
		out: &'a mut [u8],
		label: &'a [u8],
		context: &'a [u8],
	) -> Result<(), QuicError> {
		let (tx, rx) = oneshot::channel();
		self.0
			.cmd_tx
			.send(DriverCommand::Export {
				out_len: out.len(),
				label: label.to_vec(),
				context: context.to_vec(),
				reply: tx,
			})
			.map_err(|_| QuicError::LocallyClosed)?;
		match rx.await {
			Ok(Some(v)) => {
				let n = v.len().min(out.len());
				out[..n].copy_from_slice(&v[..n]);
				Ok(())
			}
			Ok(None) => Err(QuicError::Tls("export_keying_material failed".into())),
			Err(_) => Err(QuicError::ConnectionLost("driver closed".into())),
		}
	}

	fn close(&self, code: u32, reason: &[u8]) {
		let _ = self.0.cmd_tx.send(DriverCommand::Close {
			code,
			reason: reason.to_vec(),
		});
	}

	async fn closed(&self) {
		// Standard `Notify` pattern: register interest, re-check the flag, then
		// await — so a close racing between the check and the await is not lost.
		loop {
			if self.0.shared.closed.load(Ordering::SeqCst) {
				return;
			}
			let notified = self.0.shared.closed_notify.notified();
			if self.0.shared.closed.load(Ordering::SeqCst) {
				return;
			}
			notified.await;
		}
	}

	fn peer_addr(&self) -> Option<SocketAddr> {
		Some(self.0.peer_addr)
	}

	async fn byte_stats(&self) -> Option<(u64, u64)> {
		// Read the counters the driver caches in `shared`; no round-trip, so this
		// still works during the close path after the driver's worker loop exits.
		let shared = &self.0.shared;
		Some((
			shared.sent_bytes.load(Ordering::Relaxed),
			shared.recv_bytes.load(Ordering::Relaxed),
		))
	}
}
