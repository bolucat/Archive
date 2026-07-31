use constcat::concat;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
// Not CARGO_PKG_NAME: this crate is `nyanpasu-service-runtime`, but the label,
// the service data/config dirs and the installed binary name are all derived
// from the bin's name and must not change.
pub const APP_NAME: &str = "nyanpasu-service";
pub const SERVICE_LABEL: &str = concat!("moe.elaina.", APP_NAME);

// Build info
pub const COMMIT_HASH: &str = env!("COMMIT_HASH");
pub const COMMIT_AUTHOR: &str = env!("COMMIT_AUTHOR");
pub const COMMIT_DATE: &str = env!("COMMIT_DATE");
pub const BUILD_DATE: &str = env!("BUILD_DATE");
pub const BUILD_PROFILE: &str = env!("BUILD_PROFILE");
pub const BUILD_PLATFORM: &str = env!("BUILD_PLATFORM");
pub const RUSTC_VERSION: &str = env!("RUSTC_VERSION");
pub const LLVM_VERSION: &str = env!("LLVM_VERSION");

pub enum ExitCode {
    Normal = 0,
    PermissionDenied = 64,
    ServiceNotInstalled = 100,
    ServiceAlreadyInstalled = 101,
    ServiceAlreadyStopped = 102,
    ServiceAlreadyRunning = 103,
    Other = 1,
}

impl std::process::Termination for ExitCode {
    fn report(self) -> std::process::ExitCode {
        std::process::ExitCode::from(self as u8)
    }
}
