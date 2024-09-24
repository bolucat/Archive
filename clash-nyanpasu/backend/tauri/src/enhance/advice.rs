#[allow(unused_imports)]
use crate::enhance::{script::runner::ProcessOutput, Logs, LogsExt};
use rust_i18n::t;
use serde_yaml::Mapping;

// TODO: add more advice for chain
pub fn chain_advice(config: &Mapping) -> ProcessOutput {
    #[allow(unused_mut)]
    let mut logs = Logs::default();
    if config.get("tun").is_some_and(|val| {
        val.is_mapping()
            && !val
                .as_mapping()
                .unwrap()
                .get("enable")
                .is_some_and(|val| val.as_bool().unwrap_or(false))
    }) {
        let service_state = crate::core::service::ipc::get_ipc_state();
        // show a warning dialog if the user has no permission to enable tun
        #[cfg(windows)]
        {
            use deelevate::{PrivilegeLevel, Token};
            let level = {
                match Token::with_current_process() {
                    Ok(token) => token
                        .privilege_level()
                        .unwrap_or(PrivilegeLevel::NotPrivileged),
                    Err(_) => PrivilegeLevel::NotPrivileged,
                }
            };
            if level == PrivilegeLevel::NotPrivileged && !service_state.is_connected() {
                let msg = t!("dialog.warning.enable_tun_with_no_permission");
                logs.warn(msg.as_ref());
                crate::utils::dialog::warning_dialog(msg.as_ref());
            }
        }
        // If the core file is not granted the necessary permissions, grant it
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            #[cfg(target_os = "macos")]
            const ROOT_GROUP: &str = "admin";
            #[cfg(target_os = "linux")]
            const ROOT_GROUP: &str = "root";

            use nix::unistd::{Gid, Group as NixGroup, Uid, User};
            use std::os::unix::fs::MetadataExt;
            if !service_state.is_connected() {
                let core: nyanpasu_utils::core::CoreType = {
                    crate::config::Config::verge()
                        .latest()
                        .clash_core
                        .as_ref()
                        .unwrap_or(&crate::config::nyanpasu::ClashCore::default())
                        .into()
                };
                let core_path = crate::core::clash::core::find_binary_path(&core);
                if let Ok(core_path) = core_path {
                    if let Ok(metadata) = std::fs::metadata(&core_path) {
                        let uid = metadata.uid();
                        let gid = metadata.gid();
                        let user = User::from_uid(Uid::from_raw(uid)).ok().flatten();
                        let group = NixGroup::from_gid(Gid::from_raw(gid)).ok().flatten();
                        if let (Some(user), Some(group)) = (user, group) {
                            if !*crate::consts::IS_APPIMAGE
                                && (user.name != "root" || group.name != ROOT_GROUP)
                            {
                                tracing::warn!("The core file is not granted the necessary permissions, grant it");
                                let msg = t!("dialog.info.grant_core_permission");
                                if crate::utils::dialog::ask_dialog(msg.as_ref()) {
                                    if let Err(err) = crate::core::manager::grant_permission(&core)
                                    {
                                        tracing::error!(
                                            "Failed to grant permission to the core file: {}",
                                            err
                                        );
                                        crate::utils::dialog::error_dialog(format!(
                                            "failed to grant core permission:\n{:#?}",
                                            err
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    (Ok(Mapping::new()), logs)
}
