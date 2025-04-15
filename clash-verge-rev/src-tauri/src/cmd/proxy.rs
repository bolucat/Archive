use mihomo_api::model::MihomoClient;

use super::CmdResult;
use crate::module::mihomo::MihomoManager;

#[tauri::command]
pub async fn get_proxies() -> CmdResult<serde_json::Value> {
    let manager = MihomoManager::global();
    manager.refresh_proxies().await.map_err(|e| e.to_string())?;
    let data = manager.get_data_proxies().await;
    Ok(data)
}

#[tauri::command]
pub async fn get_providers_proxies() -> CmdResult<serde_json::Value> {
    let manager = MihomoManager::global();
    manager
        .refresh_providers_proxies()
        .await
        .map_err(|e| e.to_string())?;
    let data = manager.get_data_providers_proxies().await;
    Ok(data)
}
