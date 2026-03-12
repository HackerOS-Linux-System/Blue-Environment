use anyhow::{anyhow, Result};
use std::{
    collections::HashMap,
    os::unix::io::OwnedFd,
    sync::Arc,
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        drm::{
            compositor::{DrmCompositor, FrameFlags},
            exporter::gbm::GbmFramebufferExporter,
            DrmDevice, DrmDeviceFd, DrmNode,
        },
        egl::{EGLContext, EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{
            damage::OutputDamageTracker,
            element::solid::SolidColorRenderElement,
            gles::{GlesRenderer, GlesTexture},
            ImportDma, ImportMem,
        },
        session::{libseat::LibSeatSession, Session},
        udev::UdevBackend,
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            channel::{channel, Channel, Sender},
            EventLoop,
        },
        drm::{
            buffer::DrmFourcc,
            control::{connector, crtc, Device as DrmControlDevice, ModeTypeFlags},
        },
        input::Libinput,
        rustix::fs::OFlags,
        wayland_server::{Display, ListeningSocket},
    },
    utils::{Buffer, Physical, Rectangle, Size, Transform},
};
use tracing::{debug, info, warn};

use crate::{
    compositor::{BlueState, ClientState},
    config::BlueConfig,
    session,
};
use super::BackendType;

// ══════════════════════════════════════════════════════════
// Typy
// ══════════════════════════════════════════════════════════

// DrmCompositor<A, F, U, G>
//   A = GbmAllocator<DrmDeviceFd>
//   F = GbmFramebufferExporter<DrmDeviceFd>  (ExportFramebuffer<GbmBuffer>)
//   U = ()
//   G = DrmDeviceFd
type BlueDrmCompositor = DrmCompositor<
GbmAllocator<DrmDeviceFd>,
GbmFramebufferExporter<DrmDeviceFd>,
(),
DrmDeviceFd,
>;

struct OutputDevice {
    output:          Output,
    compositor:      BlueDrmCompositor,
    _damage_tracker: OutputDamageTracker,
    size:            (u32, u32),
    wallpaper_tex:   Option<GlesTexture>,
    wallpaper_dirty: bool,
}

enum BackendMsg { RenderTick }

// ══════════════════════════════════════════════════════════
// Główna funkcja backendu
// ══════════════════════════════════════════════════════════

