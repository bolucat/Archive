//! P0 probe (design doc §6/§9 O3): print the actual containment mechanism and
//! prove kill-on-drop on this host. Run on every deployment target class.
//!
//! cargo run -p nyanpasu-utils --features process --example containment_probe

use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mechanism = processkit::ProcessGroup::new()?.mechanism();
    println!("containment mechanism = {mechanism:?}");

    #[cfg(windows)]
    assert_eq!(
        mechanism,
        processkit::Mechanism::JobObject,
        "Windows containment must use a Job Object"
    );

    #[cfg(windows)]
    let cmd = processkit::Command::new("cmd")
        .args(["/C", "ping -n 3600 127.0.0.1 >NUL"])
        .create_no_window();
    #[cfg(unix)]
    let cmd = processkit::Command::new("sleep").args(["3600"]);

    let run = cmd.start().await?;
    let pid = run.pid().expect("a real child must have a pid");
    println!("child pid = {pid}");

    drop(run);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let alive = {
        use sysinfo::{Pid, ProcessRefreshKind, RefreshKind, System};

        let kind = RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing());
        let mut system = System::new_with_specifics(kind);
        system.refresh_specifics(kind);
        system.process(Pid::from_u32(pid)).is_some()
    };
    println!("child alive after drop = {alive} (MUST be false)");
    assert!(!alive, "kill-on-drop failed on this host");
    println!("PROBE OK");
    Ok(())
}
