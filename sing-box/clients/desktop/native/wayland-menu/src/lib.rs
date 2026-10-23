use std::ffi::c_void;
use std::io::ErrorKind;
use std::os::fd::{AsFd, OwnedFd};
use std::ptr;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use napi::bindgen_prelude::Buffer;
use napi::threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode};
use napi::{Error, Result, Status};
use napi_derive::napi;
use rustix::event::{EventfdFlags, PollFd, PollFlags, Timespec, eventfd, poll};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::io::Errno;
use rustix::mm::{MapFlags, ProtFlags, mmap, munmap};
use wayland_client::backend::WaylandError;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::{self, WlBuffer};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::protocol::wl_touch::{self, WlTouch};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_cursor::CursorTheme;
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape, WpCursorShapeDeviceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_manager_v1::WpCursorShapeManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::{
    Layer, ZwlrLayerShellV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

const LOCATE_TIMEOUT: Duration = Duration::from_millis(500);
const PANEL_BUFFER_COUNT: usize = 3;
const DEFAULT_CURSOR_SIZE: u32 = 24;
const BUTTON_LEFT: u32 = 0x110;
const DEFAULT_CURSOR: Cursor = Cursor {
    shape: Shape::Default,
    names: &["default", "left_ptr"],
};

type EventCallback = ThreadsafeFunction<MenuEvent, (), MenuEvent, Status, false, true>;

#[napi(object)]
#[derive(Default)]
pub struct MenuEvent {
    pub kind: String,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub usable_width: Option<u32>,
    pub usable_height: Option<u32>,
    pub scale: Option<f64>,
    pub button: Option<u32>,
    pub pressed: Option<bool>,
    pub key: Option<u32>,
    pub delta_x: Option<f64>,
    pub delta_y: Option<f64>,
    pub message: Option<String>,
}

enum Command {
    Locate,
    Show(PanelPlacement),
    Present(Frame),
    SetCursor(Cursor),
    Hide,
    Stop,
}

#[derive(Clone, Copy)]
struct Cursor {
    shape: Shape,
    names: &'static [&'static str],
}

struct PanelPlacement {
    anchor: Anchor,
    margin_top: i32,
    margin_right: i32,
    margin_bottom: i32,
    margin_left: i32,
    width: u32,
    height: u32,
}

struct Frame {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

#[napi]
pub struct LayerShellMenu {
    commands: Sender<Command>,
    wake: Arc<OwnedFd>,
    thread: Option<JoinHandle<()>>,
}

#[napi]
impl LayerShellMenu {
    #[napi(constructor)]
    pub fn new(callback: EventCallback) -> Result<Self> {
        let connection = Connection::connect_to_env()
            .map_err(|error| Error::from_reason(format!("connect to Wayland: {error}")))?;
        let (globals, queue) = registry_queue_init::<State>(&connection)
            .map_err(|error| Error::from_reason(format!("initialize Wayland registry: {error}")))?;
        let handle = queue.handle();
        let compositor = globals
            .bind::<WlCompositor, _, _>(&handle, 4..=6, ())
            .map_err(|error| Error::from_reason(format!("bind wl_compositor: {error}")))?;
        let shm = globals
            .bind::<WlShm, _, _>(&handle, 1..=1, ())
            .map_err(|error| Error::from_reason(format!("bind wl_shm: {error}")))?;
        let layer_shell = globals
            .bind::<ZwlrLayerShellV1, _, _>(&handle, 1..=4, ())
            .map_err(|error| Error::from_reason(format!("bind zwlr_layer_shell_v1: {error}")))?;
        let viewporter = globals.bind::<WpViewporter, _, _>(&handle, 1..=1, ()).ok();
        let fractional_scale_manager = globals
            .bind::<WpFractionalScaleManagerV1, _, _>(&handle, 1..=1, ())
            .ok();
        let cursor_shape_manager = globals
            .bind::<WpCursorShapeManagerV1, _, _>(&handle, 1..=1, ())
            .ok();
        let scale_expected = fractional_scale_manager.is_some() || compositor.version() >= 6;
        let mut state = State {
            callback,
            connection,
            compositor,
            shm,
            layer_shell,
            viewporter,
            fractional_scale_manager,
            cursor_shape_manager,
            scale_expected,
            outputs: Vec::new(),
            seats: Vec::new(),
            shields: Vec::new(),
            locate_deadline: None,
            located: None,
            panel: None,
            frame: None,
            cursor: DEFAULT_CURSOR,
            cursor_theme: None,
            cursor_surface: None,
            next_id: 1,
        };
        globals.contents().with_list(|list| {
            for global in list {
                state.add_global(
                    globals.registry(),
                    global.name,
                    &global.interface,
                    global.version,
                    &handle,
                );
            }
        });
        let wake = Arc::new(
            eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)
                .map_err(|error| Error::from_reason(format!("create eventfd: {error}")))?,
        );
        let (commands, receiver) = mpsc::channel();
        let thread_wake = wake.clone();
        let thread = thread::Builder::new()
            .name("wayland-menu".into())
            .spawn(move || run(queue, state, receiver, thread_wake))
            .map_err(|error| Error::from_reason(format!("start Wayland thread: {error}")))?;
        Ok(Self {
            commands,
            wake,
            thread: Some(thread),
        })
    }

