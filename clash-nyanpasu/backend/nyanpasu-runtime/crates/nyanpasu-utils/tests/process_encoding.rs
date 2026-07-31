#![cfg(feature = "process")]

use nyanpasu_utils::process::{Command, ProcessEvent};

fn child() -> &'static str {
    env!("CARGO_BIN_EXE_nyanpasu-test-child")
}

#[tokio::test]
async fn gbk_stdout_decodes_correctly() {
    let (_handle, mut rx) = Command::new(child())
        .args(["gbk-stdout"])
        .encoding(Some(encoding_rs::GBK))
        .spawn()
        .await
        .unwrap();
    let mut decoded = None;
    while let Some(e) = rx.recv().await {
        if let ProcessEvent::Stdout(l) = e {
            decoded = Some(l);
            break;
        }
    }
    assert_eq!(decoded.unwrap().trim(), "中文");
}
