use thiserror::Error;

#[derive(Debug, Error)]
pub enum GeoDataError {
	#[error("failed to decode geodata: {0}")]
	Decode(String),
	#[error("I/O error: {0}")]
	Io(#[from] std::io::Error),
	#[error("failed to serialize rkyv snapshot: {0}")]
	Serialize(String),
	#[error("cache file too small or truncated")]
	Truncated,
	#[error("not a wind-geodata cache file (bad magic)")]
	BadMagic,
	#[error("unsupported cache format version: {0}")]
	UnsupportedVersion(u32),
	#[error("failed to validate rkyv snapshot: {0}")]
	Validate(String),
}
