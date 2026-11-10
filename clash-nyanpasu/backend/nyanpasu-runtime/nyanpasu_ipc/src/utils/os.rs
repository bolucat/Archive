#![allow(unused_variables)]
use std::io::Result;

use tracing_attributes::instrument;

pub const NYANPASU_USER_GROUP: &str = "nyanpasu";

#[instrument]
pub(crate) fn change_socket_owner(placeholder: &str, owner_uid: Option<u32>) -> Result<()> {
    #[cfg(not(windows))]
    {
        use std::{io::Error as IoError, process::Command};
        let output = Command::new("chown")
            .arg(socket_owner(owner_uid))
            .arg(format!("/var/run/{placeholder}.sock"))
            .output()?;
        tracing::debug!("output: {:?}", output);
        if !output.status.success() {
            return Err(IoError::other("failed to change socket owner"));
        }
    }
    Ok(())
}

#[instrument]
pub(crate) fn change_socket_mode(placeholder: &str, owner_uid: Option<u32>) -> Result<()> {
    #[cfg(not(windows))]
    {
        use std::{io::Error as IoError, process::Command};
        let output = Command::new("chmod")
            .arg(socket_mode(owner_uid))
            .arg(format!("/var/run/{placeholder}.sock"))
            .output()?;
        tracing::debug!("output: {:?}", output);
        if !output.status.success() {
            return Err(IoError::other("failed to change socket mode"));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn socket_owner(owner_uid: Option<u32>) -> String {
    owner_uid.map_or_else(
        || format!("root:{NYANPASU_USER_GROUP}"),
        |uid| format!("{uid}:{NYANPASU_USER_GROUP}"),
    )
}

#[cfg(not(windows))]
fn socket_mode(owner_uid: Option<u32>) -> &'static str {
    if owner_uid.is_some() { "660" } else { "664" }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::{socket_mode, socket_owner};

    #[test]
    fn the_desktop_owner_can_access_the_socket_without_relogin() {
        assert_eq!(socket_owner(Some(1000)), "1000:nyanpasu");
        assert_eq!(socket_mode(Some(1000)), "660");
    }

    #[test]
    fn missing_owner_preserves_the_group_only_policy() {
        assert_eq!(socket_owner(None), "root:nyanpasu");
        assert_eq!(socket_mode(None), "664");
    }
}
