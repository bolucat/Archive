pub mod user;

pub fn register_ctrlc_handler() -> tokio::sync::mpsc::Receiver<()> {
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    ctrlc::set_handler(move || {
        eprintln!("Ctrl-C received, stopping service...");
        let _ = tx.try_send(());
    })
    .expect("Error setting Ctrl-C handler");
    rx
}
