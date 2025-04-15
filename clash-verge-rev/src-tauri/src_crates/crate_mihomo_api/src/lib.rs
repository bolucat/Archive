// impl MihomoManager {
//     pub async fn patch_configs(&self, config: serde_json::Value) -> Result<(), String> {
//         let url = format!("{}/configs", self.mihomo_server);
//         let response = self.send_request(Method::PATCH, url, Some(config)).await?;
//         if response["code"] == 204 {
//             Ok(())
//         } else {
//             Err(response["message"]
//                 .as_str()
//                 .unwrap_or("unknown error")
//                 .to_string())
//         }
//     }

//     pub async fn test_proxy_delay(
//         &self,
//         name: &str,
//         test_url: Option<String>,
//         timeout: i32,
//     ) -> Result<serde_json::Value, String> {
//         let test_url = test_url.unwrap_or("http://cp.cloudflare.com/generate_204".to_string());
//         let url = format!(
//             "{}/proxies/{}/delay?url={}&timeout={}",
//             self.mihomo_server, name, test_url, timeout
//         );
//         let response = self.send_request(Method::GET, url, None).await?;
//         Ok(response)
//     }

//     pub async fn delete_connection(&self, id: &str) -> Result<(), String> {
//         let url = format!("{}/connections/{}", self.mihomo_server, id);
//         let response = self.send_request(Method::DELETE, url, None).await?;
//         if response["code"] == 204 {
//             Ok(())
//         } else {
//             Err(response["message"]
//                 .as_str()
//                 .unwrap_or("unknown error")
//                 .to_string())
//         }
//     }
// }

pub mod model;
pub use model::{E, MihomoData, MihomoManager};
pub mod platform;
pub mod sock;
pub use platform::Client;
