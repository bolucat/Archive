use crate::{model::E, sock};
use hyper::Method;
use serde_json::Value;
use tokio_util::codec::{Framed, LinesCodec};
use std::{sync::Arc, time::Duration};
use tokio::{
    time::timeout,
    sync::Mutex,
};
use futures::{SinkExt, StreamExt};
use tokio::net::windows::named_pipe::ClientOptions;

pub struct WindowsClient {
    lock: Arc<Mutex<()>>,
}

impl WindowsClient {
    pub fn new() -> Self {
        Self {
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub async fn send_request(
        &self,
        socket_path: String,
        path: &str,
        method: Method,
        body: Option<Value>,
    ) -> Result<Value, E> {
        // Acquire lock before opening pipe
        // let _guard = self.lock.lock().await;

        // Attempt to open the pipe with retry logic
        let mut retries = 0;
        let pipe = loop {
            match ClientOptions::new().open(socket_path.clone()) {
                Ok(pipe) => break pipe,
                Err(e) if e.raw_os_error() == Some(231) && retries < 5 => {
                    retries += 1;
                    let delay = Duration::from_millis(200 * retries);
                    tokio::time::sleep(delay).await;
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        };

        // Use a scope to ensure the pipe is dropped when done
        let result = async {
            let mut framed = Framed::new(pipe, LinesCodec::new());
        
            // Build request
            let mut request = format!(
                "{} {} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n",
                method.as_str(),
                path
            );
        
            if let Some(ref json_body) = body {
                let body_str = json_body.to_string();
                request += &format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str);
            } else {
                request += "\r\n";
            }
        
            framed.send(request).await?;
        
            // Parse headers
            let mut headers_done = false;
            let mut is_chunked = false;
        
            while let Ok(Some(Ok(line))) = timeout(Duration::from_secs(5), framed.next()).await {
                if line.is_empty() {
                    headers_done = true;
                    break;
                }
                
                if line.starts_with("HTTP/1.1 4") || line.starts_with("HTTP/1.1 5") {
                    return Err(format!("Server error: {}", line).into());
                }
                
                if line.eq_ignore_ascii_case("Transfer-Encoding: chunked") {
                    is_chunked = true;
                }
            }
        
            if !headers_done {
                return Err("Malformed response: no headers end".into());
            }
        
            let mut response_body = String::new();
        
            if is_chunked {
                // Handle chunked encoding
                loop {
                    // Read chunk size line
                    let chunk_size_line = match timeout(Duration::from_secs(5), framed.next()).await {
                        Ok(Some(Ok(line))) => line,
                        _ => break,
                    };
        
                    let chunk_size = match usize::from_str_radix(chunk_size_line.trim(), 16) {
                        Ok(0) => break, // End of chunks
                        Ok(_) => (), // We don't actually need the size with LinesCodec
                        Err(_) => return Err("Invalid chunk size".into()),
                    };
        
                    // Read chunk data line
                    if let Ok(Some(Ok(data_line))) = timeout(Duration::from_secs(5), framed.next()).await {
                        response_body.push_str(&data_line);
                    }
        
                    // Skip trailing CRLF (empty line)
                    let _ = framed.next().await;
                }
            } else {
                // Handle normal content
                while let Ok(Some(Ok(line))) = timeout(Duration::from_secs(5), framed.next()).await {
                    response_body.push_str(&line);
                }
            }
        
            serde_json::from_str(&response_body).map_err(|e| e.into())
        }.await;
    
        result
    }
}