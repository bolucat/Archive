//! Minimal SOCKS5 plumbing for real-core integration tests.
//!
//! The server implements just enough of RFC 1928 for a clash-family core to
//! use it as an outbound `socks5` proxy node: version-5 handshake with
//! no-auth, CONNECT command, then a plain TCP relay. The client helper plays
//! the other side, so tests can drive traffic through a core's inbound
//! (mixed port) and assert it comes out the far end of the chain:
//!
//! test client → core inbound → core `socks5` outbound node → this server →
//! echo target.

use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

/// A running SOCKS5 server. Dropping it aborts the accept loop.
pub struct Socks5Server {
    port: u16,
    connections: Arc<AtomicUsize>,
    task: JoinHandle<()>,
}

impl Socks5Server {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind socks5 server");
        let port = listener.local_addr().expect("local addr").port();
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = connections.clone();
        let task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((mut stream, _)) => {
                        counter.fetch_add(1, Ordering::Relaxed);
                        tokio::spawn(async move {
                            let _ = handle_session(&mut stream).await;
                        });
                    }
                    Err(_) => return,
                }
            }
        });
        Self {
            port,
            connections,
            task,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// How many core-side connections the server has accepted so far.
    pub fn connection_count(&self) -> usize {
        self.connections.load(Ordering::Relaxed)
    }
}

impl Drop for Socks5Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn read_exact<const N: usize>(stream: &mut TcpStream) -> io::Result<[u8; N]> {
    let mut buf = [0u8; N];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

async fn handle_session(mut stream: &mut TcpStream) -> io::Result<()> {
    // Greeting: VER, NMETHODS, METHODS...
    let [ver, nmethods] = read_exact(stream).await?;
    if ver != 0x05 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not socks5"));
    }
    let mut methods = vec![0u8; nmethods as usize];
    stream.read_exact(&mut methods).await?;
    // VER, no-auth selected.
    stream.write_all(&[0x05, 0x00]).await?;

    // Request: VER, CMD, RSV, ATYP, DST.ADDR, DST.PORT
    let [ver, cmd, _rsv, atyp] = read_exact(stream).await?;
    if ver != 0x05 || cmd != 0x01 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "only CONNECT is supported",
        ));
    }
    let host = match atyp {
        0x01 => {
            let octets = read_exact::<4>(stream).await?;
            std::net::Ipv4Addr::from(octets).to_string()
        }
        0x03 => {
            let [len] = read_exact(stream).await?;
            let mut name = vec![0u8; len as usize];
            stream.read_exact(&mut name).await?;
            String::from_utf8(name)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad domain"))?
        }
        0x04 => {
            let octets = read_exact::<16>(stream).await?;
            std::net::Ipv6Addr::from(octets).to_string()
        }
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "bad atyp")),
    };
    let port = {
        let [hi, lo] = read_exact(stream).await?;
        u16::from_be_bytes([hi, lo])
    };

    let mut target = match TcpStream::connect((host.as_str(), port)).await {
        Ok(target) => target,
        Err(_) => {
            // Reply: connection refused (REP=0x05).
            stream
                .write_all(&[0x05, 0x05, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Ok(());
        }
    };
    // Reply: succeeded.
    stream
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;
    tokio::io::copy_bidirectional(&mut stream, &mut target).await?;
    Ok(())
}

/// Connects to a SOCKS5 proxy at `proxy_port` and issues CONNECT to
/// `target_host:target_port`, returning the relay-ready stream.
pub async fn connect_via(
    proxy_port: u16,
    target_host: &str,
    target_port: u16,
) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect(("127.0.0.1", proxy_port)).await?;
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    let [ver, method] = read_exact(&mut stream).await?;
    if ver != 0x05 || method == 0xff {
        return Err(io::Error::other("no acceptable method"));
    }

    let mut request = vec![0x05, 0x01, 0x00];
    if let Ok(octets) = target_host.parse::<std::net::Ipv4Addr>() {
        request.push(0x01);
        request.extend_from_slice(&octets.octets());
    } else {
        request.push(0x03);
        request.push(target_host.len() as u8);
        request.extend_from_slice(target_host.as_bytes());
    }
    request.extend_from_slice(&target_port.to_be_bytes());
    stream.write_all(&request).await?;

    let [ver, rep, _rsv, atyp] = read_exact(&mut stream).await?;
    if ver != 0x05 || rep != 0x00 {
        return Err(io::Error::other(format!("connect failed with rep={rep}")));
    }
    // Consume BND.ADDR + BND.PORT.
    match atyp {
        0x01 => {
            read_exact::<4>(&mut stream).await?;
        }
        0x03 => {
            let [len] = read_exact(&mut stream).await?;
            let mut name = vec![0u8; len as usize];
            stream.read_exact(&mut name).await?;
        }
        0x04 => {
            read_exact::<16>(&mut stream).await?;
        }
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "bad atyp")),
    }
    read_exact::<2>(&mut stream).await?;
    Ok(stream)
}

/// A TCP echo server; returns its port. Everything sent to it comes back.
pub async fn echo_server() -> (u16, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind echo server");
    let port = listener.local_addr().expect("local addr").port();
    let task = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((mut stream, _)) => {
                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        loop {
                            match stream.read(&mut buf).await {
                                Ok(0) | Err(_) => return,
                                Ok(n) => {
                                    if stream.write_all(&buf[..n]).await.is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    });
                }
                Err(_) => return,
            }
        }
    });
    (port, task)
}
