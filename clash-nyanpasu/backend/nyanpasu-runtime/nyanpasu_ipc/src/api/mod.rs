pub mod contract;
pub mod core;
pub mod log;
pub mod network;
pub mod status;
pub mod ws;

use serde::{Deserialize, Serialize, de::DeserializeOwned};

/// Stable machine-readable failure kinds carried by [`R::error_kind`].
///
/// These strings are protocol: a caller branches on them instead of parsing
/// `msg`. [`ResponseCode`] deliberately stays a two-variant enum — a new
/// variant would make every older client fail to decode the whole envelope —
/// so the detail lives in an additive string field instead (report §4 P0-B).
///
/// Absent means "not classified", never "no error": only the operations added
/// in S8 populate it, and only for the failures listed here. Mapping the
/// manager's remaining error variants is report P3.
pub use nyanpasu_core_metadata::CoreErrorKind;
use std::{borrow::Cow, fmt::Debug, io::Error as IoError};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default, PartialEq)]
pub enum ResponseCode {
    #[default]
    Ok = 0,
    OtherError = -1,
}

/// ResponseCode message mapping
impl ResponseCode {
    pub const fn msg(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::OtherError => "other error",
        }
    }
}

/// The IPC Response body definition
#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[builder(build_fn(validate = "Self::validate"))]
#[serde(bound = "T: Serialize + DeserializeOwned")]
pub struct R<'a, T: Serialize + DeserializeOwned + Debug> {
    pub code: ResponseCode,
    #[builder(default = "self.default_msg()")]
    pub msg: Cow<'a, str>,
    #[builder(setter(into, strip_option))]
    pub data: Option<T>,
    #[builder(setter(skip), default = "self.default_ts()")]
    pub ts: i64,
    /// Machine-readable failure classification, see [`CoreErrorKind`]. Declared
    /// last and omitted when absent, so every pre-S8 envelope stays
    /// byte-identical and every pre-S8 payload still decodes.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<Cow<'a, str>>,
    /// Whether retrying the same request could succeed. Absent means the
    /// service did not say — never "no": a caller that needs a decision here
    /// must fall back to [`CoreErrorKind::default_retryable`], because an older
    /// service omits the field entirely.
    #[builder(default)]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
}

impl<T: Serialize + DeserializeOwned + Debug> R<'_, T> {
    pub fn ok(self) -> Result<Self, IoError> {
        if self.code == ResponseCode::Ok {
            Ok(self)
        } else {
            Err(IoError::other(format!(
                "Response code is not Ok: {self:#?}"
            )))
        }
    }
}

impl<'a, T: Serialize + DeserializeOwned + Debug> RBuilder<'a, T> {
    fn default_ts(&self) -> i64 {
        crate::utils::get_current_ts()
    }

    fn default_msg(&self) -> Cow<'a, str> {
        Cow::Borrowed(if let Some(code) = self.code {
            code.msg()
        } else {
            ResponseCode::Ok.msg()
        })
    }

    fn validate(&self) -> Result<(), String> {
        if self.code.is_none() {
            return Err("code is required".to_string());
        }
        if self.msg.is_none() {
            return Err("msg is required".to_string());
        }
        Ok(())
    }

    pub fn other_error(msg: Cow<'a, str>) -> R<'a, T> {
        let code = ResponseCode::OtherError;
        R {
            code,
            msg,
            data: None,
            ts: crate::utils::get_current_ts(),
            error_kind: None,
            retryable: None,
        }
    }

    /// An error envelope carrying a [`CoreErrorKind`] classification.
    ///
    /// `kind` is `Option` because a failure the service cannot classify must
    /// still be reportable — guessing a kind is worse than omitting it. Taking
    /// the enum rather than a string is what keeps a hand-typed kind off the
    /// wire. `retryable` is `Option` for the same reason, and is the producer's
    /// answer rather than the kind's default when both exist.
    pub fn other_error_with_kind(
        msg: Cow<'a, str>,
        kind: Option<CoreErrorKind>,
        retryable: Option<bool>,
    ) -> R<'a, T> {
        let code = ResponseCode::OtherError;
        R {
            code,
            msg,
            data: None,
            ts: crate::utils::get_current_ts(),
            error_kind: kind.map(|kind| Cow::Borrowed(kind.as_str())),
            retryable,
        }
    }

    pub fn success(data: T) -> R<'a, T> {
        let code = ResponseCode::Ok;
        R {
            code,
            msg: Cow::Borrowed(code.msg()),
            data: Some(data),
            ts: crate::utils::get_current_ts(),
            error_kind: None,
            retryable: None,
        }
    }
}
