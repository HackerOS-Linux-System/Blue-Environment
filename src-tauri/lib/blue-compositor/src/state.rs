use calloop::LoopHandle;
use smithay::{
    delegate_compositor, delegate_data_device, delegate_fractional_scale,
    delegate_layer_shell, delegate_output, delegate_presentation,
    delegate_primary_selection, delegate_seat, delegate_shm,
    delegate_viewporter, delegate_xdg_shell,
    desktop::{PopupManager, Space, Window},
    input::{Seat, SeatHandler, SeatState, pointer::CursorImageStatus},
    output::Output,
    reexports::{
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Clock, Logical, Monotonic, Point, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        output::{OutputHandler, OutputManagerState},
        presentation::PresentationState,
        seat::WaylandFocus,
        selection::{
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler},
            primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
            SelectionHandler, SelectionSource, SelectionTarget,
        },
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface,
                WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::{
                PopupSurface, PositionerState, ToplevelSurface,
                XdgShellHandler, XdgShellState,
            },
        },
        shm::{ShmHandler, ShmState},
        socket::ListeningSocketSource,
        viewporter::ViewporterState,
    },
    input::dnd::DndGrabHandler,
    xwayland::{XWayland, xwm::X11Wm},
};
use std::{
    collections::HashMap,
    os::unix::io::OwnedFd,
    os::unix::net::UnixStream,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{info, warn};

// ── IPC window info ────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_fullscreen: bool,
    pub is_minimized: bool,
    pub workspace: u32,
}

// ── Client state ───────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

// ── Backend data ───────────────────────────────────────────────────────────

pub enum BackendData {
    None,
    Udev(Box<UdevData>),
    Winit(Box<WinitData>),
}

pub struct UdevData {
    pub session: smithay::backend::session::libseat::LibSeatSession,
    pub primary_gpu: smithay::backend::drm::DrmNode,
    pub devices: HashMap<smithay::backend::drm::DrmNode, GpuDevice>,
}

pub struct GpuDevice {
    pub drm: smithay::backend::drm::DrmDevice,
    pub gbm: smithay::backend::allocator::gbm::GbmDevice<smithay::backend::drm::DrmDeviceFd>,
}

pub struct WinitData {
    pub backend: smithay::backend::winit::WinitGraphicsBackend<smithay::backend::renderer::gles::GlesRenderer>,
    pub output: Output,
    pub damage_tracker: smithay::backend::renderer::damage::OutputDamageTracker,
}

// ── Multi‑monitor configuration ───────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct OutputConfig {
    pub name: String,
    pub position: Point<i32, Logical>,
    pub scale: f64,
    pub mode: smithay::output::Mode,
}

// ── Main compositor state ──────────────────────────────────────────────────

pub struct BlueState {
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub clock: Clock<Monotonic>,
    pub socket_name: String,

    pub space: Space<Window>,
    pub popup_manager: PopupManager,
    pub current_workspace: usize,
    pub workspace_count: usize,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Self>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub layer_shell_state: WlrLayerShellState,
    pub presentation_state: PresentationState,
    pub fractional_scale_manager_state: FractionalScaleManagerState,
    pub viewporter_state: ViewporterState,

    pub seat: Seat<Self>,
    pub pointer_location: Point<f64, Logical>,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,
    pub outputs: Vec<Output>,

    pub xwayland: Option<XWayland>,
    pub xwm: Option<X11Wm>,
    pub x11_display: Option<u32>,

    pub backend_data: BackendData,
    pub ipc_windows: Arc<Mutex<Vec<WindowInfo>>>,
    pub clients: Arc<Mutex<Vec<UnixStream>>>,
    pub should_exit: bool,

    // Multi‑monitor configuration
    pub output_configs: Vec<OutputConfig>,

    // Idle (DPMS disabled)
    pub last_input_time: Instant,
    pub dpms_blanked: bool,
    pub dpms_timeout: Duration,

    // Window switcher and keyboard shortcuts
    pub show_switcher: bool,
    pub switcher_index: usize,
    pub super_pressed: bool,
    pub super_used: bool,
    pub start_menu_visible: bool,
    pub fullscreen_menu_visible: bool,
}

impl BlueState {
    pub fn new(
        loop_handle: &LoopHandle<'static, Self>,
        display: Display<Self>,
    ) -> Self {
        let display_handle = display.handle();
        let clock = Clock::new();

        let compositor_state = CompositorState::new::<Self>(&display_handle);
        let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
        let shm_state = ShmState::new::<Self>(&display_handle, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display_handle);
        let mut seat_state = SeatState::new();
        let seat = seat_state.new_wl_seat(&display_handle, "seat0");
        let data_device_state = DataDeviceState::new::<Self>(&display_handle);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&display_handle);
        let presentation_state = PresentationState::new::<Self>(&display_handle, clock.id() as u32);
        let fractional_scale_manager_state = FractionalScaleManagerState::new::<Self>(&display_handle);
        let viewporter_state = ViewporterState::new::<Self>(&display_handle);

