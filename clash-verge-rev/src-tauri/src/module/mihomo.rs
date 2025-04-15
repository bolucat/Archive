use crate::{config::Config, utils::dirs::app_socket_path};
use mihomo_api;
use once_cell::sync::{Lazy, OnceCell};
use std::sync::Mutex;
use tauri::http::{HeaderMap, HeaderValue};
#[cfg(target_os = "macos")]
use tokio_tungstenite::tungstenite::http;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rate {
    pub up: u64,
    pub down: u64,
}

pub struct MihomoManager {
    mihomo: Mutex<OnceCell<mihomo_api::MihomoManager>>,
}

impl MihomoManager {
    fn __global() -> &'static MihomoManager {
        static INSTANCE: Lazy<MihomoManager> = Lazy::new(|| MihomoManager {
            mihomo: Mutex::new(OnceCell::new()),
        });
        &INSTANCE
    }

    pub fn global() -> &'static mihomo_api::MihomoManager {
        let instance = MihomoManager::__global();

        let mihomo = &instance.mihomo;
        let lock = mihomo.lock().unwrap();

        if lock.get().is_none() {
            let socket_path = MihomoManager::get_socket_path();
            lock.set(mihomo_api::MihomoManager::new(socket_path)).ok();
        }

        unsafe { std::mem::transmute(lock.get().unwrap()) }
    }
}

impl MihomoManager {
    pub fn get_clash_client_info() -> Option<(String, HeaderMap)> {
        let client = { Config::clash().data().get_client_info() };
        let server = format!("http://{}", client.server);
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse().unwrap());
        if let Some(secret) = client.secret {
            let secret = format!("Bearer {}", secret).parse().unwrap();
            headers.insert("Authorization", secret);
        }

        Some((server, headers))
    }
    #[cfg(target_os = "macos")]
    pub fn get_traffic_ws_url() -> (String, HeaderValue) {
        let (url, headers) = MihomoManager::get_clash_client_info().unwrap();
        let ws_url = url.replace("http://", "ws://") + "/traffic";
        let auth = headers
            .get("Authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        let token = http::header::HeaderValue::from_str(&auth).unwrap();
        (ws_url, token)
    }

    fn get_socket_path() -> String {
        #[cfg(unix)]
        let socket_path = app_socket_path().unwrap();
        #[cfg(windows)]
        let socket_path = r"\\.\pipe\mihomo";
        socket_path
    }
}
