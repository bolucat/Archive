//! Internal helpers for parsing and forwarding core output.

use std::{io::BufReader, sync::Arc};

use encoding_rs::Encoding;
use os_pipe::PipeReader;
use parking_lot::RwLock;
use tokio::sync::mpsc::Sender;
use tracing_attributes::instrument;

use crate::runtime;

use super::CommandEvent;

#[instrument]
pub fn parse_check_output(log: String) -> String {
    let t = log.find("time=");
    let m = log.find("msg=");
    let mr = log.rfind('"');

    if let (Some(_), Some(m), Some(mr)) = (t, m, mr) {
        let e = match log.find("level=error msg=") {
            Some(e) => e + 17,
            None => m + 5,
        };

        if mr > m {
            return log[e..mr].to_owned();
        }
    }

    let l = log.find("error=");
    let r = log.find("path=").or(Some(log.len()));

    if let (Some(l), Some(r)) = (l, r) {
        return log[(l + 6)..(r - 1)].to_owned();
    }

    log
}

/// Ref: tauri v1.7.1
pub(super) fn spawn_pipe_reader<F: Fn(String) -> CommandEvent + Send + Copy + 'static>(
    tx: Sender<CommandEvent>,
    guard: Arc<RwLock<()>>,
    pipe_reader: PipeReader,
    wrapper: F,
    character_encoding: Option<&'static Encoding>,
) {
    std::thread::spawn(move || {
        let _lock = guard.read();
        let mut reader = BufReader::new(pipe_reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match crate::io::read_line(&mut reader, &mut buf) {
                Ok(n) => {
                    if n == 0 {
                        break;
                    }
                    let tx_ = tx.clone();
                    let line = match character_encoding {
                        Some(encoding) => Ok(encoding.decode_with_bom_removal(&buf).0.into()),
                        None => String::from_utf8(buf.clone()),
                    };
                    runtime::spawn(async move {
                        let _ = match line {
                            Ok(line) => tx_.send(wrapper(line)).await,
                            Err(e) => tx_.send(CommandEvent::Error(e.to_string())).await,
                        };
                    });
                }
                Err(e) => {
                    let tx_ = tx.clone();
                    runtime::spawn(
                        async move { tx_.send(CommandEvent::Error(e.to_string())).await },
                    );
                    break;
                }
            }
        }
    });
}
