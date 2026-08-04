use std::{backtrace::Backtrace, str::Utf8Error};

use snafu::prelude::*;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum ProtoError {
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
	NumericOverflow {
		field: String,
		num: String,
		backtrace: Backtrace,
	},
}

impl From<std::io::Error> for ProtoError {
	#[inline(always)]
	fn from(source: std::io::Error) -> Self {
		// `Decoder::Error: From<io::Error>` is required by tokio-util's `Framed*`,
		// and the underlying transport's IO errors (e.g. a peer reset) flow
		// through here. The previous debug-build `panic!` turned any such error
		// into a crash the moment these codecs were driven over real IO, so map
		// it to the `Io` variant in all builds instead.
		use snafu::IntoError as _;
		IoSnafu.into_error(source)
	}
}
