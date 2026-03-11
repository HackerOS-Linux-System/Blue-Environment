pub mod backend;

use anyhow::Result;
use parking_lot::RwLock;
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_data_device, delegate_fractional_scale,
    delegate_layer_shell, delegate_output, delegate_presentation,
    delegate_primary_selection, delegate_seat, delegate_shm,
    delegate_viewporter, delegate_xdg_decoration, delegate_xdg_shell,
    desktop::Window,
    input::{keyboard::KeyboardHandle, pointer::PointerHandle, SeatHandler, SeatState},
    // OutputHandler jest w smithay::wayland::output, nie smithay::output
    output::Output,
    reexports::{
        calloop::{LoopHandle, LoopSignal},
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1,
        wayland_server::{
            backend::{ClientData as WlClientData, ClientId, DisconnectReason},
            protocol::{wl_seat::WlSeat, wl_surface::WlSurface},
            Display, DisplayHandle, Resource,
        },
    },
    utils::{Clock, Monotonic, Serial},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState, CompositorHandler, CompositorState},
        dmabuf::DmabufState,
        fractional_scale::{FractionalScaleHandler, FractionalScaleManagerState},
        output::{OutputHandler, OutputManagerState},
        presentation::PresentationState,
        selection::{
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState,
                ServerDndGrabHandler,
            },
            primary_selection::{PrimarySelectionHandler, PrimarySelectionState},
            SelectionHandler,
        },
        shell::{
            wlr_layer::{Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState},
            xdg::{
                decoration::{XdgDecorationHandler, XdgDecorationState},
                PopupSurface, PositionerState, ToplevelSurface,
                XdgShellHandler, XdgShellState,
            },
        },
        shm::{ShmHandler, ShmState},
        viewporter::ViewporterState,
    },
};
use std::sync::Arc;
use tracing::{debug, info};

use crate::{
    config::BlueConfig,
    csd::{CsdManager, DecoMode},
    input::InputState,
    ipc::IpcServer,
    wallpaper::WallpaperManager,
    workspace::WorkspaceManager,
};
use backend::BackendType;

// ── Per-client state ──────────────────────────────────────

pub struct ClientState {
    pub compositor: CompositorClientState,
}
impl Default for ClientState {
    fn default() -> Self { Self { compositor: CompositorClientState::default() } }
}
impl WlClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

// ── Central state ─────────────────────────────────────────

pub struct BlueState {
    pub dh:          DisplayHandle,
    pub loop_handle: LoopHandle<'static, Self>,
    pub loop_signal: LoopSignal,
    pub config:      Arc<RwLock<BlueConfig>>,

    pub compositor_state:        CompositorState,
    pub xdg_shell_state:         XdgShellState,
    pub xdg_decoration_state:    XdgDecorationState,
    pub layer_shell_state:       WlrLayerShellState,
    pub shm_state:               ShmState,
    pub output_manager_state:    OutputManagerState,
    pub dmabuf_state:            DmabufState,
    pub data_device_state:       DataDeviceState,
    pub primary_selection_state: PrimarySelectionState,
    pub seat_state:              SeatState<Self>,
    pub presentation_state:      PresentationState,
    pub viewporter_state:        ViewporterState,
    pub fractional_scale_state:  FractionalScaleManagerState,

    pub keyboard:    Option<KeyboardHandle<Self>>,
    pub pointer:     Option<PointerHandle<Self>>,
    pub input_state: InputState,

    pub workspaces: WorkspaceManager,
    pub csd:        CsdManager,
    pub outputs:    Vec<Output>,
    pub wallpaper:  WallpaperManager,
    pub ipc:        IpcServer,

    pub backend_type: BackendType,
    pub clock:        Clock<Monotonic>,
    pub seat_name:    String,
}

impl BlueState {
    pub fn new(
        display:      &mut Display<Self>,
        loop_handle:  LoopHandle<'static, Self>,
        loop_signal:  LoopSignal,
        config:       BlueConfig,
        backend_type: BackendType,
    ) -> Result<Self> {
        let dh    = display.handle();
        let clock = Clock::new();

        let compositor_state        = CompositorState::new::<Self>(&dh);
        let xdg_shell_state         = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state    = XdgDecorationState::new::<Self>(&dh);
        let layer_shell_state       = WlrLayerShellState::new::<Self>(&dh);
        let shm_state               = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state    = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let dmabuf_state            = DmabufState::new();
        let data_device_state       = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let presentation_state      = PresentationState::new::<Self>(&dh, clock.id() as u32);
        let viewporter_state        = ViewporterState::new::<Self>(&dh);
        let fractional_scale_state  = FractionalScaleManagerState::new::<Self>(&dh);

        let seat_name      = "seat0".to_string();
        let mut seat_state = SeatState::new();
        let mut seat       = seat_state.new_wl_seat(&dh, &seat_name);

        let keyboard = seat.add_keyboard(
            smithay::input::keyboard::XkbConfig {
                layout: &config.keyboard_layout.clone(),
                                         ..Default::default()
            },
            config.keyboard_repeat_delay as i32,
            config.keyboard_repeat_rate as i32,
        ).ok();

        let pointer = Some(seat.add_pointer());
        let wc      = config.workspace_count;
        let wp      = config.wallpaper.clone();

        Ok(Self {
            dh, loop_handle, loop_signal,
            config: Arc::new(RwLock::new(config)),
           compositor_state, xdg_shell_state, xdg_decoration_state,
           layer_shell_state, shm_state, output_manager_state, dmabuf_state,
           data_device_state, primary_selection_state, seat_state,
           presentation_state, viewporter_state, fractional_scale_state,
           keyboard, pointer,
           input_state: InputState::new(),
           workspaces:  WorkspaceManager::new(wc),
           csd:         CsdManager::new(),
           outputs:     Vec::new(),
           wallpaper:   WallpaperManager::new(&wp),
           ipc:         IpcServer::new()?,
           backend_type, clock, seat_name,
        })
    }
}

