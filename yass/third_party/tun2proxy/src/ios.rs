#![cfg(target_os = "ios")]

use crate::{error::Error, tun2proxy::TunToProxy, tun_to_proxy, IosContext, NetworkInterface, Options, Proxy};
use std::ffi::CStr;
#[allow(non_camel_case_types)]
type c_char = libc::c_char;
#[allow(non_camel_case_types)]
type c_int = libc::c_int;
#[allow(non_camel_case_types)]
type c_size_t = libc::size_t;
#[allow(non_camel_case_types)]
type c_void = libc::c_void;

/// # Safety
///
/// Initialize tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_init(
    context: *mut c_void,
    read_fd: c_int,
    get_read_packet_context_data_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_void,
    get_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_size_t,
    free_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void),
    write_packets_fn: unsafe extern "C" fn(*mut c_void, *const *mut c_void, *const c_size_t, c_int),
    proxy_url: *const c_char,
    tun_mtu: c_int,
    _log_level_int: c_int,
    dns_over_tcp: c_int,
) -> *mut c_void {
    let block = || -> Result<TunToProxy, Error> {
        let proxy_url = unsafe { CStr::from_ptr(proxy_url) }.to_str()?;
        let proxy = Proxy::from_url(proxy_url)?;

        let addr = proxy.addr;
        let proxy_type = proxy.proxy_type;
        log::info!("Proxy {proxy_type} server: {addr}");

        let dns_addr = "8.8.8.8".parse::<std::net::IpAddr>().unwrap();
        let options = Options::new().with_dns_addr(Some(dns_addr)).with_mtu(tun_mtu as usize);
        let options = if dns_over_tcp != 0 { options.with_dns_over_tcp() } else { options };

        let context = IosContext {
            context: context,
            read_fd: read_fd,
            get_read_packet_context_data_fn: get_read_packet_context_data_fn,
            get_read_packet_context_size_fn: get_read_packet_context_size_fn,
            free_read_packet_context_size_fn: free_read_packet_context_size_fn,
            write_packets_fn: write_packets_fn,
        };
        let interface = NetworkInterface::Context(context);
        let tun2proxy = tun_to_proxy(&interface, &proxy, options)?;
        Ok::<TunToProxy, Error>(tun2proxy)
    };
    match block() {
        Ok(tun2proxy) => {
            let b = Box::new(tun2proxy);
            Box::into_raw(b) as *mut c_void
        }
        Err(error) => {
            log::error!("failed to run tun2proxy with error: {:?}", error);
            0 as *mut c_void
        }
    }
}

/// # Safety
///
/// Run tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_run(tun2proxy_ptr: *mut c_void) -> c_int {
    if tun2proxy_ptr == 0 as *mut c_void {
        return -1;
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
pub unsafe extern "C" fn tun2proxy_shutdown(tun2proxy_ptr: *mut c_void) -> c_int {
    if tun2proxy_ptr == 0 as *mut c_void {
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
pub unsafe extern "C" fn tun2proxy_destroy(tun2proxy_ptr: *mut c_void) -> () {
    if tun2proxy_ptr == 0 as *mut c_void {
        return;
    }
    let ptr = tun2proxy_ptr as *mut TunToProxy;
    let _tun2proxy = unsafe { Box::from_raw(ptr) };
}
