//! TUIC protocol surface, backend-agnostic over [`wind_quic::QuicConnection`].
//!
//! The on-wire codecs, [`ProtoError`], and the pure decode helpers live in the
//! [`tuic_core::proto`] crate and are re-exported here so existing
//! `wind_tuic::proto::…` paths keep working. The connection-coupled glue
//! ([`ClientProtoExt`], [`encode_and_send_uni`], [`UdpStream`]) is written once
//! against the `QuicConnection` trait — shared by both backends — and gated on
//! the `encode` feature (it builds wire frames via the encoders).

pub use tuic_core::proto::*;

#[cfg(feature = "encode")]
mod client_proto;
#[cfg(feature = "encode")]
mod udp_stream;

#[cfg(feature = "encode")]
pub use client_proto::*;
#[cfg(feature = "encode")]
pub use udp_stream::*;
