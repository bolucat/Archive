use std::{backtrace::Backtrace, str::Utf8Error};

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum ProtoError {
	#[snafu(display("Version mismatch: expected {expect}, got {current}"))]
	VersionMismatch {
		expect: u8,
		current: u8,
		backtrace: Backtrace,
	},
	#[snafu(display("Unknown command type {value}"))]
	UnknownCommandType {
		value: u8,
		backtrace: Backtrace,
	},
	#[snafu(display("Unable to decode address due to type {value}"))]
	UnknownAddressType {
		value: u8,
		backtrace: Backtrace,
	},
	#[snafu(display("Unable to decode domain {raw} as UTF-8"))]
	FailParseDomain {
		// HEX
		raw: String,
		source: Utf8Error,
		backtrace: Backtrace,
	},
	DomainTooLong {
		domain: String,
		backtrace: Backtrace,
	},
	// Caller should yield
	BytesRemaining,
	Io {
		source: std::io::Error,
		backtrace: Backtrace,
	},
	#[snafu(display("{field} {num} does not fit into its wire field"))]
	NumericOverflow {
		field: String,
		num: String,
		backtrace: Backtrace,
	},
}

/// Structured failure of the nom parsers in this module.
///
/// The parsers used to report every failure as `ErrorKind::Verify`, so the
/// `Decoder` impls collapsed a version mismatch, an unknown command type, an
/// unknown address type and a non-UTF-8 domain into the generic
/// [`BytesRemaining`](ProtoError::BytesRemaining) error — the specific signal
/// was lost, and the matching [`ProtoError`] variants had no construction
/// point at all. Naming the failure here keeps both paths informative:
/// `From<ParseError> for ProtoError` feeds the codecs and the `Display` impl
/// feeds the `eyre`-based free helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseError {
	VersionMismatch {
		expect: u8,
		current: u8,
	},
	UnknownCommandType {
		value: u8,
	},
	UnknownAddressType {
		value: u8,
	},
	FailParseDomain {
		raw: String,
		source: Utf8Error,
	},
	/// Any other structural failure reported by a nom combinator.
	Verify,
}

impl nom::error::ParseError<&[u8]> for ParseError {
	fn from_error_kind(_input: &[u8], _kind: nom::error::ErrorKind) -> Self {
		Self::Verify
	}

	fn append(_input: &[u8], _kind: nom::error::ErrorKind, other: Self) -> Self {
		other
	}
}

impl std::fmt::Display for ParseError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// The wording mirrors the `ProtoError` display attributes below so the
		// codec path and the free-helper path report the same failure.
		match self {
			Self::VersionMismatch { expect, current } => write!(f, "Version mismatch: expected {expect}, got {current}"),
			Self::UnknownCommandType { value } => write!(f, "Unknown command type {value}"),
			Self::UnknownAddressType { value } => write!(f, "Unable to decode address due to type {value}"),
			Self::FailParseDomain { raw, .. } => write!(f, "Unable to decode domain {raw} as UTF-8"),
			Self::Verify => write!(f, "Malformed input"),
		}
	}
}

impl From<ParseError> for ProtoError {
	fn from(value: ParseError) -> Self {
		use snafu::IntoError as _;

		match value {
			ParseError::VersionMismatch { expect, current } => VersionMismatchSnafu { expect, current }.build(),
			ParseError::UnknownCommandType { value } => UnknownCommandTypeSnafu { value }.build(),
			ParseError::UnknownAddressType { value } => UnknownAddressTypeSnafu { value }.build(),
			ParseError::FailParseDomain { raw, source } => FailParseDomainSnafu { raw }.into_error(source),
			// A structural failure with no more specific name keeps the historic
			// "caller should yield" signal that `decode_eof` also produces.
			ParseError::Verify => BytesRemainingSnafu.build(),
		}
	}
}

impl From<std::io::Error> for ProtoError {
	#[inline(always)]
	fn from(source: std::io::Error) -> Self {
		// `Decoder::Error: From<io::Error>` is required by tokio-util's
		// `Framed*`, and the underlying transport's IO errors (e.g. a peer
		// reset) flow through here. The previous debug-build `panic!` turned
		// any such error into a crash the moment these codecs were driven
		// over real IO, so map it to the `Io` variant in all builds instead.
		use snafu::IntoError as _;
		IoSnafu.into_error(source)
	}
}
