#![cfg(target_os = "android")]

use crate::{error::Error, tun2proxy::TunToProxy, tun_to_proxy, NetworkInterface, Options, Proxy};
use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jint},
    JNIEnv,
};
use std::io::Error as IoError;
use std::os::fd::RawFd;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::fence;
use std::sync::atomic::Ordering;

static mut TUN_INIT: AtomicBool = AtomicBool::new(false);
static mut TUN_TO_PROXY: Option<TunToProxy> = None;

/// # Safety
///
/// Initialize tun2proxy
#[no_mangle]
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyInit(
    mut env: JNIEnv,
    _clazz: JClass,
    proxy_url: JString,
    tun_fd: jint,
    tun_mtu: jint,
    log_level_int: jint,
    dns_over_tcp: jboolean,
) -> jint {
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
    let filter = android_logger::FilterBuilder::new().parse(filter_str).build();
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("tun2proxy")
            .with_max_level(log::LevelFilter::Trace)
            .with_filter(filter),
    );

    let mut block = || -> Result<(), Error> {
        let proxy_url = get_java_string(&mut env, &proxy_url)?;
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
        TUN_TO_PROXY = Some(tun2proxy);
        fence(Ordering::Release);
        TUN_INIT.store(true, Ordering::Relaxed);
        Ok::<(), Error>(())
    };
    if let Err(error) = block() {
        log::error!("failed to run tun2proxy with error: {:?}", error);
        return 1;
    }
    0
}

/// # Safety
///
/// Running tun2proxy
#[no_mangle]
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyRun(
    mut _env: JNIEnv,
    _clazz: JClass
) -> jint {
    let block = || -> Result<(), Error> {
        while !TUN_INIT.load(Ordering::Relaxed) {
            fence(Ordering::Acquire);
            std::thread::yield_now();
        }
        if let Some(tun2proxy) = &mut TUN_TO_PROXY {
            tun2proxy.run()?;
        }
        TUN_TO_PROXY = None;
        fence(Ordering::Release);
        TUN_INIT.store(false, Ordering::Relaxed);
        Ok::<(), Error>(())
    };
    if let Err(error) = block() {
        log::error!("failed to run tun2proxy with error: {:?}", error);
        return 1;
    }
    0
}

/// # Safety
///
/// Shutdown tun2proxy
#[no_mangle]
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyDestroy(_env: JNIEnv, _: JClass) -> jint {
    if !TUN_INIT.load(Ordering::Relaxed) {
        fence(Ordering::Acquire);
        log::error!("tun2proxy already stopped");
        return 0;
    }
    match &mut TUN_TO_PROXY {
        None => {
            log::error!("tun2proxy not started");
            1
        }
        Some(tun2proxy) => {
            if let Err(e) = tun2proxy.shutdown() {
                log::error!("failed to shutdown tun2proxy with error: {:?}", e);
                1
            } else {
                0
            }
        }
    }
}

unsafe fn get_java_string<'a>(env: &'a mut JNIEnv, string: &'a JString) -> Result<&'a str, Error> {
    let str_ptr = env.get_string(string)?.as_ptr();
    let s: &str = std::ffi::CStr::from_ptr(str_ptr).to_str()?;
    Ok(s)
}

fn dup_fd(fd: RawFd) -> Result<RawFd, Error> {
    let dup_fd = unsafe { libc::dup(fd) };
    if dup_fd < 0 {
      return Err(Error::Io(IoError::last_os_error()));
    }
    Ok(dup_fd)
}
