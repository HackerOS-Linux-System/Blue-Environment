use smithay::{
    delegate_compositor, delegate_data_device, delegate_fractional_scale,
    delegate_layer_shell, delegate_output, delegate_presentation,
    delegate_primary_selection, delegate_seat, delegate_shm,
    delegate_viewporter, delegate_xdg_shell,
    backend::{
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer},
        udev::UdevBackend,
        winit::{WinitEventLoop, WinitGraphicsBackend},
        allocator::gbm::GbmDevice,
        drm::{DrmDevice, DrmDeviceFd, DrmNode},
        session::libseat::LibSeatSession,
    },
    desktop::{Space, Window},
    input::{Seat, SeatState, SeatHandler, pointer::CursorImageStatus},
    output::Output,
    reexports::{
        calloop::LoopHandle,
        wayland_server::{
            backend::ClientData,
            Display, DisplayHandle, protocol::wl_surface::WlSurface, Client,
        },
    },
    utils::{Clock, Logical, Monotonic, Point, Serial, SERIAL_COUNTER},
    wayland::{
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        data_device::{DataDeviceHandler, DataDeviceState, ClientDndGrabHandler, ServerDndGrabHandler},
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        output::OutputManagerState,
        presentation::PresentationState,
        primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
        seat::WaylandFocus,
        shell::{
            wlr_layer::{Layer, LayerSurface, LayerSurfaceData, WlrLayerShellHandler, WlrLayerShellState},
            xdg::{
                XdgShellHandler, XdgShellState, ToplevelSurface,
                PopupSurface, PositionerState, SurfaceCachedState,
            },
        },
        shm::{ShmHandler, ShmState},
        viewporter::ViewporterState,
        xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData},
        xwayland_shell::XWaylandShellState,
    },
    xwayland::{XWayland, xwm::X11Wm},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{error, info, warn};

// ── Public types ─────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct WindowMeta {
    pub title: String,
    pub app_id: String,
    pub workspace: usize,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub workspace: usize,
    pub is_minimized: bool,
    pub is_fullscreen: bool,
    pub is_focused: bool,
}

pub struct GpuDevice {
    pub drm: DrmDevice,
    pub gbm: GbmDevice<DrmDeviceFd>,
}

pub struct WinitData {
    pub backend: WinitGraphicsBackend<GlesRenderer>,
    pub output: Output,
    pub damage_tracker: OutputDamageTracker,
}

pub struct UdevData {
    pub session: LibSeatSession,
    pub primary_gpu: DrmNode,
    pub devices: HashMap<DrmNode, GpuDevice>,
}

pub enum BackendData {
    Winit(Box<WinitData>),
    Udev(Box<UdevData>),
    None,
}

#[derive(Default, Clone)]
struct ClientState {
    compositor_state: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _client_id: wayland_server::backend::ClientId) {}
    fn disconnected(&self, _client_id: wayland_server::backend::ClientId, _reason: wayland_server::backend::DisconnectReason) {}
}

// ── BlueState ─────────────────────────────────────────────────────────────

pub struct BlueState {
    // Wayland core
    pub display_handle: DisplayHandle,
    pub loop_handle: LoopHandle<'static, BlueState>,
    pub clock: Clock<Monotonic>,

    // Protocols
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<BlueState>,
    pub data_device_state: DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub output_manager_state: OutputManagerState,
    pub presentation_state: PresentationState,
    pub fractional_scale_manager_state: FractionalScaleManagerState,
    pub viewporter_state: ViewporterState,
    pub layer_shell_state: WlrLayerShellState,
    pub xdg_activation_state: XdgActivationState,
    pub xwayland_shell_state: XWaylandShellState,

    // Seat
    pub seat: Seat<BlueState>,
    pub pointer_location: Point<f64, Logical>,
    pub cursor_status: CursorImageStatus,

    // Desktop
    pub space: Space<Window>,
    pub outputs: Vec<Output>,
    pub window_meta: HashMap<u64, WindowMeta>,
    pub current_workspace: usize,
    pub workspace_count: usize,
    pub should_exit: bool,

    // Backend
    pub backend_data: BackendData,

    // XWayland
    pub xwayland: Option<XWayland>,
    pub xwm: Option<X11Wm>,
    pub x11_display: Option<u32>,

    // IPC
    pub socket_name: String,
    pub ipc_windows: Arc<Mutex<Vec<WindowInfo>>>,
}

