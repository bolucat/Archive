//! Registry of live authenticated TUIC connections — re-exported from
//! [`wind_core`], shared with the naive inbound. A TUIC connection is exactly
//! one authenticated user, so the per-connection cancel token is a precise kick
//! handle: cancelling it trips the `serve_connection` shutdown path, which
//! closes the QUIC connection.

pub use wind_core::ActiveConnections;
