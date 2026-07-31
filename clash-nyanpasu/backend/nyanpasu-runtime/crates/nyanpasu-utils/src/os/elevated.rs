#[cfg(windows)]
mod windows {
    #![allow(clippy::needless_doctest_main)]

    //! Checks if the current Windows process is elevated.
    //! Returns true if the process is elevated, false if not.
    //! ## Example
    //! ```rust
    //! use nyanpasu_utils::os::is_elevated;
    //! fn main() {
    //!     if is_elevated().expect("Failed to get elevation status.") {
    //!         println!("Running as administrator.");
    //!     } else {
    //!         println!("Not running as administrator.");
    //!     }
    //! }
    //! ```
    //!
    //! made with ♥  by h4rl
    //! uses bsd-2-clause license

    use windows::Win32::{
        Foundation::HANDLE,
        Security::{
            GetTokenInformation, TOKEN_ACCESS_MASK, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    pub fn is_elevated() -> windows::core::Result<bool> {
        unsafe {
            let mut h_token: HANDLE = HANDLE(0 as _);
            let result = OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_ACCESS_MASK(TOKEN_QUERY.0),
                &mut h_token,
            );
            match result {
                Ok(_) => {
                    let mut token_elevation: TOKEN_ELEVATION = core::mem::zeroed();
                    let mut return_length = 0;

                    match GetTokenInformation(
                        h_token,
                        TokenElevation,
                        Some(&mut token_elevation as *mut _ as *mut _),
                        core::mem::size_of::<TOKEN_ELEVATION>() as u32,
                        &mut return_length,
                    ) {
                        Ok(_) => {
                            if token_elevation.TokenIsElevated != 0 {
                                Ok(true)
                            } else {
                                Ok(false)
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
                Err(e) => Err(e),
            }
        }
    }
}

#[cfg(unix)]
mod unix {
    use nix::unistd::Uid;
    #[cfg(not(target_os = "macos"))]
    use nix::unistd::{Group, getgroups};

    pub fn is_elevated() -> bool {
        #[cfg(not(target_os = "macos"))]
        const ROOT_GROUPS: [&str; 2] = ["root", "admin"];
        let uid = Uid::current();
        #[cfg(not(target_os = "macos"))]
        let groups = getgroups();
        let is_root = uid.is_root();
        #[cfg(not(target_os = "macos"))]
        let is_root = is_root
            || groups.is_ok_and(|g| {
                g.iter().any(|g| {
                    Group::from_gid(*g)
                        .is_ok_and(|g| g.is_some_and(|g| ROOT_GROUPS.contains(&g.name.as_str())))
                })
            });
        is_root
    }
}

#[cfg(unix)]
pub use unix::*;
#[cfg(windows)]
pub use windows::*;
