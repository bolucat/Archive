#![cfg(target_os = "android")]

use crate::{error::Error, tun2proxy::TunToProxy, tun_to_proxy, NetworkInterface, Options, Proxy};
use jni::{
    objects::{JClass, JString},
    sys::{jboolean, jint, jlong},
    JNIEnv,
};
use std::io::Error as IoError;
use std::os::fd::RawFd;

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
) -> jlong {
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

    let mut block = || -> Result<TunToProxy, Error> {
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
        Ok::<TunToProxy, Error>(tun2proxy)
    };
    match block() {
        Ok(tun2proxy) => {
            let b = Box::new(tun2proxy);
            Box::into_raw(b) as jlong
        }
        Err(error) => {
            log::error!("failed to run tun2proxy with error: {:?}", error);
            0 as jlong
        }
    }
}

/// # Safety
///
/// Running tun2proxy
#[no_mangle]
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyRun(mut _env: JNIEnv, _clazz: JClass, tun2proxy_ptr: jlong) -> jint {
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
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyShutdown(_env: JNIEnv, _: JClass, tun2proxy_ptr: jlong) -> jint {
    if tun2proxy_ptr == 0 {
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
pub unsafe extern "C" fn Java_it_gui_yass_MainActivity_tun2ProxyDestroy(_env: JNIEnv, _: JClass, tun2proxy_ptr: jlong) -> () {
    if tun2proxy_ptr == 0 {
        return;
    }
    let ptr = tun2proxy_ptr as *mut TunToProxy;
    let _tun2proxy = unsafe { Box::from_raw(ptr) };
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
