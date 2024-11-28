#![cfg(target_os = "ios")]

use smoltcp::phy::Medium;
use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use std::cell::RefCell;
use std::rc::Rc;

use smoltcp::phy::{self, Device, DeviceCapabilities};
use smoltcp::time::Instant;

use crate::IosContext;

#[allow(non_camel_case_types)]
type c_int = libc::c_int;
#[allow(non_camel_case_types)]
type c_size_t = libc::size_t;
#[allow(non_camel_case_types)]
type c_void = libc::c_void;

#[derive(Debug)]
pub struct ContextInterfaceDesc {
    context: *mut c_void,
    read_fd: c_int,
    get_read_packet_context_data_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_void,
    get_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_size_t,
    free_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void),
    write_packets_fn: unsafe extern "C" fn(*mut c_void, *const *mut c_void, *const c_size_t, c_int),
}

impl AsRawFd for ContextInterfaceDesc {
    fn as_raw_fd(&self) -> RawFd {
        self.read_fd
    }
}

impl ContextInterfaceDesc {
    pub fn from_context(
        context: *mut c_void,
        read_fd: c_int,
        get_read_packet_context_data_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_void,
        get_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_size_t,
        free_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void),
        write_packets_fn: unsafe extern "C" fn(*mut c_void, *const *mut c_void, *const c_size_t, c_int),
    ) -> io::Result<ContextInterfaceDesc> {
        Ok(ContextInterfaceDesc {
            context: context,
            read_fd: read_fd,
            get_read_packet_context_data_fn: get_read_packet_context_data_fn,
            get_read_packet_context_size_fn: get_read_packet_context_size_fn,
            free_read_packet_context_size_fn: free_read_packet_context_size_fn,
            write_packets_fn: write_packets_fn,
        })
    }

    pub fn recv(&mut self) -> io::Result<(*mut c_void, *mut c_void)> {
        unsafe {
            let mut context_buffer = vec![0; 8];
            let len = libc::read(self.read_fd, context_buffer.as_mut_ptr() as *mut c_void, context_buffer.len());
            if len == -1 {
                return Err(io::Error::last_os_error());
            }
            let read_ctx: *mut c_void = *(context_buffer.as_ptr() as *mut *mut c_void);
            Ok((self.context, read_ctx))
        }
    }

    pub fn send(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let packet: *mut c_void = buffer.as_ptr() as *mut c_void;
            let packet_length: c_size_t = buffer.len();
            (self.write_packets_fn)(self.context, &packet, &packet_length, 1);
            Ok(buffer.len() as usize)
        }
    }
}

impl Drop for ContextInterfaceDesc {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.read_fd);
        }
    }
}

/// A virtual TUN (IP) or TAP (Ethernet) interface.
#[derive(Debug)]
pub struct ContextInterface {
    lower: Rc<RefCell<ContextInterfaceDesc>>,
    mtu: usize,
    medium: Medium,
}

impl AsRawFd for ContextInterface {
    fn as_raw_fd(&self) -> RawFd {
        self.lower.borrow().as_raw_fd()
    }
}

impl ContextInterface {
    /// Attaches to a TUN/TAP interface specified by file descriptor `fd`.
    ///
    /// On platforms like Android, a file descriptor to a tun interface is exposed.
    /// On these platforms, a ContextInterface cannot be instantiated with a name.
    pub fn from_context(context: &IosContext, medium: Medium, mtu: usize) -> io::Result<ContextInterface> {
        let lower = ContextInterfaceDesc::from_context(
            context.context,
            context.read_fd,
            context.get_read_packet_context_data_fn,
            context.get_read_packet_context_size_fn,
            context.free_read_packet_context_size_fn,
            context.write_packets_fn,
        )?;
        Ok(ContextInterface {
            lower: Rc::new(RefCell::new(lower)),
            mtu,
            medium,
        })
    }
}

impl Device for ContextInterface {
    type RxToken<'a> = RxToken;
    type TxToken<'a> = TxToken;

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.max_transmission_unit = self.mtu;
        caps.medium = self.medium;
        caps
    }

    fn receive(&mut self, _timestamp: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        let mut lower = self.lower.borrow_mut();
        let get_read_packet_context_data_fn = lower.get_read_packet_context_data_fn;
        let get_read_packet_context_size_fn = lower.get_read_packet_context_size_fn;
        let free_read_packet_context_size_fn = lower.free_read_packet_context_size_fn;
        match lower.recv() {
            Ok((context, read_ctx)) => {
                let rx = RxToken {
                    context,
                    read_ctx,
                    get_read_packet_context_data_fn,
                    get_read_packet_context_size_fn,
                    free_read_packet_context_size_fn,
                };
                let tx = TxToken { lower: self.lower.clone() };
                Some((rx, tx))
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => None,
            Err(err) => panic!("{}", err),
        }
    }

    fn transmit(&mut self, _timestamp: Instant) -> Option<Self::TxToken<'_>> {
        Some(TxToken { lower: self.lower.clone() })
    }
}

#[doc(hidden)]
pub struct RxToken {
    context: *mut c_void,
    read_ctx: *mut c_void,
    get_read_packet_context_data_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const c_void,
    get_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_size_t,
    free_read_packet_context_size_fn: unsafe extern "C" fn(*mut c_void, *mut c_void),
}

impl phy::RxToken for RxToken {
    fn consume<R, F>(self, f: F) -> R
    where
        F: FnOnce(&[u8]) -> R,
    {
        let data: *const c_void = unsafe { (self.get_read_packet_context_data_fn)(self.context, self.read_ctx) };
        let len: c_size_t = unsafe { (self.get_read_packet_context_size_fn)(self.context, self.read_ctx) };
        let buffer = unsafe { std::slice::from_raw_parts(data as *const u8, len as usize) };
        f(buffer)
    }
}

impl Drop for RxToken {
    fn drop(&mut self) {
        unsafe {
            (self.free_read_packet_context_size_fn)(self.context, self.read_ctx);
        }
    }
}

#[doc(hidden)]
pub struct TxToken {
    lower: Rc<RefCell<ContextInterfaceDesc>>,
}

impl phy::TxToken for TxToken {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where
        F: FnOnce(&mut [u8]) -> R,
    {
        let mut lower = self.lower.borrow_mut();
        let mut buffer = vec![0; len];
        let result = f(&mut buffer);
        match lower.send(&mut buffer[..]) {
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                // net_debug!("phy: tx failed due to WouldBlock")
            }
            Err(err) => panic!("{}", err),
        }
        result
    }
}
