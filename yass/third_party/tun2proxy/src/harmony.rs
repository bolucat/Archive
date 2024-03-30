#![cfg(all(target_os = "linux", target_env = "ohos"))]

use crate::{error::Error, tun2proxy::TunToProxy, tun_to_proxy, NetworkInterface, Options, Proxy};
use std::ffi::CStr;
use std::io::Error as IoError;
use std::os::fd::RawFd;

#[allow(non_camel_case_types)]
type c_int = std::os::raw::c_int;
#[allow(non_camel_case_types)]
type c_char = std::os::raw::c_char;

/// # Safety
///
/// Initialize tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_init(
    proxy_url: *const c_char,
    tun_fd: c_int,
    tun_mtu: c_int,
    log_level_int: c_int,
    dns_over_tcp: c_int,
) -> *mut c_char {
    let log_level = match log_level_int {
        0 => "off",
        1 => "error",
        2 => "warn",
        3 => "info",
        4 => "debug",
        5 => "trace", // verbose
        _ => "warn",  // default
    };
    let filter_str = &format!("off,tun2proxy={log_level}");
    let filter = ohos_hilog::FilterBuilder::new().parse(filter_str).build();
    ohos_hilog::init_once(
        ohos_hilog::Config::default()
            .with_tag("tun2proxy")
            .with_max_level(log::LevelFilter::Trace)
            .with_filter(filter),
    );

    let block = || -> Result<TunToProxy, Error> {
        let proxy_url = unsafe { CStr::from_ptr(proxy_url) }.to_str()?;
        let proxy = Proxy::from_url(proxy_url)?;

        let addr = proxy.addr;
        let proxy_type = proxy.proxy_type;
        log::info!("Proxy {proxy_type} server: {addr}");

        let dns_addr = "8.8.8.8".parse::<std::net::IpAddr>().unwrap();
        let options = Options::new().with_dns_addr(Some(dns_addr)).with_mtu(tun_mtu as usize);
        let options = if dns_over_tcp != 0 { options.with_dns_over_tcp() } else { options };

        let dup_tun_fd = dup_fd(tun_fd)?;
        let interface = NetworkInterface::Fd(dup_tun_fd);
        let tun2proxy = tun_to_proxy(&interface, &proxy, options)?;
        Ok::<TunToProxy, Error>(tun2proxy)
    };
    match block() {
        Ok(tun2proxy) => {
            let b = Box::new(tun2proxy);
            Box::into_raw(b) as *mut c_char
        }
        Err(error) => {
            log::error!("failed to run tun2proxy with error: {:?}", error);
            0 as *mut c_char
        }
    }
}

/// # Safety
///
/// Running tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_run(tun2proxy_ptr: *mut c_char) -> c_int {
    if tun2proxy_ptr == 0 as *mut c_char {
        return 1;
    }
    let ptr = tun2proxy_ptr as *mut TunToProxy;
    let mut tun2proxy = unsafe { Box::from_raw(ptr) };
    let mut block = || -> Result<(), Error> {
        tun2proxy.run()?;
        Ok::<(), Error>(())
    };
    match block() {
        Ok(()) => {
            std::mem::forget(tun2proxy);
            0
        }
        Err(error) => {
            std::mem::forget(tun2proxy);
            log::error!("failed to run tun2proxy with error: {:?}", error);
            1
        }
    }
}

/// # Safety
///
/// Shutdown tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_shutdown(tun2proxy_ptr: *mut c_char) -> c_int {
    if tun2proxy_ptr == 0 as *mut c_char {
        return 1;
    }
    let ptr = tun2proxy_ptr as *mut TunToProxy;
    let mut tun2proxy = unsafe { Box::from_raw(ptr) };
    if let Err(e) = tun2proxy.shutdown() {
        std::mem::forget(tun2proxy);
        log::error!("failed to shutdown tun2proxy with error: {:?}", e);
        1
    } else {
        std::mem::forget(tun2proxy);
        0
    }
}

/// # Safety
///
/// Destroy tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_destroy(tun2proxy_ptr: *mut c_char) -> () {
    if tun2proxy_ptr == 0 as *mut c_char {
        return;
    }
    let ptr = tun2proxy_ptr as *mut TunToProxy;
    let _tun2proxy = unsafe { Box::from_raw(ptr) };
}

fn dup_fd(fd: RawFd) -> Result<RawFd, Error> {
    let dup_fd = unsafe { libc::dup(fd) };
    if dup_fd < 0 {
        return Err(Error::Io(IoError::last_os_error()));
    }
    Ok(dup_fd)
}
