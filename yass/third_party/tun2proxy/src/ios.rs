#![cfg(target_os = "ios")]

use crate::{error::Error, tun2proxy::TunToProxy, tun_to_proxy, IosContext, NetworkInterface, Options, Proxy};
use std::ffi::CStr;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::fence;
use std::sync::atomic::Ordering;

static mut TUN_INIT: AtomicBool = AtomicBool::new(false);
static mut TUN_TO_PROXY: Option<TunToProxy> = None;

/// # Safety
///
/// Initialize tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_init(
    context: *mut libc::c_void,
    read_fd: libc::c_int,
    get_read_packet_context_data_fn: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> *const libc::c_void,
    get_read_packet_context_size_fn: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void) -> libc::size_t,
    free_read_packet_context_size_fn: unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void),
    write_packets_fn: unsafe extern "C" fn(*mut libc::c_void, *const *mut libc::c_void, *const libc::size_t, libc::c_int),
    proxy_url: *const libc::c_char,
    tun_mtu: libc::c_int,
    _log_level_int: libc::c_int,
    dns_over_tcp: libc::c_int,
) -> libc::c_int {

    let block = || -> Result<(), Error> {
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
        TUN_TO_PROXY = Some(tun2proxy);
        fence(Ordering::Release);
        TUN_INIT.store(true, Ordering::Relaxed);
        Ok::<(), Error>(())
    };
    if let Err(error) = block() {
        log::error!("failed to init tun2proxy with error: {:?}", error);
        return -1;
    }
    0
}

/// # Safety
///
/// Run tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_run() -> libc::c_int {
    let block = || -> Result<(), Error> {
        while !TUN_INIT.load(Ordering::Relaxed) {
            fence(Ordering::Acquire);
            std::thread::yield_now();
        }
        if let Some(tun2proxy) = &mut TUN_TO_PROXY {
            tun2proxy.run()?;
        }
        TUN_TO_PROXY = None;
        Ok::<(), Error>(())
    };
    if let Err(error) = block() {
        log::error!("failed to run tun2proxy with error: {:?}", error);
        return -1;
    }
    0
}

/// # Safety
///
/// Shutdown tun2proxy
#[no_mangle]
pub unsafe extern "C" fn tun2proxy_destroy() -> libc::c_int {
    if !TUN_INIT.load(Ordering::Relaxed) {
        fence(Ordering::Acquire);
        log::error!("tun2proxy already stopped");
        return 0;
    }
    match &mut TUN_TO_PROXY {
        None => {
            log::error!("tun2proxy not started");
            -1
        }
        Some(tun2proxy) => {
            if let Err(e) = tun2proxy.shutdown() {
                log::error!("failed to shutdown tun2proxy with error: {:?}", e);
                -1
            } else {
                0
            }
        }
    }
}