// ── Run ───────────────────────────────────────────────────

pub fn run(config: BlueConfig, backend_type: BackendType) -> Result<()> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        info!("Nested mode → winit backend");
        backend::winit::run(config, backend_type)
    } else {
        info!("TTY mode → udev/DRM backend");
        backend::udev::run(config, backend_type)
    }
}

// ── Protocol handlers ─────────────────────────────────────

impl CompositorHandler for BlueState {
    fn compositor_state(&mut self) -> &mut CompositorState { &mut self.compositor_state }
    fn client_compositor_state<'a>(
        &self, client: &'a smithay::reexports::wayland_server::Client,
    ) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor
    }
    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        self.workspaces.handle_commit(surface);
    }
}

impl XdgShellHandler for BlueState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState { &mut self.xdg_shell_state }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        debug!("New XDG toplevel");
        let id = surface.wl_surface().id().protocol_id();
        let title = smithay::wayland::compositor::with_states(
            surface.wl_surface(),
                                                              |states| {
                                                                  states
                                                                  .data_map
                                                                  .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                                                                  .and_then(|d| d.lock().ok())
                                                                  .and_then(|d| d.title.clone())
                                                                  .unwrap_or_else(|| "Untitled".to_string())
                                                              },
        );
        self.csd.register(id, &title);
        let window = Window::new_wayland_window(surface);
        self.workspaces.add_window(window);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        self.workspaces.handle_new_popup(surface, positioner);
    }
    fn grab(&mut self, _s: PopupSurface, _seat: WlSeat, _serial: Serial) {}
    fn reposition_request(
        &mut self, surface: PopupSurface, positioner: PositionerState, token: u32,
    ) {
        surface.with_pending_state(|s| {
            s.geometry   = positioner.get_geometry();
            s.positioner = positioner;
        });
        surface.send_repositioned(token);
    }
    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.csd.unregister(surface.wl_surface().id().protocol_id());
    }
}

impl XdgDecorationHandler for BlueState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        // Wymuś ServerSide — używamy zxdg Mode bezpośrednio (smithay 0.6)
        toplevel.with_pending_state(|s| {
            s.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
        });
        toplevel.send_pending_configure();
        let id = toplevel.wl_surface().id().protocol_id();
        if let Some(d) = self.csd.get_mut(id) { d.mode = DecoMode::ServerSide; }
    }

    fn request_mode(
        &mut self,
        toplevel: ToplevelSurface,
        _mode: zxdg_toplevel_decoration_v1::Mode,
    ) {
        // Zawsze odpowiadamy ServerSide
        toplevel.with_pending_state(|s| {
            s.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
        });
        toplevel.send_pending_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        let id = toplevel.wl_surface().id().protocol_id();
        if let Some(d) = self.csd.get_mut(id) { d.mode = DecoMode::ServerSide; }
    }
}

impl WlrLayerShellHandler for BlueState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState { &mut self.layer_shell_state }
    fn new_layer_surface(
        &mut self, surface: LayerSurface,
        _output: Option<smithay::reexports::wayland_server::protocol::wl_output::WlOutput>,
        _layer: Layer, namespace: String,
    ) {
        debug!("New layer surface: {}", namespace);
        if let Some(output) = self.outputs.first() {
            smithay::desktop::layer_map_for_output(output)
            .map_layer(&smithay::desktop::LayerSurface::new(surface, namespace))
            .ok();
        }
    }
    fn layer_destroyed(&mut self, _surface: LayerSurface) {}
}

impl SeatHandler for BlueState {
    type KeyboardFocus = WlSurface;
    type PointerFocus  = WlSurface;
    type TouchFocus    = WlSurface;
    fn seat_state(&mut self) -> &mut SeatState<Self> { &mut self.seat_state }
    fn focus_changed(&mut self, _: &smithay::input::Seat<Self>, _: Option<&WlSurface>) {}
    fn cursor_image(&mut self, _: &smithay::input::Seat<Self>, _: smithay::input::pointer::CursorImageStatus) {}
}

impl ShmHandler for BlueState {
    fn shm_state(&self) -> &ShmState { &self.shm_state }
}
impl BufferHandler for BlueState {
    fn buffer_destroyed(&mut self, _: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer) {}
}

impl SelectionHandler for BlueState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for BlueState {
    fn data_device_state(&self) -> &DataDeviceState { &self.data_device_state }
}
impl ClientDndGrabHandler for BlueState {}
impl ServerDndGrabHandler for BlueState {}

impl PrimarySelectionHandler for BlueState {
    fn primary_selection_state(&self) -> &PrimarySelectionState { &self.primary_selection_state }
}

impl OutputHandler for BlueState {}

impl FractionalScaleHandler for BlueState {
    fn new_fractional_scale(&mut self, _surface: WlSurface) {}
}

delegate_compositor!(BlueState);
delegate_xdg_shell!(BlueState);
delegate_xdg_decoration!(BlueState);
delegate_layer_shell!(BlueState);
delegate_shm!(BlueState);
delegate_output!(BlueState);
delegate_seat!(BlueState);
delegate_data_device!(BlueState);
delegate_primary_selection!(BlueState);
delegate_presentation!(BlueState);
delegate_viewporter!(BlueState);
delegate_fractional_scale!(BlueState);
