#[cfg(unix)] pub mod unix;
#[cfg(unix)] pub use unix::UnixClient as Client;
#[cfg(windows)] pub mod windows;
#[cfg(windows)] pub use windows::WindowsClient as Client;