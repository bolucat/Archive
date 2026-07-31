#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]

use std::ffi::OsStr;

use anyhow::Context;

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::*,
        Security::{Authorization::*, *},
        Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING},
    },
    core::*,
};

#[cfg(windows)]
pub struct NamedPipeACL;

#[cfg(windows)]
impl NamedPipeACL {
    /// 将字符串转换为 Windows 宽字符
    fn to_wide_string(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(Some(0)).collect()
    }

    /// 获取错误描述
    fn get_error_description(error_code: u32) -> &'static str {
        match error_code {
            2 => "文件不存在 (ERROR_FILE_NOT_FOUND)",
            5 => "访问被拒绝 (ERROR_ACCESS_DENIED)",
            231 => "管道忙碌 (ERROR_PIPE_BUSY)",
            _ => "未知错误",
        }
    }

    /// 方法1: 使用 GetNamedSecurityInfo 直接获取 SDDL
    pub fn get_sddl_direct(pipe_name: &str) -> anyhow::Result<String> {
        let pipe_path = format!("\\\\.\\pipe\\{pipe_name}");

        println!("尝试直接获取管道 '{pipe_name}' 的 SDDL...");
        println!("管道路径: {pipe_path}");

        let pipe_path_wide = Self::to_wide_string(&pipe_path);

        let security_info =
            OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION;

        unsafe {
            let handle = CreateFileW(
                PCWSTR::from_raw(pipe_path_wide.as_ptr()),
                GENERIC_READ.0, // 只需要读权限来获取安全信息
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            );

            let handle = match handle {
                Ok(h) if h != INVALID_HANDLE_VALUE => {
                    println!("✓ 成功打开管道句柄");
                    h
                }
                Ok(_) => {
                    return Err(anyhow::anyhow!("failed to create file: empty handle"));
                }
                Err(e) => {
                    return Err(e).context("failed to create file");
                }
            };

            let mut security_descriptor_ptr = PSECURITY_DESCRIPTOR::default();
            println!("正在获取安全描述符...");

            let security_info =
                DACL_SECURITY_INFORMATION | OWNER_SECURITY_INFORMATION | GROUP_SECURITY_INFORMATION;

            // 调用 GetNamedSecurityInfo
            let result = GetSecurityInfo(
                handle,
                SE_KERNEL_OBJECT, // 命名管道被视为文件对象
                security_info,
                None,                               // psidOwner
                None,                               // psidGroup
                None,                               // ppDacl
                None,                               // ppSacl
                Some(&mut security_descriptor_ptr), // ppSecurityDescriptor
            );

            if result.is_err() {
                let e = windows::core::Error::from_hresult(result.to_hresult());
                return Err(anyhow::anyhow!(
                    "GetSecurityInfo 失败: {:#x} - {} ",
                    result.0,
                    e,
                ));
            }

            if security_descriptor_ptr.is_invalid() {
                return Err(anyhow::anyhow!("获取的安全描述符无效"));
            }

            println!("✓ 成功获取安全描述符");

            // 转换为 SDDL
            let mut sddl_string = PSTR::null();
            ConvertSecurityDescriptorToStringSecurityDescriptorA(
                security_descriptor_ptr,
                SDDL_REVISION_1,
                security_info,
                &mut sddl_string,
                None,
            )
            .context("failed to convert security descriptor to string")?;

            {
                // 释放安全描述符内存
                HLOCAL(security_descriptor_ptr.0).free();
            }

            // 转换为 Rust 字符串
            let sddl = sddl_string.to_string()?;

            println!("✓ 成功获取 SDDL: {sddl}");
            Ok(sddl)
        }
    }

    pub fn check_pipe_exists(pipe_name: &str) -> bool {
        use windows::Win32::Storage::FileSystem::{CreateFileW, OPEN_EXISTING};

        let pipe_path = format!("\\\\.\\pipe\\{pipe_name}");
        let pipe_path_wide = Self::to_wide_string(&pipe_path);

        unsafe {
            let handle = CreateFileW(
                PCWSTR::from_raw(pipe_path_wide.as_ptr()),
                0, // 不需要任何访问权限，只是检查存在性
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            );

            match handle {
                Ok(h) if h != INVALID_HANDLE_VALUE => {
                    let _ = CloseHandle(h);
                    true
                }
                _ => false,
            }
        }
    }

    /// 检查常见的系统管道
    pub fn scan_common_pipes() -> Result<Vec<String>> {
        let common_pipes = vec![
            "lsass",
            "winreg",
            "wkssvc",
            "trkwks",
            "srvsvc",
            "samr",
            "lsarpc",
            "netlogon",
            "spoolss",
            "atsvc",
            "browser",
            "keysvc",
            "protected_storage",
            "scerpc",
        ];

        let mut accessible_pipes = Vec::new();

        println!("\n=== 扫描常见系统管道 ===");

        for pipe_name in common_pipes {
            print!("检查管道 '{pipe_name}' ... ");

            // 尝试方法1
            match Self::get_sddl_direct(pipe_name) {
                Ok(sddl) => {
                    println!("✓ 可访问");
                    println!("  SDDL: {sddl}");
                    accessible_pipes.push(pipe_name.to_string());
                }
                Err(e) => {
                    eprintln!("✗ 不可访问: {e}");
                }
            }
        }

        Ok(accessible_pipes)
    }
}

/// 检查是否以管理员身份运行
#[cfg(windows)]
fn is_running_as_admin() -> bool {
    use windows::Win32::{
        Foundation::TRUE,
        Security::{GetTokenInformation, TokenElevation},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };

    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(
            GetCurrentProcess(),
            windows::Win32::Security::TOKEN_QUERY,
            &mut token,
        )
        .is_err()
        {
            return false;
        }

        let mut elevation = windows::Win32::Security::TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length = 0u32;

        if GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut std::ffi::c_void),
            std::mem::size_of::<windows::Win32::Security::TOKEN_ELEVATION>() as u32,
            &mut return_length,
        )
        .is_ok()
        {
            let _ = CloseHandle(token);
            return elevation.TokenIsElevated == TRUE.0 as u32;
        }

        let _ = CloseHandle(token);
        false
    }
}

#[cfg(windows)]
fn main() -> Result<()> {
    println!("=== Rust 命名管道 ACL 获取工具 ===");
    println!("使用 windows crate");

    // 检查管理员权限
    let is_admin = is_running_as_admin();
    println!("管理员权限: {}", if is_admin { "✓" } else { "✗" });

    if !is_admin {
        println!("⚠️  建议以管理员身份运行以获得更好的结果");
    }

    println!();

    // 测试特定管道
    let test_pipes = vec!["nyanpasu_ipc"];

    for pipe_name in test_pipes {
        println!("--- 测试管道: {pipe_name} ---");
        if !NamedPipeACL::check_pipe_exists(pipe_name) {
            println!("✗ 管道不存在");
            continue;
        }

        // 方法1: 直接获取
        println!("方法1 (GetNamedSecurityInfo):");
        match NamedPipeACL::get_sddl_direct(pipe_name) {
            Ok(sddl) => println!("✓ SDDL: {sddl}"),
            Err(e) => println!("✗ 失败: {e}"),
        }

        println!();
    }

    // // 扫描所有常见管道
    // match NamedPipeACL::scan_common_pipes() {
    //     Ok(accessible) => {
    //         println!("可访问的管道: {:?}", accessible);
    //     }
    //     Err(e) => {
    //         println!("扫描失败: {}", e);
    //     }
    // }

    Ok(())
}

#[cfg(not(windows))]
fn main() {
    panic!("not supported on non-windows platform");
}