    #[napi]
    pub fn locate(&self) {
        self.send(Command::Locate);
    }

    #[napi]
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &self,
        anchor: u32,
        margin_top: i32,
        margin_right: i32,
        margin_bottom: i32,
        margin_left: i32,
        width: u32,
        height: u32,
    ) {
        self.send(Command::Show(PanelPlacement {
            anchor: Anchor::from_bits_truncate(anchor),
            margin_top,
            margin_right,
            margin_bottom,
            margin_left,
            width,
            height,
        }));
    }

    #[napi]
    pub fn present(&self, data: Buffer, width: u32, height: u32) {
        self.send(Command::Present(Frame {
            data: data.to_vec(),
            width,
            height,
        }));
    }

    #[napi]
    pub fn set_cursor(&self, name: String) {
        let (shape, names): (Shape, &'static [&'static str]) = match name.as_str() {
            "hand" => (Shape::Pointer, &["pointer", "hand2"]),
            "text" => (Shape::Text, &["text", "xterm"]),
            "not-allowed" => (Shape::NotAllowed, &["not-allowed", "crossed_circle"]),
            "wait" => (Shape::Wait, &["wait", "watch"]),
            "progress" => (Shape::Progress, &["progress", "left_ptr_watch"]),
            "help" => (Shape::Help, &["help", "question_arrow"]),
            "crosshair" => (Shape::Crosshair, &["crosshair"]),
            "move" => (Shape::Move, &["move", "fleur"]),
            "grab" => (Shape::Grab, &["grab", "openhand"]),
            "grabbing" => (Shape::Grabbing, &["grabbing", "closedhand"]),
            _ => (DEFAULT_CURSOR.shape, DEFAULT_CURSOR.names),
        };
        self.send(Command::SetCursor(Cursor { shape, names }));
    }

    #[napi]
    pub fn hide(&self) {
        self.send(Command::Hide);
    }

    #[napi]
    pub fn destroy(&mut self) {
        self.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_ok() {
            let _ = rustix::io::write(&*self.wake, &1u64.to_ne_bytes());
        }
    }
}

impl Drop for LayerShellMenu {
    fn drop(&mut self) {
        self.destroy();
    }
}

fn run(
    mut queue: EventQueue<State>,
    mut state: State,
    commands: Receiver<Command>,
    wake: Arc<OwnedFd>,
) {
    let result = (|| -> std::result::Result<(), String> {
        let handle = queue.handle();
        loop {
            let mut presented = false;
            loop {
                match commands.try_recv() {
                    Ok(Command::Stop) | Err(TryRecvError::Disconnected) => {
                        state.hide();
                        let _ = queue.flush();
                        return Ok(());
                    }
                    Ok(command) => {
                        presented |= matches!(command, Command::Present(_));
                        state.handle(command, &handle);
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            if presented {
                state.draw(&handle);
            }
            queue
                .dispatch_pending(&mut state)
                .map_err(|error| format!("dispatch Wayland events: {error}"))?;
            if state
                .locate_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                state.finish_locate(true);
            }
            match queue.flush() {
                Ok(()) => {}
                Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                Err(error) => return Err(format!("flush Wayland requests: {error}")),
            }
            let Some(guard) = queue.prepare_read() else {
                continue;
            };
            let timeout = state.locate_deadline.map(|deadline| {
                let remaining = deadline.saturating_duration_since(Instant::now());
                Timespec {
                    tv_sec: remaining.as_secs() as _,
                    tv_nsec: remaining.subsec_nanos() as _,
                }
            });
            let (wayland_ready, wake_ready) = {
                let connection_fd = guard.connection_fd();
                let mut fds = [
                    PollFd::new(&connection_fd, PollFlags::IN),
                    PollFd::new(&*wake, PollFlags::IN),
                ];
                match poll(&mut fds, timeout.as_ref()) {
                    Ok(_) => {}
                    Err(Errno::INTR) => continue,
                    Err(error) => return Err(format!("poll Wayland connection: {error}")),
                }
                (!fds[0].revents().is_empty(), !fds[1].revents().is_empty())
            };
            if wayland_ready {
                match guard.read() {
                    Ok(_) => {}
                    Err(WaylandError::Io(error)) if error.kind() == ErrorKind::WouldBlock => {}
                    Err(error) => return Err(format!("read Wayland events: {error}")),
                }
            } else {
                drop(guard);
            }
            if wake_ready {
                let mut counter = [0u8; 8];
                let _ = rustix::io::read(&*wake, &mut counter);
            }
        }
    })();
    if let Err(message) = result {
        state.emit(MenuEvent {
            kind: "error".into(),
            message: Some(message),
            ..Default::default()
        });
    }
}

struct State {
    callback: EventCallback,
    connection: Connection,
    compositor: WlCompositor,
    shm: WlShm,
    layer_shell: ZwlrLayerShellV1,
    viewporter: Option<WpViewporter>,
    fractional_scale_manager: Option<WpFractionalScaleManagerV1>,
    cursor_shape_manager: Option<WpCursorShapeManagerV1>,
    scale_expected: bool,
    outputs: Vec<(u32, WlOutput)>,
    seats: Vec<Seat>,
    shields: Vec<Shield>,
    locate_deadline: Option<Instant>,
    located: Option<u64>,
    panel: Option<Panel>,
    frame: Option<Frame>,
    cursor: Cursor,
    cursor_theme: Option<(u32, CursorTheme)>,
    cursor_surface: Option<WlSurface>,
    next_id: u64,
}

struct Seat {
    name: u32,
    seat: WlSeat,
    pointer: Option<Pointer>,
    keyboard: Option<Keyboard>,
    touch: Option<Touch>,
}

struct Pointer {
    pointer: WlPointer,
    shape_device: Option<WpCursorShapeDeviceV1>,
    focus: Option<WlSurface>,
    enter_serial: u32,
    x: f64,
    y: f64,
}

struct Keyboard {
    keyboard: WlKeyboard,
    focus: Option<WlSurface>,
}

struct Touch {
    touch: WlTouch,
    active: Option<TouchPoint>,
}

struct TouchPoint {
    id: i32,
    surface: WlSurface,
    x: f64,
    y: f64,
}

struct Shield {
    id: u64,
    output: Option<WlOutput>,
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    viewport: Option<WpViewport>,
    fractional_scale: Option<WpFractionalScaleV1>,
    buffer: Option<ShmBuffer>,
    probe: Option<(WlSurface, ZwlrLayerSurfaceV1)>,
    size: Option<(u32, u32)>,
    usable_size: Option<(u32, u32)>,
    scale: Option<f64>,
    pointer: Option<(f64, f64)>,
}

struct Panel {
    surface: WlSurface,
    layer_surface: ZwlrLayerSurfaceV1,
    viewport: Option<WpViewport>,
    width: u32,
    height: u32,
    configured: bool,
    buffers: Vec<ShmBuffer>,
}

struct ShmBuffer {
    id: u64,
    buffer: WlBuffer,
    pool: WlShmPool,
    memory: *mut c_void,
    length: usize,
    width: u32,
    height: u32,
    busy: bool,
    _fd: OwnedFd,
}

unsafe impl Send for ShmBuffer {}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        self.buffer.destroy();
        self.pool.destroy();
        unsafe {
            let _ = munmap(self.memory, self.length);
        }
    }
}