impl BlueState {
    pub fn new(
        loop_handle: &LoopHandle<'static, BlueState>,
        display: Display<BlueState>,
    ) -> Self {
        let display_handle = display.handle();
        let dh = display_handle.clone();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let presentation_state = PresentationState::new::<Self>(&dh, 1);
        let fractional_scale_manager_state = FractionalScaleManagerState::new::<Self>(&dh);
        let viewporter_state = ViewporterState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let xdg_activation_state = XdgActivationState::new::<Self>(&dh);
        let xwayland_shell_state = XWaylandShellState::new::<Self>(&dh);

        let seat_name = "seat0".to_string();
        let seat = seat_state.new_wl_seat(&dh, seat_name);

        // Socket
        let socket_name = format!("wayland-blue-{}", std::process::id());
        let socket = smithay::wayland::socket::ListeningSocketSource::with_name(&socket_name)
            .expect("Failed to create wayland socket");

        // Clone for closure
        let dh2 = dh.clone();
        loop_handle
            .insert_source(socket, move |client, _, _state: &mut BlueState| {
                if let Err(e) = dh2.insert_client(client, Arc::new(ClientState::default())) {
                    warn!("Failed to insert client: {}", e);
                }
            })
            .expect("Failed to insert socket source");

        // Flush display
        let dh3 = dh.clone();
        loop_handle
            .insert_source(
                smithay::reexports::calloop::generic::Generic::new(
                    display,
                    smithay::reexports::calloop::Interest::READ,
                    smithay::reexports::calloop::Mode::Level,
                ),
                move |_, display, state: &mut BlueState| {
                    // Safety: display not moved
                    unsafe { display.get_mut().dispatch_clients(state).unwrap() };
                    Ok(smithay::reexports::calloop::PostAction::Continue)
                },
            )
            .expect("Failed to insert display source");

        Self {
            display_handle,
            loop_handle: loop_handle.clone(),
            clock: Clock::new(),
            compositor_state,
            xdg_shell_state,
            shm_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            output_manager_state,
            presentation_state,
            fractional_scale_manager_state,
            viewporter_state,
            layer_shell_state,
            xdg_activation_state,
            xwayland_shell_state,
            seat,
            pointer_location: Point::from((0.0, 0.0)),
            cursor_status: CursorImageStatus::default_named(),
            space: Space::default(),
            outputs: Vec::new(),
            window_meta: HashMap::new(),
            current_workspace: 0,
            workspace_count: 4,
            should_exit: false,
            backend_data: BackendData::None,
            xwayland: None,
            xwm: None,
            x11_display: None,
            socket_name,
            ipc_windows: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn socket_name(&self) -> &str { &self.socket_name }
    pub fn should_exit(&self) -> bool { self.should_exit }

    pub fn init_udev(&mut self, session: LibSeatSession, lh: &LoopHandle<'static, BlueState>) {
        crate::render::init_udev(self, session, lh);
    }

    pub fn init_winit(
        &mut self,
        backend: WinitGraphicsBackend<GlesRenderer>,
        events: WinitEventLoop,
        lh: &LoopHandle<'static, BlueState>,
    ) {
        crate::render::init_winit(self, backend, events, lh);
    }

    pub fn init_xwayland(&mut self, lh: &LoopHandle<'static, BlueState>) -> Result<(), Box<dyn std::error::Error>> {
        crate::xwayland::init_xwayland(self, lh)
    }

    pub fn init_ipc(&mut self, lh: &LoopHandle<'static, BlueState>) {
        crate::ipc::init_ipc(self, lh);
    }

    pub fn refresh(&mut self) {
        self.space.refresh();
        self.update_ipc_windows();
        if let Err(e) = self.display_handle.flush_clients() {
            warn!("flush_clients: {}", e);
        }
    }

    fn update_ipc_windows(&self) {
        let mut list = self.ipc_windows.lock().unwrap();
        list.clear();
        let focused_id = self.seat.get_keyboard()
            .and_then(|kb| kb.current_focus())
            .and_then(|s| s.wl_surface())
            .map(|s| s.id().protocol_id() as u64);

        for win in self.space.elements() {
            let id = Self::window_id(win);
            let meta = self.window_meta.get(&id).cloned().unwrap_or_default();
            list.push(WindowInfo {
                id,
                title: meta.title,
                app_id: meta.app_id,
                workspace: meta.workspace,
                is_minimized: meta.is_minimized,
                is_fullscreen: meta.is_fullscreen,
                is_focused: focused_id == Some(id),
            });
        }
    }

    pub fn window_id(win: &Window) -> u64 {
        win.wl_surface()
            .map(|s| s.id().protocol_id() as u64)
            .unwrap_or(0)
    }

    pub fn window_by_id(&self, id: u64) -> Option<Window> {
        self.space.elements().find(|w| Self::window_id(w) == id).cloned()
    }

    pub fn switch_workspace(&mut self, idx: usize) {
        if idx >= self.workspace_count { return; }
        self.current_workspace = idx;
        for win in self.space.elements().cloned().collect::<Vec<_>>() {
            let id = Self::window_id(&win);
            let visible = self.window_meta.get(&id).map(|m| m.workspace == idx).unwrap_or(true);
            if visible {
                self.space.raise_element(&win, false);
            }
        }
    }
}

// ── Protocol handlers ─────────────────────────────────────────────────────

impl CompositorHandler for BlueState {
    fn compositor_state(&mut self) -> &mut CompositorState { &mut self.compositor_state }
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }
    fn commit(&mut self, surface: &WlSurface) {
        smithay::backend::renderer::utils::on_commit_buffer_handler::<Self>(surface);
        self.space.commit(surface);
    }
}
delegate_compositor!(BlueState);

impl XdgShellHandler for BlueState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState { &mut self.xdg_shell_state }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let win = Window::new_wayland_window(surface.clone());
        let count = self.space.elements().count();
        let loc = Point::from((150 + (count % 10) as i32 * 30, 80 + (count % 10) as i32 * 30));
        self.space.map_element(win.clone(), loc, true);
        let id = Self::window_id(&win);
        if id != 0 {
            let state = surface.current_state();
            self.window_meta.insert(id, WindowMeta {
                title: state.title.clone().unwrap_or_default(),
                app_id: state.app_id.clone().unwrap_or_default(),
                workspace: self.current_workspace,
                ..Default::default()
            });
        }
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let found = self.space.elements()
            .find(|w| w.toplevel().map(|t| t == &surface).unwrap_or(false))
            .cloned();
        if let Some(win) = found {
            let id = Self::window_id(&win);
            self.window_meta.remove(&id);
            self.space.unmap_elem(&win);
        }
    }

    fn popup_destroyed(&mut self, _surface: PopupSurface) {}

    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {}
}
delegate_xdg_shell!(BlueState);