        let socket = ListeningSocketSource::new_auto()
        .expect("Failed to create Wayland socket");
        let socket_name = socket.socket_name().to_string_lossy().to_string();
        info!("Wayland socket: {}", socket_name);

        loop_handle
        .insert_source(socket, |client, _, state: &mut BlueState| {
            if let Err(e) = state
                .display_handle
                .insert_client(client, Arc::new(ClientState::default()))
                {
                    warn!("Failed to insert client: {}", e);
                }
        })
        .expect("Failed to init socket source");

        BlueState {
            display_handle,
            loop_handle: loop_handle.clone(),
            clock,
            socket_name,
            space: Space::default(),
            popup_manager: PopupManager::default(),
            current_workspace: 0,
            workspace_count: 4,
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            layer_shell_state,
            presentation_state,
            fractional_scale_manager_state,
            viewporter_state,
            seat,
            pointer_location: Point::from((0.0, 0.0)),
            cursor_status: Arc::new(Mutex::new(CursorImageStatus::default_named())),
            outputs: Vec::new(),
            xwayland: None,
            xwm: None,
            x11_display: None,
            backend_data: BackendData::None,
            ipc_windows: Arc::new(Mutex::new(Vec::new())),
            clients: Arc::new(Mutex::new(Vec::new())),
            should_exit: false,
            output_configs: Vec::new(),
            last_input_time: Instant::now(),
            dpms_blanked: false,
            dpms_timeout: Duration::from_secs(300),
            show_switcher: false,
            switcher_index: 0,
                super_pressed: false,
                super_used: false,
                start_menu_visible: false,
                fullscreen_menu_visible: false,
        }
    }

    pub fn socket_name(&self) -> &str { &self.socket_name }
    pub fn should_exit(&self) -> bool { self.should_exit }

    pub fn refresh(&mut self) {
        self.space.refresh();
        self.popup_manager.cleanup();
        self.update_ipc_windows();
        // DPMS disabled – brak wywołania handle_dpms
    }

    fn update_ipc_windows(&self) {
        let windows: Vec<WindowInfo> = self
        .space
        .elements()
        .map(|win| {
            let geo = self.space.element_geometry(win).unwrap_or_default();
            let surface_id = win
            .wl_surface()
            .map(|s| s.id().protocol_id() as u64)
            .unwrap_or(0);
            WindowInfo {
                id: surface_id,
                title: String::new(),
             app_id: String::new(),
             x: geo.loc.x,
             y: geo.loc.y,
             width: geo.size.w as u32,
             height: geo.size.h as u32,
             is_fullscreen: false,
             is_minimized: false,
             workspace: self.current_workspace as u32,
            }
        })
        .collect();
        *self.ipc_windows.lock().unwrap() = windows;
    }

    pub fn init_udev(
        &mut self,
        session: smithay::backend::session::libseat::LibSeatSession,
        loop_handle: &LoopHandle<'static, Self>,
    ) {
        crate::render::init_udev(self, session, loop_handle);
    }

    pub fn init_winit(
        &mut self,
        backend: smithay::backend::winit::WinitGraphicsBackend<smithay::backend::renderer::gles::GlesRenderer>,
        events: smithay::backend::winit::WinitEventLoop,
        loop_handle: &LoopHandle<'static, Self>,
    ) {
        crate::render::init_winit(self, backend, events, loop_handle);
    }

    pub fn init_xwayland(&mut self, loop_handle: &LoopHandle<'static, Self>) -> Result<(), Box<dyn std::error::Error>> {
        crate::xwayland::init_xwayland(self, loop_handle)
    }

    pub fn init_ipc(&mut self, loop_handle: &LoopHandle<'static, Self>) {
        crate::ipc::init_ipc(self, loop_handle);
    }

    pub fn window_by_surface(&self, surface: &WlSurface) -> Option<Window> {
        self.space
        .elements()
        .find(|w| w.wl_surface().as_deref() == Some(surface))
        .cloned()
    }

    pub fn window_by_id(&self, id: u64) -> Option<Window> {
        self.space
        .elements()
        .find(|w| {
            w.wl_surface()
            .map(|s| s.id().protocol_id() as u64 == id)
            .unwrap_or(false)
        })
        .cloned()
    }

    pub fn record_input(&mut self) {
        self.last_input_time = Instant::now();
        // DPMS disabled
    }

    pub fn toggle_start_menu(&mut self) {
        self.start_menu_visible = !self.start_menu_visible;
    }

    pub fn toggle_fullscreen_menu(&mut self) {
        self.fullscreen_menu_visible = !self.fullscreen_menu_visible;
    }

    pub fn cycle_switcher(&mut self, forward: bool) {
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if windows.is_empty() {
            return;
        }
        if forward {
            self.switcher_index = (self.switcher_index + 1) % windows.len();
        } else {
            self.switcher_index = (self.switcher_index + windows.len() - 1) % windows.len();
        }
    }

    pub fn apply_switcher_selection(&mut self) {
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if let Some(win) = windows.get(self.switcher_index) {
            self.space.raise_element(win, true);
            if let Some(surface) = win.wl_surface() {
                if let Some(keyboard) = self.seat.get_keyboard() {
                    let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                    keyboard.set_focus(self, Some(surface.into_owned()), serial);
                }
            }
        }
        self.show_switcher = false;
    }
}

