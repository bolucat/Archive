use semver::{Version, VersionReq};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreVersion {
    Release(Version),
    Nightly,
    Unknown,
}

impl CoreVersion {
    /// Accepts either a bare version or a complete `-v` banner, taking the
    /// first recognizable token from left to right.
    pub fn parse(raw: &str) -> Self {
        for token in raw.split_whitespace() {
            let token = token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && !matches!(character, '.' | '-' | '+' | '_')
            });
            let version = token.strip_prefix('v').unwrap_or(token);
            if let Ok(version) = Version::parse(version) {
                return Self::Release(Version::new(version.major, version.minor, version.patch));
            }
            if ["alpha-", "nightly-"].iter().any(|prefix| {
                token
                    .strip_prefix(prefix)
                    .is_some_and(|value| !value.is_empty())
            }) {
                return Self::Nightly;
            }
        }
        Self::Unknown
    }
}

pub mod clash;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    Yes,
    /// Since `VersionReq`, the core supports this feature.
    Since(VersionReq),
    No,
}