pub fn run(config: BlueConfig, _backend_type: BackendType) -> Result<()> {
    info!("Initializing DRM/KMS backend");

    let mut event_loop: EventLoop<BlueState> = EventLoop::try_new()?;
    let mut display:    Display<BlueState>   = Display::new()?;

    // ── 1. Sesja przez libseat ────────────────────────────
    let (mut session, notifier) = LibSeatSession::new()
    .map_err(|e| anyhow!("LibSeat failed: {e}\n  → seatd/logind required"))?;
    let seat_name = <LibSeatSession as Session>::seat(&session).to_string();
    info!("Seat: {}", seat_name);

    let (ses_tx, ses_rx) = channel::<()>();
    event_loop.handle()
    .insert_source(ses_rx, |_, _, _| {})
    .map_err(|e| anyhow!("session rx: {e}"))?;
    event_loop.handle()
    .insert_source(notifier, move |_ev, _, _| { ses_tx.send(()).ok(); })
    .map_err(|e| anyhow!("session notifier: {e}"))?;

    // ── 2. Wayland socket ─────────────────────────────────
    let listening_socket =
    ListeningSocket::bind_auto("wayland", 0usize..=255)
    .map_err(|e| anyhow!("Wayland socket: {e}"))?;
    let socket_name = listening_socket
    .socket_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_else(|| "wayland-0".to_string());
    std::env::set_var("WAYLAND_DISPLAY", &socket_name);
    info!("Wayland socket: {}", socket_name);

    // ── 3. UDev ───────────────────────────────────────────
    let udev = UdevBackend::new(&seat_name)
    .map_err(|e| anyhow!("UdevBackend: {e}"))?;

    let (msg_tx, msg_rx): (Sender<BackendMsg>, Channel<BackendMsg>) = {
        let (t, r) = channel::<BackendMsg>();
        (t, r)
    };

    // ── 4. libinput ───────────────────────────────────────
    let li_iface  = LibinputSessionInterface::from(session.clone());
    let mut li    = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(li_iface);
    li.udev_assign_seat(&seat_name)
    .map_err(|_| anyhow!("libinput assign seat failed"))?;
    let li_backend = LibinputInputBackend::new(li);

    event_loop.handle()
    .insert_source(li_backend, move |event, _, state: &mut BlueState| {
        let mut input_state = std::mem::take(&mut state.input_state);
        input_state.handle(event, state);
        state.input_state = input_state;
    })
    .map_err(|e| anyhow!("insert libinput: {e}"))?;

    // ── 5. Udev hotplug ───────────────────────────────────
    event_loop.handle()
    .insert_source(udev, move |_ev, _, _| { debug!("udev hotplug"); })
    .map_err(|e| anyhow!("insert udev: {e}"))?;

    // ── 6. Stan compositora ───────────────────────────────
    let loop_signal = event_loop.get_signal();
    let mut state = BlueState::new(
        &mut display,
        event_loop.handle(),
                                   loop_signal.clone(),
                                   config,
                                   BackendType::OpenGL,
    )?;

    // ── 7. DRM devices ────────────────────────────────────
    let mut output_devices: HashMap<DrmNode, OutputDevice> = HashMap::new();
    let mut renderer_opt:   Option<GlesRenderer>           = None;

    for path in glob_drm_cards() {
        match init_drm_device(&mut session, &path, &mut renderer_opt) {
            Ok((node, od)) => {
                info!("DRM {:?}: {}×{}", node, od.size.0, od.size.1);
                state.outputs.push(od.output.clone());
                output_devices.insert(node, od);
            }
            Err(e) => warn!("DRM init {:?}: {}", path, e),
        }
    }

    if output_devices.is_empty() {
        return Err(anyhow!(
            "No DRM output found. Check /dev/dri/card*, video group, seatd."
        ));
    }

    // ── 8. Spawn panel/appmenu/dock ───────────────────────
    launch_bundled(socket_name.clone());
    let _daemons = session::start_daemons();
    session::run_autostart();
    info!("Blue Environment ready — {} output(s)", output_devices.len());

    // ── 9. Render timer ~60Hz ─────────────────────────────
    std::thread::spawn(move || {
        let target = Duration::from_micros(16_667);
        loop {
            std::thread::sleep(target);
            if msg_tx.send(BackendMsg::RenderTick).is_err() { break; }
        }
    });
    event_loop.handle()
    .insert_source(msg_rx, |_, _, _| {})
    .map_err(|e| anyhow!("insert msg_rx: {e}"))?;

    // ── 10. Pętla główna ──────────────────────────────────
    let mut last_frame = Instant::now();
    let frame_budget   = Duration::from_millis(15);

    event_loop.run(
        Some(Duration::from_millis(4)),
                   &mut state,
                   |s| {
                       while let Ok(Some(client)) = listening_socket.accept() {
                           s.dh.insert_client(client, Arc::new(ClientState::default())).ok();
                       }
                       s.dh.flush_clients().ok();

                       let now = Instant::now();
                       if now.duration_since(last_frame) >= frame_budget {
                           last_frame = now;
                           let wallpaper_rgba = s.wallpaper.rgba();
                           if let Some(renderer) = renderer_opt.as_mut() {
                               for (_, od) in output_devices.iter_mut() {
                                   render_output(od, renderer, &wallpaper_rgba);
                               }
                           }
                       }
                   },
    )?;

    Ok(())
}

// ══════════════════════════════════════════════════════════
// Inicjalizacja jednego DRM device
// ══════════════════════════════════════════════════════════

