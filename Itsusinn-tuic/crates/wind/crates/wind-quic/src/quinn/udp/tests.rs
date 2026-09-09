use std::{
	collections::VecDeque,
	sync::{
		Arc, Mutex,
		atomic::{AtomicUsize, Ordering},
	},
};

use quinn::{ClientConfig, Connection, Endpoint, EndpointConfig, ServerConfig};
use rustls::pki_types::PrivatePkcs8KeyDer;

use super::*;

#[derive(Debug)]
struct FaultSocket {
	inner: Box<dyn AsyncUdpSocket>,
	errors: Arc<Mutex<VecDeque<io::ErrorKind>>>,
	polls: Arc<AtomicUsize>,
}

impl AsyncUdpSocket for FaultSocket {
	fn create_sender(&self) -> Pin<Box<dyn UdpSender>> {
		self.inner.create_sender()
	}

	fn poll_recv(
		&mut self,
		cx: &mut Context<'_>,
		bufs: &mut [IoSliceMut<'_>],
		meta: &mut [RecvMeta],
	) -> Poll<io::Result<usize>> {
		self.polls.fetch_add(1, Ordering::SeqCst);
		if let Some(kind) = self.errors.lock().unwrap().pop_front() {
			return Poll::Ready(Err(kind.into()));
		}
		self.inner.poll_recv(cx, bufs, meta)
	}

	fn local_addr(&self) -> io::Result<SocketAddr> {
		self.inner.local_addr()
	}

	fn max_receive_segments(&self) -> usize {
		self.inner.max_receive_segments()
	}

	fn may_fragment(&self) -> bool {
		self.inner.may_fragment()
	}
}

fn fault_socket() -> FaultSocket {
	let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
	FaultSocket {
		inner: TokioRuntime.wrap_udp_socket(socket).unwrap(),
		errors: Arc::default(),
		polls: Arc::default(),
	}
}

#[tokio::test]
async fn repeated_peer_errors_yield_and_local_errors_propagate() {
	let fault = fault_socket();
	let errors = fault.errors.clone();
	let polls = fault.polls.clone();
	errors.lock().unwrap().extend([io::ErrorKind::ConnectionRefused; 100]);
	let mut socket = ServerUdpSocket::new(Box::new(fault));
	let mut buf = [0; 1500];
	let mut bufs = [IoSliceMut::new(&mut buf)];
	let mut meta = [RecvMeta::default()];
	let mut cx = Context::from_waker(std::task::Waker::noop());
	assert!(socket.poll_recv(&mut cx, &mut bufs, &mut meta).is_pending());
	// Polling again without advancing the timer must not consume another error.
	assert!(socket.poll_recv(&mut cx, &mut bufs, &mut meta).is_pending());
	assert_eq!(polls.load(Ordering::SeqCst), 1);
	errors.lock().unwrap().clear();
	errors.lock().unwrap().push_back(io::ErrorKind::PermissionDenied);
	let result = tokio::time::timeout(
		Duration::from_secs(1),
		std::future::poll_fn(|cx| socket.poll_recv(cx, &mut bufs, &mut meta)),
	)
	.await
	.unwrap();
	assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
}

fn endpoints(protected: bool) -> (Endpoint, Endpoint, Arc<Mutex<VecDeque<io::ErrorKind>>>) {
	super::super::tls::ensure_provider();
	let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
	let server_config = ServerConfig::with_single_cert(
		vec![cert.cert.der().clone()],
		PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
	)
	.unwrap();
	let mut roots = rustls::RootCertStore::empty();
	roots.add(cert.cert.der().clone()).unwrap();
	let client_config = ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
	let fault = fault_socket();
	let errors = fault.errors.clone();
	let socket: Box<dyn AsyncUdpSocket> = if protected {
		Box::new(ServerUdpSocket::new(Box::new(fault)))
	} else {
		Box::new(fault)
	};
	let server =
		Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, Arc::new(TokioRuntime))
			.unwrap();
	let client = Endpoint::client("127.0.0.1:0".parse().unwrap()).unwrap();
	client.set_default_client_config(client_config);
	(server, client, errors)
}

async fn connect_pair(server: &Endpoint, client: &Endpoint) -> (Connection, Connection) {
	let (client, server) = tokio::join!(client.connect(server.local_addr().unwrap(), "localhost").unwrap(), async {
		server.accept().await.unwrap().await.unwrap()
	},);
	(client.unwrap(), server)
}

async fn echo(client: &Connection, server: &Connection) {
	let (mut send, mut recv) = client.open_bi().await.unwrap();
	send.write_all(b"ping").await.unwrap();
	send.finish().unwrap();
	let (mut reply, mut request) = server.accept_bi().await.unwrap();
	assert_eq!(request.read_to_end(16).await.unwrap(), b"ping");
	reply.write_all(b"pong").await.unwrap();
	reply.finish().unwrap();
	assert_eq!(recv.read_to_end(16).await.unwrap(), b"pong");
}

#[tokio::test]
async fn peer_errors_preserve_existing_connections_and_new_handshakes() {
	tokio::time::timeout(Duration::from_secs(10), async {
		let (server, client, errors) = endpoints(true);
		let (client_conn, server_conn) = connect_pair(&server, &client).await;
		echo(&client_conn, &server_conn).await;
		for kind in [
			io::ErrorKind::ConnectionRefused,
			io::ErrorKind::ConnectionReset,
			io::ErrorKind::HostUnreachable,
			io::ErrorKind::NetworkUnreachable,
		] {
			errors.lock().unwrap().push_back(kind);
			echo(&client_conn, &server_conn).await;
			assert!(errors.lock().unwrap().is_empty());
			let (new_client, new_server) = connect_pair(&server, &client).await;
			echo(&new_client, &new_server).await;
			new_client.close(0u32.into(), b"done");
		}
		assert!(client_conn.close_reason().is_none());
		assert!(server_conn.close_reason().is_none());
		server.close(0u32.into(), b"done");
		client.close(0u32.into(), b"done");
	})
	.await
	.expect("endpoint must remain usable after peer UDP errors");
}

#[tokio::test]
async fn unprotected_endpoint_reproduces_connection_refused_shutdown() {
	tokio::time::timeout(Duration::from_secs(5), async {
		let (server, client, errors) = endpoints(false);
		let (client_conn, server_conn) = connect_pair(&server, &client).await;
		echo(&client_conn, &server_conn).await;
		errors.lock().unwrap().push_back(io::ErrorKind::ConnectionRefused);
		client_conn.send_datagram(bytes::Bytes::from_static(b"trigger recv")).unwrap();
		server_conn.closed().await;
		assert!(errors.lock().unwrap().is_empty());
		assert!(server.accept().await.is_none());
		client.close(0u32.into(), b"done");
	})
	.await
	.expect("unprotected endpoint must reproduce driver loss");
}