#[derive(Clone, Copy)]
enum LayerRole {
    Shield(u64),
    Probe(u64),
    Panel,
}

impl State {
    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn emit(&self, event: MenuEvent) {
        self.callback
            .call(event, ThreadsafeFunctionCallMode::NonBlocking);
    }

    fn add_global(
        &mut self,
        registry: &WlRegistry,
        name: u32,
        interface: &str,
        version: u32,
        handle: &QueueHandle<Self>,
    ) {
        if interface == WlOutput::interface().name {
            let output = registry.bind::<WlOutput, _, _>(name, version.min(4), handle, ());
            self.outputs.push((name, output));
        } else if interface == WlSeat::interface().name {
            let seat = registry.bind::<WlSeat, _, _>(name, version.min(8), handle, name);
            self.seats.push(Seat {
                name,
                seat,
                pointer: None,
                keyboard: None,
                touch: None,
            });
        }
    }

    fn remove_global(&mut self, name: u32) {
        if let Some(index) = self
            .outputs
            .iter()
            .position(|(output_name, _)| *output_name == name)
        {
            let (_, output) = self.outputs.remove(index);
            if output.version() >= 3 {
                output.release();
            }
        }
        if let Some(index) = self.seats.iter().position(|seat| seat.name == name) {
            let seat = self.seats.remove(index);
            release_seat_devices(seat.pointer, seat.keyboard, seat.touch);
            if seat.seat.version() >= 5 {
                seat.seat.release();
            }
        }
    }

    fn handle(&mut self, command: Command, handle: &QueueHandle<Self>) {
        match command {
            Command::Locate => self.locate(handle),
            Command::Show(placement) => self.show(placement, handle),
            Command::Present(frame) => self.frame = Some(frame),
            Command::SetCursor(cursor) => {
                self.cursor = cursor;
                for seat_index in 0..self.seats.len() {
                    let over_panel = self.seats[seat_index]
                        .pointer
                        .as_ref()
                        .and_then(|pointer| pointer.focus.as_ref())
                        .is_some_and(|focus| self.is_panel(focus));
                    if over_panel {
                        self.apply_cursor(seat_index, cursor, handle);
                    }
                }
            }
            Command::Hide => self.hide(),
            Command::Stop => {}
        }
    }