fn init_drm_device(
    session:      &mut LibSeatSession,
    path:         &std::path::Path,
    renderer_out: &mut Option<GlesRenderer>,
) -> Result<(DrmNode, OutputDevice)> {
    let owned_fd: OwnedFd = session
    .open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK)
    .map_err(|e| anyhow!("session.open {:?}: {}", path, e))?;

    let drm_fd = DrmDeviceFd::new(owned_fd.into());
    let (drm, _drm_notifier) = DrmDevice::new(drm_fd.clone(), true)
    .map_err(|e| anyhow!("DrmDevice::new: {}", e))?;

    // DrmNode z path
    let node = {
        use std::os::unix::fs::MetadataExt;
        let dev_id = std::fs::metadata(path).map(|m| m.rdev()).unwrap_or(0);
        DrmNode::from_dev_id(dev_id)
        .unwrap_or_else(|_| DrmNode::from_dev_id(0).unwrap())
    };

    // GBM device
    let gbm = GbmDevice::new(drm_fd.clone())
    .map_err(|e| anyhow!("GbmDevice: {}", e))?;

    // EGL + GLES renderer (raz, dzielony)
    if renderer_out.is_none() {
        let egl_display = unsafe { EGLDisplay::new(gbm.clone()) }
        .map_err(|e| anyhow!("EGLDisplay: {}", e))?;
        let egl_ctx = EGLContext::new(&egl_display)
        .map_err(|e| anyhow!("EGLContext: {}", e))?;
        let renderer = unsafe { GlesRenderer::new(egl_ctx) }
        .map_err(|e| anyhow!("GlesRenderer: {}", e))?;
        info!("GLES renderer initialized");
        *renderer_out = Some(renderer);
    }

    // Connector + mode + CRTC
    let resources   = drm.resource_handles()
    .map_err(|e| anyhow!("resource_handles: {}", e))?;
    let conn_handle = find_connected_connector(&drm, &resources)
    .ok_or_else(|| anyhow!("No connected connector on {:?}", path))?;
    let conn_info   = drm.get_connector(conn_handle, false)
    .map_err(|e| anyhow!("get_connector: {}", e))?;

    let mode = best_mode(&conn_info)
    .ok_or_else(|| anyhow!("No mode for connector"))?;
    let (mw, mh) = (mode.size().0 as u32, mode.size().1 as u32);
    info!("Mode: {}×{}@{}Hz", mw, mh, mode.vrefresh());

    let crtc_h = find_crtc_for_connector(&drm, &resources, &conn_info)
    .ok_or_else(|| anyhow!("No CRTC for connector"))?;

    // Allocator
    let gbm_alloc = GbmAllocator::new(
        gbm.clone(),
                                      GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );

    // Exporter — GbmFramebufferExporter::new(gbm: GbmDevice<A>)
    let gbm_exporter = GbmFramebufferExporter::new(gbm.clone());

    // Formaty kolorów — Item = Fourcc (nie &Fourcc), więc iter().copied()
    let color_formats = [DrmFourcc::Xrgb8888, DrmFourcc::Argb8888];

    // Formaty render (dmabuf) — FormatSet::into_iter().cloned()
    let render_formats: Vec<_> = renderer_out.as_ref()
    .map(|r| r.dmabuf_formats().into_iter().cloned().collect())
    .unwrap_or_default();

    // DRM surface
    let drm_surface = drm
    .create_surface(crtc_h, mode, &[conn_handle])
    .map_err(|e| anyhow!("drm.create_surface: {}", e))?;

    // Rozmiar bufora
    let buffer_size = Size::<u32, Buffer>::from((mw, mh));

    // DrmCompositor::new(output, surface, planes, allocator, exporter,
    //                    color_formats, render_formats, buffer_size, dmabuf_feedback)
    let drm_compositor = DrmCompositor::new(
        &make_output(mw, mh, mode.vrefresh()),
                                            drm_surface,
                                            None,
                                            gbm_alloc,
                                            gbm_exporter,
                                            color_formats.into_iter(),
                                            render_formats.iter().cloned(),
                                            buffer_size,
                                            None,
    ).map_err(|e| anyhow!("DrmCompositor: {}", e))?;

    // Smithay Output
    let output = Output::new(
        format!("DRM-{:?}", node),
            PhysicalProperties {
                // conn_info.size() → Option<(u32,u32)> → cast do (i32,i32)
                size: conn_info.size().map(|(w,h)|(w as i32,h as i32)).unwrap_or((520,293)).into(),
                             subpixel: Subpixel::Unknown,
                             make:     "Blue".into(),
                             model:    "DRM".into(),
            },
    );
    let smithay_mode = Mode {
        size:    (mw as i32, mh as i32).into(),
        refresh: mode.vrefresh() as i32 * 1000,
    };
    output.change_current_state(
        Some(smithay_mode), Some(Transform::Normal), Some(Scale::Integer(1)), None,
    );
    output.set_preferred(smithay_mode);

    let damage_tracker = OutputDamageTracker::from_output(&output);

    Ok((node, OutputDevice {
        output,
        compositor:      drm_compositor,
        _damage_tracker: damage_tracker,
        size:            (mw, mh),
        wallpaper_tex:   None,
        wallpaper_dirty: true,
    }))
}

fn make_output(w: u32, h: u32, refresh_hz: u32) -> Output {
    let output = Output::new(
        "drm-tmp".into(),
                             PhysicalProperties {
                                 size:     (w as i32, h as i32).into(),
                             subpixel: Subpixel::Unknown,
                             make:     "Blue".into(),
                             model:    "DRM".into(),
                             },
    );
    let mode = Mode {
        size:    (w as i32, h as i32).into(),
        refresh: refresh_hz as i32 * 1000,
    };
    output.change_current_state(
        Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), None,
    );
    output.set_preferred(mode);
    output
}

// ══════════════════════════════════════════════════════════
// Render jednego outputu
// ══════════════════════════════════════════════════════════

