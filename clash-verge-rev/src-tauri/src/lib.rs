mod cmd;
mod config;
mod core;
mod enhance;
mod feat;
mod utils;
mod module;
use crate::core::hotkey;
use crate::utils::{resolve, resolve::resolve_scheme, server};
use config::Config;
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_deep_link::DeepLinkExt;
use std::sync::{Mutex, Once};
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Manager;

/// A global singleton handle to the application.
pub struct AppHandleManager {
    inner: Mutex<Option<AppHandle>>,
    init: Once,
}

impl AppHandleManager {
    /// Get the global instance of the app handle manager.
    pub fn global() -> &'static Self {
        static INSTANCE: AppHandleManager = AppHandleManager {
            inner: Mutex::new(None),
            init: Once::new(),
        };
        &INSTANCE
    }

    /// Initialize the app handle manager with an app handle.
    pub fn init(&self, handle: AppHandle) {
        self.init.call_once(|| {
            let mut app_handle = self.inner.lock().unwrap();
            *app_handle = Some(handle);
        });
    }

    /// Get the app handle if it has been initialized.
    pub fn get(&self) -> Option<AppHandle> {
        self.inner.lock().unwrap().clone()
    }

    /// Get the app handle, panics if it hasn't been initialized.
    pub fn get_handle(&self) -> AppHandle {
        self.get().expect("AppHandle not initialized")
    }

    pub fn set_activation_policy_regular(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Regular);
        }
    }
    
    pub fn set_activation_policy_accessory(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Accessory);
        }
    }
    
    pub fn set_activation_policy_prohibited(&self) {
        #[cfg(target_os = "macos")]
        {
            let app_handle = self.inner.lock().unwrap();
            let app_handle = app_handle.as_ref().unwrap();
            let _ = app_handle.set_activation_policy(tauri::ActivationPolicy::Prohibited);
        }
    }
}

pub fn run() {
    // 单例检测
    let app_exists: bool = tauri::async_runtime::block_on(async move {
        if server::check_singleton().await.is_err() {
            println!("app exists");
            true
        } else {
            false
        }
    });
    if app_exists {
        return;
    }

    #[cfg(target_os = "linux")]
    std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");

    #[cfg(debug_assertions)]
    let devtools = tauri_plugin_devtools::init();

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            #[cfg(any(target_os = "linux", all(debug_assertions, windows)))]
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                log_err!(app.deep_link().register_all());
            }

            app.deep_link().on_open_url(|event| {
                tauri::async_runtime::spawn(async move {
                    if let Some(url) = event.urls().first() {
                        log_err!(resolve_scheme(url.to_string()).await);
                    }
                });
            });

            tauri::async_runtime::block_on(async move {
                resolve::resolve_setup(app).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // common
            cmd::get_sys_proxy,
            cmd::get_auto_proxy,
            cmd::open_app_dir,
            cmd::open_logs_dir,
            cmd::open_web_url,
            cmd::open_core_dir,
            cmd::get_portable_flag,
            cmd::get_network_interfaces,
            cmd::restart_core,
            cmd::restart_app,
            // 添加新的命令
            cmd::get_running_mode,
            cmd::install_service,
            // clash
            cmd::get_clash_info,
            cmd::patch_clash_config,
            cmd::patch_clash_mode,
            cmd::change_clash_core,
            cmd::get_runtime_config,
            cmd::get_runtime_yaml,
            cmd::get_runtime_exists,
            cmd::get_runtime_logs,
            cmd::invoke_uwp_tool,
            cmd::copy_clash_env,
            cmd::get_proxies,
            cmd::get_providers_proxies,
            // verge
            cmd::get_verge_config,
            cmd::patch_verge_config,
            cmd::test_delay,
            cmd::get_app_dir,
            cmd::copy_icon_file,
            cmd::download_icon_cache,
            cmd::open_devtools,
            cmd::exit_app,
            cmd::get_network_interfaces_info,
            // profile
            cmd::get_profiles,
            cmd::enhance_profiles,
            cmd::patch_profiles_config,
            cmd::view_profile,
            cmd::patch_profile,
            cmd::create_profile,
            cmd::import_profile,
            cmd::reorder_profile,
            cmd::update_profile,
            cmd::delete_profile,
            cmd::read_profile_file,
            cmd::save_profile_file,
            // script validation
            cmd::script_validate_notice,
            cmd::validate_script_file,
            // clash api
            cmd::clash_api_get_proxy_delay,
            // backup
            cmd::create_webdav_backup,
            cmd::save_webdav_config,
            cmd::list_webdav_backup,
            cmd::delete_webdav_backup,
            cmd::restore_webdav_backup,
            // export diagnostic info for issue reporting
            cmd::export_diagnostic_info,
        ]);

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app_handle, e| match e {
        tauri::RunEvent::Ready | tauri::RunEvent::Resumed => {
            AppHandleManager::global().init(app_handle.clone());
            #[cfg(target_os = "macos")]
            {
                let main_window = AppHandleManager::global().get_handle().get_webview_window("main").unwrap();
                let _ = main_window.set_title("Clash Verge");
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { has_visible_windows, .. } => {
            if !has_visible_windows {
                AppHandleManager::global().set_activation_policy_regular();
            }
            AppHandleManager::global().init(app_handle.clone());
        }
        tauri::RunEvent::ExitRequested { api, code, .. } => {
            if code.is_none() {
                api.prevent_exit();
            }
        }
        tauri::RunEvent::WindowEvent { label, event, .. } => {
            if label == "main" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        #[cfg(target_os = "macos")]
                        AppHandleManager::global().set_activation_policy_accessory();
                        if core::handle::Handle::global().is_exiting() {
                            return;
                        }
                        println!("closing window...");
                        api.prevent_close();
                        let window = core::handle::Handle::global().get_window().unwrap();
                        let _ = window.hide();
                    }
                    tauri::WindowEvent::Focused(true) => {
                        #[cfg(target_os = "macos")]
                        {
                            log_err!(hotkey::Hotkey::global().register("CMD+Q", "quit"));
                        }

                        #[cfg(not(target_os = "macos"))]
                        {
                            log_err!(hotkey::Hotkey::global().register("Control+Q", "quit"));
                        };
                        {   
                            let is_enable_global_hotkey = Config::verge().latest().enable_global_hotkey.unwrap_or(true);
                            if !is_enable_global_hotkey {
                                log_err!(hotkey::Hotkey::global().init())
                            }
                        }
                    }
                    tauri::WindowEvent::Focused(false) => {
                        #[cfg(target_os = "macos")]
                        {
                            log_err!(hotkey::Hotkey::global().unregister("CMD+Q"));
                        }
                        #[cfg(not(target_os = "macos"))]
                        {
                            log_err!(hotkey::Hotkey::global().unregister("Control+Q"));
                        };
                        {   
                            let is_enable_global_hotkey = Config::verge().latest().enable_global_hotkey.unwrap_or(true);
                            if !is_enable_global_hotkey {
                                log_err!(hotkey::Hotkey::global().reset())
                            }
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        #[cfg(target_os = "macos")]
                        {
                            log_err!(hotkey::Hotkey::global().unregister("CMD+Q"));
                        }

                        #[cfg(not(target_os = "macos"))]
                        {
                            log_err!(hotkey::Hotkey::global().unregister("Control+Q"));
                        };
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    });
}