// ── Protocol implementations (pełne, bez zmian) ─────────────────────────────
impl BufferHandler for BlueState {
    fn buffer_destroyed(&mut self, _buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {}
}
impl CompositorHandler for BlueState {
    fn compositor_state(&mut self) -> &mut CompositorState { &mut self.compositor_state }
    fn client_compositor_state<'a>(&self, client: &'a smithay::reexports::wayland_server::Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }
    fn commit(&mut self, surface: &WlSurface) {
        smithay::backend::renderer::utils::on_commit_buffer_handler::<Self>(surface);
        if let Some(window) = self.window_by_surface(surface) {
            window.on_commit();
        }
        self.popup_manager.commit(surface);
    }
}
impl ShmHandler for BlueState {
    fn shm_state(&self) -> &ShmState { &self.shm_state }
}
impl OutputHandler for BlueState {}
impl SeatHandler for BlueState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;
    fn seat_state(&mut self) -> &mut SeatState<Self> { &mut self.seat_state }
    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        *self.cursor_status.lock().unwrap() = image;
    }
}
impl XdgShellHandler for BlueState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState { &mut self.xdg_shell_state }
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        let offset = self.space.elements().count() * 30;
        let loc = Point::from(((offset + 150) as i32, (offset + 80) as i32));
        self.space.map_element(window, loc, true);
    }
    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        let _ = self.popup_manager.track_popup(smithay::desktop::PopupKind::Xdg(surface));
    }
    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {}
    fn move_request(&mut self, surface: ToplevelSurface, seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();
        let wl = surface.wl_surface().clone();
        if let Some(window) = self.window_by_surface(&wl) {
            if let Some(pointer) = seat.get_pointer() {
                if let Some(start_data) = pointer.grab_start_data() {
                    crate::input::start_move_grab(self, window, start_data, serial);
                }
            }
        }
    }
    fn resize_request(&mut self, _surface: ToplevelSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial, _edges: xdg_toplevel::ResizeEdge) {}
    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {}
    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let wl = surface.wl_surface().clone();
        if let Some(w) = self.window_by_surface(&wl) {
            self.space.unmap_elem(&w);
        }
    }
}
impl WlrLayerShellHandler for BlueState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState { &mut self.layer_shell_state }
    fn new_layer_surface(&mut self, _surface: WlrLayerSurface, _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>, _layer: Layer, _namespace: String) {}
    fn layer_destroyed(&mut self, _surface: WlrLayerSurface) {}
}
impl SelectionHandler for BlueState {
    type SelectionUserData = ();
    fn send_selection(&mut self, _target: SelectionTarget, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>, _user_data: &Self::SelectionUserData) {}
    fn new_selection(&mut self, _target: SelectionTarget, _source: Option<SelectionSource>, _seat: Seat<Self>) {}
}
impl WaylandDndGrabHandler for BlueState {}
impl DataDeviceHandler for BlueState {
    fn data_device_state(&mut self) -> &mut DataDeviceState { &mut self.data_device_state }
}
impl PrimarySelectionHandler for BlueState {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState { &mut self.primary_selection_state }
}
impl FractionalScaleHandler for BlueState {
    fn new_fractional_scale(&mut self, _surface: WlSurface) {}
}
impl DndGrabHandler for BlueState {}

delegate_compositor!(BlueState);
delegate_shm!(BlueState);
delegate_seat!(BlueState);
delegate_xdg_shell!(BlueState);
delegate_layer_shell!(BlueState);
delegate_output!(BlueState);
delegate_data_device!(BlueState);
delegate_primary_selection!(BlueState);
delegate_presentation!(BlueState);
delegate_viewporter!(BlueState);
delegate_fractional_scale!(BlueState);
