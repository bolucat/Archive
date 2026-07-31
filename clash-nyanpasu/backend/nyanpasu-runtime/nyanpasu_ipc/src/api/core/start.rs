use crate::api::R;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::PathBuf};

pub const CORE_START_ENDPOINT: &str = "/core/start";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct CoreStartReq<'n> {
    pub core_type: Cow<'n, nyanpasu_utils::core::CoreType>,
    pub config_file: Cow<'n, PathBuf>,
}

pub type CoreStartRes<'a> = R<'a, ()>;