    fn locate(&mut self, handle: &QueueHandle<Self>) {
        self.hide();
        let outputs: Vec<Option<WlOutput>> = if self.outputs.is_empty() {
            vec![None]
        } else {
            self.outputs
                .iter()
                .map(|(_, output)| Some(output.clone()))
                .collect()
        };
        for output in outputs {
            let id = self.allocate_id();
            let surface = self.compositor.create_surface(handle, id);
            let layer_surface = self.layer_shell.get_layer_surface(
                &surface,
                output.as_ref(),
                Layer::Overlay,
                "sing-box-tray-shield".into(),
                handle,
                LayerRole::Shield(id),
            );
            layer_surface.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
            layer_surface.set_exclusive_zone(-1);
            layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
            let viewport = self
                .viewporter
                .as_ref()
                .map(|viewporter| viewporter.get_viewport(&surface, handle, ()));
            let fractional_scale = self
                .fractional_scale_manager
                .as_ref()
                .map(|manager| manager.get_fractional_scale(&surface, handle, id));
            surface.commit();
            let probe_surface = self.compositor.create_surface(handle, 0);
            let probe_layer_surface = self.layer_shell.get_layer_surface(
                &probe_surface,
                output.as_ref(),
                Layer::Background,
                "sing-box-tray-probe".into(),
                handle,
                LayerRole::Probe(id),
            );
            probe_layer_surface
                .set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
            probe_layer_surface.set_exclusive_zone(0);
            probe_layer_surface.set_keyboard_interactivity(KeyboardInteractivity::None);
            probe_surface.commit();
            self.shields.push(Shield {
                id,
                output,
                surface,
                layer_surface,
                viewport,
                fractional_scale,
                buffer: None,
                probe: Some((probe_surface, probe_layer_surface)),
                size: None,
                usable_size: None,
                scale: None,
                pointer: None,
            });
        }
        self.locate_deadline = Some(Instant::now() + LOCATE_TIMEOUT);
    }

    fn finish_locate(&mut self, timed_out: bool) {
        if self.locate_deadline.is_none() {
            return;
        }
        let ready = |shield: &Shield| {
            shield.size.is_some()
                && shield.usable_size.is_some()
                && (shield.scale.is_some() || !self.scale_expected || timed_out)
        };
        let candidate = self
            .shields
            .iter()
            .find(|shield| shield.pointer.is_some() && ready(shield))
            .or_else(|| {
                if timed_out {
                    self.shields.iter().find(|shield| ready(shield))
                } else {
                    None
                }
            });
        let Some(shield) = candidate else {
            if timed_out {
                self.dismiss();
            }
            return;
        };
        let (width, height) = shield.size.unwrap();
        let (usable_width, usable_height) = shield.usable_size.unwrap();
        let event = MenuEvent {
            kind: "located".into(),
            x: shield.pointer.map(|(x, _)| x),
            y: shield.pointer.map(|(_, y)| y),
            width: Some(width),
            height: Some(height),
            usable_width: Some(usable_width),
            usable_height: Some(usable_height),
            scale: Some(shield.scale.unwrap_or(1.0)),
            ..Default::default()
        };
        self.located = Some(shield.id);
        self.locate_deadline = None;
        self.emit(event);
    }