fn render_output(
    od:        &mut OutputDevice,
    renderer:  &mut GlesRenderer,
    wallpaper: &Option<(Vec<u8>, u32, u32)>,
) {
    // Lazy upload tapety
    if od.wallpaper_dirty || od.wallpaper_tex.is_none() {
        if let Some((data, tw, th)) = wallpaper {
            let sz = Size::<i32, Buffer>::from((*tw as i32, *th as i32));
            match renderer.import_memory(data, DrmFourcc::Abgr8888, sz, false) {
                Ok(tex) => { od.wallpaper_tex = Some(tex); od.wallpaper_dirty = false; }
                Err(e)  => warn!("Wallpaper upload: {}", e),
            }
        }
    }

    // render_frame::<R, E>(renderer, elements, clear_color, frame_flags)
    //   R = GlesRenderer
    //   E = SolidColorRenderElement  (implements RenderElement<GlesRenderer>)
    //   clear_color: impl Into<Color32F> = [f32; 4]
    //   frame_flags: FrameFlags
    let clear: [f32; 4] = [0.051, 0.067, 0.090, 1.0];

    let elements: &[SolidColorRenderElement] = &[];
    let frame_res = od.compositor
    .render_frame::<GlesRenderer, SolidColorRenderElement>(
        renderer,
        elements,
        clear,
        FrameFlags::empty(),
    );

    match frame_res {
        Ok(frame) => {
            // RenderFrameResult: field `is_empty` (nie `damage`)
            if !frame.is_empty {
                // TODO etap 2: render tapety przez TextureRenderElement
            }
            // queue_frame(user_data: U) gdzie U = () — nie None
            if let Err(e) = od.compositor.queue_frame(()) {
                let s = format!("{}", e);
                if !s.contains("EBUSY") && !s.contains("FrameSkipped") {
                    warn!("queue_frame: {}", e);
                }
            }
        }
        Err(e) => debug!("render_frame: {}", e),
    }
}

// ══════════════════════════════════════════════════════════
// DRM helpers
// ══════════════════════════════════════════════════════════

fn find_connected_connector(
    drm:       &DrmDevice,
    resources: &smithay::reexports::drm::control::ResourceHandles,
) -> Option<connector::Handle> {
    resources.connectors().iter().find_map(|&c| {
        let info: smithay::reexports::drm::control::connector::Info =
        drm.get_connector(c, false).ok()?;
        if info.state() == connector::State::Connected { Some(c) } else { None }
    })
}

fn best_mode(
    conn_info: &smithay::reexports::drm::control::connector::Info,
) -> Option<smithay::reexports::drm::control::Mode> {
    let modes = conn_info.modes();
    if let Some(m) = modes.iter().find(|m| m.mode_type().contains(ModeTypeFlags::PREFERRED)) {
        return Some(*m);
    }
    modes.iter().max_by_key(|m| m.size().0 as u32 * m.size().1 as u32).copied()
}

fn find_crtc_for_connector(
    drm:       &DrmDevice,
    resources: &smithay::reexports::drm::control::ResourceHandles,
    conn_info: &smithay::reexports::drm::control::connector::Info,
) -> Option<crtc::Handle> {
    // 1. Aktualny encoder
    if let Some(enc_h) = conn_info.current_encoder() {
        if let Ok(enc) = drm.get_encoder(enc_h) {
            let enc: smithay::reexports::drm::control::encoder::Info = enc;
            if let Some(c) = enc.crtc() { return Some(c); }
        }
    }
    // 2. Wszystkie encodery — CrtcListFilter::contains(crtc_handle)
    for &enc_h in conn_info.encoders() {
        if let Ok(enc) = drm.get_encoder(enc_h) {
            let enc: smithay::reexports::drm::control::encoder::Info = enc;
            let possible = enc.possible_crtcs();
            for &crtc_h in resources.crtcs() {
                if possible.contains(crtc_h) {
                    return Some(crtc_h);
                }
            }
        }
    }
    // 3. Fallback
    resources.crtcs().first().copied()
}

// ══════════════════════════════════════════════════════════
// Filesystem + spawn helpers
// ══════════════════════════════════════════════════════════

fn glob_drm_cards() -> Vec<std::path::PathBuf> {
    let mut cards = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/dri") {
        for e in entries.filter_map(|e| e.ok()) {
            if e.file_name().to_string_lossy().starts_with("card") {
                cards.push(e.path());
            }
        }
    }
    cards.sort();
    cards
}

fn launch_bundled(wayland_display: String) {
    let exe_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    .unwrap_or_else(|| std::path::PathBuf::from("."));

    for &(bin, delay_ms) in &[("blue-panel", 300u64), ("blue-appmenu", 600), ("blue-dock", 700)] {
        let wd  = wayland_display.clone();
        let dir = exe_dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(delay_ms));
            let _ = std::process::Command::new(dir.join(bin))
            .env("WAYLAND_DISPLAY", &wd)
            .env("XDG_CURRENT_DESKTOP", "Blue:HackerOS")
            .spawn()
            .or_else(|_| std::process::Command::new(bin)
            .env("WAYLAND_DISPLAY", &wd)
            .env("XDG_CURRENT_DESKTOP", "Blue:HackerOS")
            .spawn())
            .map_err(|e| warn!("{} spawn failed: {}", bin, e));
        });
    }
}