impl ShmHandler for BlueState {
    fn shm_state(&self) -> &ShmState { &self.shm_state }
}
delegate_shm!(BlueState);

impl SeatHandler for BlueState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;
    fn seat_state(&mut self) -> &mut SeatState<Self> { &mut self.seat_state }
    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}
    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}
}
delegate_seat!(BlueState);

impl DataDeviceHandler for BlueState {
    fn data_device_state(&self) -> &DataDeviceState { &self.data_device_state }
}
impl ClientDndGrabHandler for BlueState {}
impl ServerDndGrabHandler for BlueState {}
delegate_data_device!(BlueState);

impl PrimarySelectionHandler for BlueState {
    fn primary_selection_state(&self) -> &PrimarySelectionState { &self.primary_selection_state }
}
delegate_primary_selection!(BlueState);

delegate_output!(BlueState);

impl FractionalScaleHandler for BlueState {
    fn new_fractional_scale(&mut self, _surface: WlSurface) {}
}
delegate_fractional_scale!(BlueState);
delegate_viewporter!(BlueState);
delegate_presentation!(BlueState);

impl WlrLayerShellHandler for BlueState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState { &mut self.layer_shell_state }
    fn new_layer_surface(&mut self, surface: LayerSurface, _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>, _layer: Layer, _namespace: String) {
        surface.send_configure();
    }
    fn layer_destroyed(&mut self, _surface: LayerSurface) {}
}
delegate_layer_shell!(BlueState);

impl XdgActivationHandler for BlueState {
    fn activation_state(&self) -> &XdgActivationState { &self.xdg_activation_state }
    fn token_created(&mut self, _token: XdgActivationToken, _data: XdgActivationTokenData) -> bool { true }
    fn request_activation(&mut self, _token: XdgActivationToken, _token_data: XdgActivationTokenData, _surface: WlSurface) {}
}
smithay::delegate_xdg_activation!(BlueState);