    fn show(&mut self, placement: PanelPlacement, handle: &QueueHandle<Self>) {
        let Some(located) = self.located else {
            return;
        };
        let Some(output) = self
            .shields
            .iter()
            .find(|shield| shield.id == located)
            .map(|shield| shield.output.clone())
        else {
            return;
        };
        if let Some(panel) = self.panel.take() {
            destroy_panel(panel);
        }
        let surface = self.compositor.create_surface(handle, 0);
        let layer_surface = self.layer_shell.get_layer_surface(
            &surface,
            output.as_ref(),
            Layer::Overlay,
            "sing-box-tray-menu".into(),
            handle,
            LayerRole::Panel,
        );
        layer_surface.set_size(placement.width, placement.height);
        layer_surface.set_anchor(placement.anchor);
        layer_surface.set_margin(
            placement.margin_top,
            placement.margin_right,
            placement.margin_bottom,
            placement.margin_left,
        );
        layer_surface.set_exclusive_zone(0);
        layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
        let viewport = self
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(&surface, handle, ()));
        surface.commit();
        self.panel = Some(Panel {
            surface,
            layer_surface,
            viewport,
            width: placement.width,
            height: placement.height,
            configured: false,
            buffers: Vec::new(),
        });
    }

    fn draw(&mut self, handle: &QueueHandle<Self>) {
        let Some(panel) = self.panel.as_ref() else {
            return;
        };
        if !panel.configured {
            return;
        }
        let Some(frame) = self.frame.take() else {
            return;
        };
        let length = frame.width as usize * frame.height as usize * 4;
        if frame.width == 0 || frame.height == 0 || frame.data.len() != length {
            return;
        }
        let reusable = panel.buffers.iter().position(|buffer| {
            !buffer.busy && buffer.width == frame.width && buffer.height == frame.height
        });
        let index = match reusable {
            Some(index) => index,
            None => {
                let in_use = panel.buffers.iter().filter(|buffer| buffer.busy).count();
                if in_use >= PANEL_BUFFER_COUNT {
                    self.frame = Some(frame);
                    return;
                }
                let id = self.allocate_id();
                let buffer = match create_buffer(&self.shm, frame.width, frame.height, id, handle) {
                    Ok(buffer) => buffer,
                    Err(message) => {
                        self.emit(MenuEvent {
                            kind: "error".into(),
                            message: Some(message),
                            ..Default::default()
                        });
                        return;
                    }
                };
                let panel = self.panel.as_mut().unwrap();
                panel.buffers.retain(|buffer| buffer.busy);
                panel.buffers.push(buffer);
                panel.buffers.len() - 1
            }
        };
        let panel = self.panel.as_mut().unwrap();
        let buffer = &mut panel.buffers[index];
        unsafe {
            ptr::copy_nonoverlapping(frame.data.as_ptr(), buffer.memory.cast::<u8>(), length);
        }
        buffer.busy = true;
        panel.surface.attach(Some(&buffer.buffer), 0, 0);
        panel
            .surface
            .damage_buffer(0, 0, frame.width as i32, frame.height as i32);
        match &panel.viewport {
            Some(viewport) => {
                panel.surface.set_buffer_scale(1);
                viewport.set_destination(panel.width as i32, panel.height as i32);
            }
            None => {
                let scale = (frame.width / panel.width.max(1)).max(1);
                if frame.width % scale == 0 && frame.height % scale == 0 {
                    panel.surface.set_buffer_scale(scale as i32);
                }
            }
        }
        panel.surface.commit();
    }

    fn dismiss(&mut self) {
        self.hide();
        self.emit(MenuEvent {
            kind: "dismiss".into(),
            ..Default::default()
        });
    }

    fn update_shield_scale(&mut self, id: u64, scale: f64) {
        let Some(index) = self.shields.iter().position(|shield| shield.id == id) else {
            return;
        };
        if self.located == Some(id)
            && self.shields[index]
                .scale
                .is_some_and(|current| current != scale)
        {
            self.dismiss();
            return;
        }
        self.shields[index].scale = Some(scale);
        self.finish_locate(false);
    }

    fn hide(&mut self) {
        self.locate_deadline = None;
        self.located = None;
        self.frame = None;
        if let Some(panel) = self.panel.take() {
            destroy_panel(panel);
        }
        for shield in self.shields.drain(..) {
            destroy_shield(shield);
        }
        for seat in &mut self.seats {
            if let Some(pointer) = &mut seat.pointer {
                pointer.focus = None;
            }
            if let Some(keyboard) = &mut seat.keyboard {
                keyboard.focus = None;
            }
            if let Some(touch) = &mut seat.touch {
                touch.active = None;
            }
        }
    }

    fn configure_layer(
        &mut self,
        role: LayerRole,
        layer_surface: &ZwlrLayerSurfaceV1,
        serial: u32,
        width: u32,
        height: u32,
        handle: &QueueHandle<Self>,
    ) {
        match role {
            LayerRole::Shield(id) => {
                let Some(index) = self.shields.iter().position(|shield| shield.id == id) else {
                    return;
                };
                if self.located == Some(id)
                    && self.shields[index]
                        .size
                        .is_some_and(|size| size != (width, height))
                {
                    self.dismiss();
                    return;
                }
                layer_surface.ack_configure(serial);
                let has_viewport = self.shields[index].viewport.is_some();
                let (buffer_width, buffer_height) = if has_viewport {
                    (1, 1)
                } else {
                    (width, height)
                };
                let buffer_id = self.allocate_id();
                let shield = &mut self.shields[index];
                shield.size = Some((width, height));
                if shield.buffer.as_ref().is_none_or(|buffer| {
                    buffer.width != buffer_width || buffer.height != buffer_height
                }) {
                    match create_buffer(&self.shm, buffer_width, buffer_height, buffer_id, handle) {
                        Ok(buffer) => shield.buffer = Some(buffer),
                        Err(message) => {
                            self.emit(MenuEvent {
                                kind: "error".into(),
                                message: Some(message),
                                ..Default::default()
                            });
                            return;
                        }
                    }
                }
                let shield = &self.shields[index];
                if let Some(viewport) = &shield.viewport {
                    viewport.set_destination(width as i32, height as i32);
                }
                shield
                    .surface
                    .attach(shield.buffer.as_ref().map(|buffer| &buffer.buffer), 0, 0);
                shield
                    .surface
                    .damage_buffer(0, 0, buffer_width as i32, buffer_height as i32);
                shield.surface.commit();
                self.finish_locate(false);
            }
            LayerRole::Probe(id) => {
                let Some(shield) = self.shields.iter_mut().find(|shield| shield.id == id) else {
                    return;
                };
                shield.usable_size = Some((width, height));
                if let Some((surface, layer_surface)) = shield.probe.take() {
                    layer_surface.destroy();
                    surface.destroy();
                }
                self.finish_locate(false);
            }
            LayerRole::Panel => {
                let Some(panel) = self.panel.as_mut() else {
                    return;
                };
                if panel.layer_surface != *layer_surface {
                    return;
                }
                layer_surface.ack_configure(serial);
                if width > 0 && height > 0 {
                    panel.width = width;
                    panel.height = height;
                }
                panel.configured = true;
                self.draw(handle);
            }
        }
    }

    fn closed_layer(&mut self, role: LayerRole, layer_surface: &ZwlrLayerSurfaceV1) {
        let visible = match role {
            LayerRole::Panel => self
                .panel
                .as_ref()
                .is_some_and(|panel| panel.layer_surface == *layer_surface),
            LayerRole::Shield(id) | LayerRole::Probe(id) => {
                self.shields.iter().any(|shield| shield.id == id)
            }
        };
        if visible {
            self.dismiss();
        }
    }

    fn apply_cursor(&mut self, seat_index: usize, cursor: Cursor, handle: &QueueHandle<Self>) {
        let Some(pointer) = self.seats[seat_index].pointer.as_ref() else {
            return;
        };
        if let Some(device) = &pointer.shape_device {
            device.set_shape(pointer.enter_serial, cursor.shape);
            return;
        }
        let scale = self
            .shields
            .iter()
            .find_map(|shield| shield.scale)
            .unwrap_or(1.0)
            .ceil()
            .max(1.0) as u32;
        let size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_CURSOR_SIZE)
            * scale;
        if self
            .cursor_theme
            .as_ref()
            .is_none_or(|(theme_size, _)| *theme_size != size)
        {
            let name = std::env::var("XCURSOR_THEME").unwrap_or_else(|_| "default".into());
            let Ok(theme) =
                CursorTheme::load_from_name(&self.connection, self.shm.clone(), &name, size)
            else {
                return;
            };
            self.cursor_theme = Some((size, theme));
        }
        if self.cursor_surface.is_none() {
            self.cursor_surface = Some(self.compositor.create_surface(handle, 0));
        }
        let (_, theme) = self.cursor_theme.as_mut().unwrap();
        let Some(name) = cursor
            .names
            .iter()
            .copied()
            .find(|name| theme.get_cursor(name).is_some())
        else {
            return;
        };
        let image = &theme.get_cursor(name).unwrap()[0];
        let (width, height) = image.dimensions();
        let (hotspot_x, hotspot_y) = image.hotspot();
        let surface = self.cursor_surface.as_ref().unwrap();
        surface.set_buffer_scale(scale as i32);
        surface.attach(Some(&**image), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();
        let pointer = self.seats[seat_index].pointer.as_ref().unwrap();
        pointer.pointer.set_cursor(
            pointer.enter_serial,
            Some(surface),
            (hotspot_x / scale) as i32,
            (hotspot_y / scale) as i32,
        );
    }

    fn shield_index(&self, surface: &WlSurface) -> Option<usize> {
        self.shields
            .iter()
            .position(|shield| shield.surface == *surface)
    }

    fn is_panel(&self, surface: &WlSurface) -> bool {
        self.panel
            .as_ref()
            .is_some_and(|panel| panel.surface == *surface)
    }
}

