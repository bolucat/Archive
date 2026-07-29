//! Per-project ACL / routing rule dialects ("syntaxes").
//!
//! Each submodule parses one upstream project's rule format into the shared
//! wind representation (`wind_core::rule::Rule` / this crate's [`Ruleset`]):
//!
//! * [`apernet`] — the real Hysteria 2 ACL (`apernet/hysteria`), a
//!   function-call form: `reject(geoip:cn)`, `default(8.8.8.8, udp/53,
//!   1.1.1.1)`.
//! * [`metacubex`] — Clash / Mihomo rule lines (`MetaCubeX/mihomo`):
//!   `DOMAIN-SUFFIX,google.com,proxy`.
//! * `sagernet` — sing-box route rules (`SagerNet/sing-box`). Future work.
//!
//! The space-separated legacy ACL dialect (`proxy 10.0.0.0/8 tcp/443`) is
//! specific to tuic-server and lives in the `tuic-server` crate's `legacy`
//! module, not here — it is unrelated to apernet's function-call form despite
//! the superficial resemblance.
//!
//! [`Ruleset`]: crate::Ruleset

pub mod apernet;
pub mod metacubex;