fn release_seat_devices(
    pointer: Option<Pointer>,
    keyboard: Option<Keyboard>,
    touch: Option<Touch>,
) {
    if let Some(pointer) = pointer {
        if let Some(device) = pointer.shape_device {
            device.destroy();
        }
        if pointer.pointer.version() >= 3 {
            pointer.pointer.release();
        }
    }
    if let Some(keyboard) = keyboard
        && keyboard.keyboard.version() >= 3
    {
        keyboard.keyboard.release();
    }
    if let Some(touch) = touch
        && touch.touch.version() >= 3
    {
        touch.touch.release();
    }
}

fn destroy_panel(panel: Panel) {
    if let Some(viewport) = &panel.viewport {
        viewport.destroy();
    }
    panel.layer_surface.destroy();
    panel.surface.destroy();
}

fn destroy_shield(shield: Shield) {
    if let Some(viewport) = &shield.viewport {
        viewport.destroy();
    }
    if let Some(fractional_scale) = &shield.fractional_scale {
        fractional_scale.destroy();
    }
    shield.layer_surface.destroy();
    shield.surface.destroy();
    if let Some((surface, layer_surface)) = shield.probe {
        layer_surface.destroy();
        surface.destroy();
    }
}

fn create_buffer(
    shm: &WlShm,
    width: u32,
    height: u32,
    id: u64,
    handle: &QueueHandle<State>,
) -> std::result::Result<ShmBuffer, String> {
    let stride = width as usize * 4;
    let length = stride * height as usize;
    let fd = memfd_create("sing-box-tray-menu", MemfdFlags::CLOEXEC)
        .map_err(|error| format!("create shared memory: {error}"))?;
    ftruncate(&fd, length as u64).map_err(|error| format!("resize shared memory: {error}"))?;
    let memory = unsafe {
        mmap(
            ptr::null_mut(),
            length,
            ProtFlags::READ | ProtFlags::WRITE,
            MapFlags::SHARED,
            &fd,
            0,
        )
    }
    .map_err(|error| format!("map shared memory: {error}"))?;
    let pool = shm.create_pool(fd.as_fd(), length as i32, handle, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        wl_shm::Format::Argb8888,
        handle,
        id,
    );
    Ok(ShmBuffer {
        id,
        buffer,
        pool,
        memory,
        length,
        width,
        height,
        busy: false,
        _fd: fd,
    })
}

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => state.add_global(registry, name, &interface, version, handle),
            wl_registry::Event::GlobalRemove { name } => state.remove_global(name),
            _ => {}
        }
    }
}

impl Dispatch<WlSeat, u32> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &u32,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        else {
            return;
        };
        let cursor_shape_manager = state.cursor_shape_manager.clone();
        let Some(entry) = state.seats.iter_mut().find(|entry| entry.seat == *seat) else {
            return;
        };
        let has_pointer = capabilities.contains(wl_seat::Capability::Pointer);
        if has_pointer && entry.pointer.is_none() {
            let pointer = seat.get_pointer(handle, ());
            let shape_device = cursor_shape_manager
                .as_ref()
                .map(|manager| manager.get_pointer(&pointer, handle, ()));
            entry.pointer = Some(Pointer {
                pointer,
                shape_device,
                focus: None,
                enter_serial: 0,
                x: 0.0,
                y: 0.0,
            });
        } else if !has_pointer {
            release_seat_devices(entry.pointer.take(), None, None);
        }
        let has_keyboard = capabilities.contains(wl_seat::Capability::Keyboard);
        if has_keyboard && entry.keyboard.is_none() {
            entry.keyboard = Some(Keyboard {
                keyboard: seat.get_keyboard(handle, ()),
                focus: None,
            });
        } else if !has_keyboard {
            release_seat_devices(None, entry.keyboard.take(), None);
        }
        let has_touch = capabilities.contains(wl_seat::Capability::Touch);
        if has_touch && entry.touch.is_none() {
            entry.touch = Some(Touch {
                touch: seat.get_touch(handle, ()),
                active: None,
            });
        } else if !has_touch {
            release_seat_devices(None, None, entry.touch.take());
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        wl_pointer: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let Some(seat_index) = state.seats.iter().position(|seat| {
            seat.pointer
                .as_ref()
                .is_some_and(|pointer| pointer.pointer == *wl_pointer)
        }) else {
            return;
        };
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                let shield_index = state.shield_index(&surface);
                let is_panel = state.is_panel(&surface);
                let pointer = state.seats[seat_index].pointer.as_mut().unwrap();
                pointer.focus = Some(surface);
                pointer.enter_serial = serial;
                pointer.x = surface_x;
                pointer.y = surface_y;
                if let Some(index) = shield_index {
                    state.apply_cursor(seat_index, DEFAULT_CURSOR, handle);
                    state.shields[index].pointer = Some((surface_x, surface_y));
                    state.finish_locate(false);
                } else if is_panel {
                    state.apply_cursor(seat_index, state.cursor, handle);
                    state.emit(MenuEvent {
                        kind: "enter".into(),
                        x: Some(surface_x),
                        y: Some(surface_y),
                        ..Default::default()
                    });
                }
            }
            wl_pointer::Event::Leave { surface, .. } => {
                let pointer = state.seats[seat_index].pointer.as_mut().unwrap();
                pointer.focus = None;
                let (x, y) = (pointer.x, pointer.y);
                if let Some(index) = state.shield_index(&surface) {
                    state.shields[index].pointer = None;
                } else if state.is_panel(&surface) {
                    state.emit(MenuEvent {
                        kind: "leave".into(),
                        x: Some(x),
                        y: Some(y),
                        ..Default::default()
                    });
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let pointer = state.seats[seat_index].pointer.as_mut().unwrap();
                pointer.x = surface_x;
                pointer.y = surface_y;
                let Some(focus) = pointer.focus.clone() else {
                    return;
                };
                if let Some(index) = state.shield_index(&focus) {
                    state.shields[index].pointer = Some((surface_x, surface_y));
                    state.finish_locate(false);
                } else if state.is_panel(&focus) {
                    state.emit(MenuEvent {
                        kind: "motion".into(),
                        x: Some(surface_x),
                        y: Some(surface_y),
                        ..Default::default()
                    });
                }
            }
            wl_pointer::Event::Button {
                button,
                state: button_state,
                ..
            } => {
                let pointer = state.seats[seat_index].pointer.as_ref().unwrap();
                let Some(focus) = pointer.focus.clone() else {
                    return;
                };
                let (x, y) = (pointer.x, pointer.y);
                let pressed = button_state == WEnum::Value(wl_pointer::ButtonState::Pressed);
                if state.is_panel(&focus) {
                    state.emit(MenuEvent {
                        kind: "button".into(),
                        x: Some(x),
                        y: Some(y),
                        button: Some(button),
                        pressed: Some(pressed),
                        ..Default::default()
                    });
                } else if pressed && state.shield_index(&focus).is_some() {
                    state.dismiss();
                }
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                let pointer = state.seats[seat_index].pointer.as_ref().unwrap();
                let Some(focus) = pointer.focus.clone() else {
                    return;
                };
                if !state.is_panel(&focus) {
                    return;
                }
                let (x, y) = (pointer.x, pointer.y);
                let (delta_x, delta_y) = match axis {
                    WEnum::Value(wl_pointer::Axis::HorizontalScroll) => (value, 0.0),
                    WEnum::Value(wl_pointer::Axis::VerticalScroll) => (0.0, value),
                    _ => return,
                };
                state.emit(MenuEvent {
                    kind: "axis".into(),
                    x: Some(x),
                    y: Some(y),
                    delta_x: Some(delta_x),
                    delta_y: Some(delta_y),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        wl_keyboard: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(seat_index) = state.seats.iter().position(|seat| {
            seat.keyboard
                .as_ref()
                .is_some_and(|keyboard| keyboard.keyboard == *wl_keyboard)
        }) else {
            return;
        };
        match event {
            wl_keyboard::Event::Enter { surface, .. } => {
                state.seats[seat_index].keyboard.as_mut().unwrap().focus = Some(surface);
            }
            wl_keyboard::Event::Leave { surface, .. } => {
                state.seats[seat_index].keyboard.as_mut().unwrap().focus = None;
                if state.is_panel(&surface) {
                    state.dismiss();
                }
            }
            wl_keyboard::Event::Key {
                key,
                state: key_state,
                ..
            } => {
                let focus = state.seats[seat_index]
                    .keyboard
                    .as_ref()
                    .unwrap()
                    .focus
                    .clone();
                if !focus.is_some_and(|focus| state.is_panel(&focus)) {
                    return;
                }
                state.emit(MenuEvent {
                    kind: "key".into(),
                    key: Some(key),
                    pressed: Some(key_state == WEnum::Value(wl_keyboard::KeyState::Pressed)),
                    ..Default::default()
                });
            }
            _ => {}
        }
    }
}

impl Dispatch<WlTouch, ()> for State {
    fn event(
        state: &mut Self,
        wl_touch: &WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let Some(seat_index) = state.seats.iter().position(|seat| {
            seat.touch
                .as_ref()
                .is_some_and(|touch| touch.touch == *wl_touch)
        }) else {
            return;
        };
        match event {
            wl_touch::Event::Down {
                surface, id, x, y, ..
            } => {
                let is_shield = state.shield_index(&surface).is_some();
                let is_panel = state.is_panel(&surface);
                let touch = state.seats[seat_index].touch.as_mut().unwrap();
                if touch.active.is_some() {
                    return;
                }
                if is_shield {
                    state.dismiss();
                    return;
                }
                if !is_panel {
                    return;
                }
                touch.active = Some(TouchPoint { id, surface, x, y });
                state.emit(MenuEvent {
                    kind: "enter".into(),
                    x: Some(x),
                    y: Some(y),
                    ..Default::default()
                });
                state.emit(MenuEvent {
                    kind: "button".into(),
                    x: Some(x),
                    y: Some(y),
                    button: Some(BUTTON_LEFT),
                    pressed: Some(true),
                    ..Default::default()
                });
            }
            wl_touch::Event::Motion { id, x, y, .. } => {
                let touch = state.seats[seat_index].touch.as_mut().unwrap();
                let Some(point) = touch.active.as_mut().filter(|point| point.id == id) else {
                    return;
                };
                point.x = x;
                point.y = y;
                state.emit(MenuEvent {
                    kind: "motion".into(),
                    x: Some(x),
                    y: Some(y),
                    ..Default::default()
                });
            }
            wl_touch::Event::Up { id, .. } => {
                let touch = state.seats[seat_index].touch.as_mut().unwrap();
                if touch.active.as_ref().is_none_or(|point| point.id != id) {
                    return;
                }
                let point = touch.active.take().unwrap();
                if !state.is_panel(&point.surface) {
                    return;
                }
                state.emit(MenuEvent {
                    kind: "button".into(),
                    x: Some(point.x),
                    y: Some(point.y),
                    button: Some(BUTTON_LEFT),
                    pressed: Some(false),
                    ..Default::default()
                });
                state.emit(MenuEvent {
                    kind: "leave".into(),
                    x: Some(point.x),
                    y: Some(point.y),
                    ..Default::default()
                });
            }
            wl_touch::Event::Cancel => {
                let touch = state.seats[seat_index].touch.as_mut().unwrap();
                let Some(point) = touch.active.take() else {
                    return;
                };
                if state.is_panel(&point.surface) {
                    state.emit(MenuEvent {
                        kind: "leave".into(),
                        x: Some(point.x),
                        y: Some(point.y),
                        ..Default::default()
                    });
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlSurface, u64> for State {
    fn event(
        state: &mut Self,
        _: &WlSurface,
        event: wl_surface::Event,
        id: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wl_surface::Event::PreferredBufferScale { factor } = event else {
            return;
        };
        if state.fractional_scale_manager.is_some() {
            return;
        }
        state.update_shield_scale(*id, factor as f64);
    }
}

impl Dispatch<WpFractionalScaleV1, u64> for State {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        id: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let wp_fractional_scale_v1::Event::PreferredScale { scale } = event else {
            return;
        };
        state.update_shield_scale(*id, scale as f64 / 120.0);
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, LayerRole> for State {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        role: &LayerRole,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => state.configure_layer(*role, layer_surface, serial, width, height, handle),
            zwlr_layer_surface_v1::Event::Closed => state.closed_layer(*role, layer_surface),
            _ => {}
        }
    }
}

impl Dispatch<WlBuffer, u64> for State {
    fn event(
        state: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        id: &u64,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_buffer::Event::Release = event else {
            return;
        };
        let Some(panel) = state.panel.as_mut() else {
            return;
        };
        if let Some(buffer) = panel.buffers.iter_mut().find(|buffer| buffer.id == *id) {
            buffer.busy = false;
            if state.frame.is_some() {
                state.draw(handle);
            }
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
delegate_noop!(State: ignore WlOutput);
delegate_noop!(State: ignore ZwlrLayerShellV1);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);
delegate_noop!(State: ignore WpFractionalScaleManagerV1);
delegate_noop!(State: ignore WpCursorShapeManagerV1);
delegate_noop!(State: ignore WpCursorShapeDeviceV1);
