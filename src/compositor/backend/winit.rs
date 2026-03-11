use anyhow::{anyhow, Result};
use std::sync::Arc;
use smithay::{
    backend::{
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::EventLoop,
        wayland_server::{Display, ListeningSocket},
    },
    utils::{Rectangle, Size, Transform, Physical},
};
use tracing::{error, info, warn};

use crate::{compositor::{BlueState, ClientState}, config::BlueConfig};
use super::BackendType;

pub fn run(config: BlueConfig, backend_type: BackendType) -> Result<()> {
    info!("Initializing Winit backend (--dev / nested mode)");

    let mut event_loop: EventLoop<BlueState> = EventLoop::try_new()?;
    let mut display: Display<BlueState>      = Display::new()?;

    let (mut winit_backend, mut winit_loop) =
    winit::init::<smithay::backend::renderer::gles::GlesRenderer>()
    .map_err(|e| anyhow!("winit init failed: {}", e))?;

    // Ustaw tytuł okna
    winit_backend.window().set_title("Blue Environment [dev]");

    let window_size = winit_backend.window_size();
    let mode = Mode {
        size:    (window_size.w as i32, window_size.h as i32).into(),
        refresh: 60_000,
    };

    let output = Output::new(
        "Blue-Dev".to_string(),
                             PhysicalProperties {
                                 size:     (0, 0).into(),
                             subpixel: Subpixel::Unknown,
                             make:     "Blue Environment".to_string(),
                             model:    "Dev Window".to_string(),
                             },
    );
    output.change_current_state(
        Some(mode), Some(Transform::Normal), Some(Scale::Integer(1)), None,
    );
    output.set_preferred(mode);

    // ── Wayland socket dla zarządzanych klientów ──────────
    let listening_socket =
    ListeningSocket::bind_auto("wayland", 0usize..=255)
    .map_err(|e| anyhow!("Failed to create Wayland socket: {e}"))?;

    let socket_name = listening_socket
    .socket_name()
    .map(|n| n.to_string_lossy().to_string())
    .unwrap_or_else(|| "wayland-1".to_string());

    info!("Blue compositor socket: {}", socket_name);

    // Zapamiętaj display sesji nadrzędnej (KDE/GNOME/etc.) przed podmianą
    // Panel i app_menu będą otwierać okna w sesji nadrzędnej, nie w naszym compositorze
    // (w --dev mode to prawidłowe zachowanie — panel pływa obok okna compositor)
    let parent_wayland = std::env::var("WAYLAND_DISPLAY")
    .unwrap_or_else(|_| "wayland-0".to_string());
    let parent_display = std::env::var("DISPLAY").ok();
    info!("Parent session: WAYLAND_DISPLAY={}", parent_wayland);

    let loop_signal = event_loop.get_signal();
    let mut state = BlueState::new(
        &mut display,
        event_loop.handle(),
                                   loop_signal,
                                   config,
                                   backend_type,
    )?;
    state.outputs.push(output);

    // ── Spawn panel i app_menu ────────────────────────────
    // Próbujemy najpierw jako osobny proces (blue-panel / blue-appmenu).
    // Jeśli binarka nie istnieje, fallback do wątku — wtedy jednak tylko
    // JEDEN z nich może wywołać slint::run_event_loop(), drugi musi używać
    // invoke_from_event_loop. Na razie: panel jako wątek (nie wywołuje run()),
    // app_menu jako wątek z run_event_loop().
    let exe_dir = std::env::current_exe()
    .ok()
    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    .unwrap_or_default();

    // ── Spawn panel i app_menu ────────────────────────────
    // W trybie --dev: panel i app_menu używają sesji NADRZĘDNEJ (KDE/GNOME)
    // jako swojego WAYLAND_DISPLAY. Dzięki temu ich okna są widoczne obok
    // okna compositor zamiast renderować się wewnątrz niego (co wymagałoby
    // pełnego surface compositing — planowane w następnym etapie).

    // PANEL
    {
        let panel_bin = exe_dir.join("blue-panel");
        let pwd = parent_wayland.clone();
        let pd  = parent_display.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(400));
            if panel_bin.exists() {
                info!("Spawning blue-panel (on parent session: {})", pwd);
                let mut cmd = std::process::Command::new(&panel_bin);
                cmd.env("WAYLAND_DISPLAY", &pwd);
                if let Some(d) = pd { cmd.env("DISPLAY", d); }
                if let Err(e) = cmd.spawn() {
                    warn!("blue-panel spawn failed: {}", e);
                }
            } else {
                warn!("blue-panel binary not found at {:?}", panel_bin);
                std::env::set_var("WAYLAND_DISPLAY", &pwd);
                if let Err(e) = crate::panel::run_panel() {
                    error!("Panel error: {}", e);
                }
            }
        });
    }

    // APP MENU
    {
        let appmenu_bin = exe_dir.join("blue-appmenu");
        let pwd = parent_wayland.clone();
        let pd  = parent_display.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            if appmenu_bin.exists() {
                info!("Spawning blue-appmenu (on parent session: {})", pwd);
                let mut cmd = std::process::Command::new(&appmenu_bin);
                cmd.env("WAYLAND_DISPLAY", &pwd);
                if let Some(d) = pd { cmd.env("DISPLAY", d); }
                if let Err(e) = cmd.spawn() {
                    warn!("blue-appmenu spawn failed: {}", e);
                }
            } else {
                warn!("blue-appmenu binary not found, running in-process");
                std::env::set_var("WAYLAND_DISPLAY", &pwd);
                crate::app_menu::run();
            }
        });
    }

    info!("Blue Environment ready (dev mode)");

    // Wczytaj dane tapety do CPU — będzie uploadowana do GPU przy pierwszym render
    let wallpaper_rgba = state.wallpaper.rgba();
    // Option<GlesTexture> uploadowana lazily przy pierwszym bind()
    let mut wallpaper_texture: Option<smithay::backend::renderer::gles::GlesTexture> = None;

    let mut egl_ready       = false;
    let mut egl_retry_count = 0u32;
    let mut need_redraw     = true;
    let mut current_size    = window_size;

    event_loop.run(
        Some(std::time::Duration::from_millis(16)),
                   &mut state,
                   |state| {
                       // ── 1. Akceptuj klientów Wayland ──────────────
                       if let Ok(Some(client)) = listening_socket.accept() {
                           state.dh.insert_client(client, Arc::new(ClientState::default())).ok();
                       }

                       // ── 2. Zbierz eventy winit ────────────────────
                       let mut input_events = Vec::new();
                       let mut new_size = None;

                       winit_loop.dispatch_new_events(|event| {
                           match event {
                               WinitEvent::Input(e)             => { input_events.push(e); }
                               WinitEvent::Resized { size, .. } => {
                                   new_size    = Some(size);
                                   need_redraw = true;
                                   egl_ready   = false;
                               }
                               WinitEvent::Redraw   => { need_redraw = true; }
                               WinitEvent::Focus(_) => { need_redraw = true; }
                               _ => {}
                           }
                       });

                       // ── 3. Obsłuż zmianę rozmiaru ─────────────────
                       if let Some(size) = new_size {
                           current_size = size;
                           let new_mode = Mode {
                               size:    (size.w as i32, size.h as i32).into(),
                   refresh: 60_000,
                           };
                           if let Some(output) = state.outputs.first() {
                               output.change_current_state(Some(new_mode), None, None, None);
                           }
                       }

                       // ── 4. Obsłuż input (mem::take — bez aliasingu) ──
                       if !input_events.is_empty() {
                           let mut input_state = std::mem::take(&mut state.input_state);
                           for event in input_events {
                               input_state.handle(event, state);
                           }
                           state.input_state = input_state;
                       }

                       // ── 5. Render ─────────────────────────────────
                       if need_redraw || !egl_ready {
                           let w = current_size.w as i32;
                           let h = current_size.h as i32;
                           let damage = Rectangle::new(
                               (0, 0).into(),
                                                       Size::<i32, Physical>::from((w, h)),
                           );

                           let render_result = do_render(
                               &mut winit_backend, damage, w, h,
                               &wallpaper_rgba, &mut wallpaper_texture,
                           );
                           match render_result {
                               RenderResult::NotReady => {
                                   egl_retry_count += 1;
                                   if egl_retry_count <= 3 || egl_retry_count % 60 == 0 {
                                       warn!("EGL bind retry #{}", egl_retry_count);
                                   }
                                   return;
                               }
                               RenderResult::Ok => {
                                   egl_ready       = true;
                                   egl_retry_count = 0;
                                   need_redraw     = false;
                                   if let Err(e) = winit_backend.submit(Some(&[damage])) {
                                       warn!("EGL submit error: {} — resetting surface", e);
                                       egl_ready   = false;
                                       need_redraw = true;
                                   }
                               }
                               RenderResult::RenderFailed => {
                                   egl_ready       = true;
                                   egl_retry_count = 0;
                                   need_redraw     = false;
                               }
                           }
                       }

                       // ── 6. Flush klientów ─────────────────────────
                       state.dh.flush_clients().ok();
                   },
    )?;

    Ok(())
}

// ── Render helper ──────────────────────────────────────────────────────────

enum RenderResult { NotReady, Ok, RenderFailed }

fn do_render(
    backend:  &mut smithay::backend::winit::WinitGraphicsBackend<
    smithay::backend::renderer::gles::GlesRenderer,
    >,
    damage:   smithay::utils::Rectangle<i32, smithay::utils::Physical>,
    w:        i32,
    h:        i32,
    wallpaper: &Option<(Vec<u8>, u32, u32)>,
             texture:   &mut Option<smithay::backend::renderer::gles::GlesTexture>,
) -> RenderResult {
    use smithay::{
        backend::renderer::{
            gles::GlesRenderer,
            Frame, Renderer, ImportMem, Texture,
        },
        reexports::drm::buffer::DrmFourcc,
        utils::{Size, Physical, Buffer, Transform, Rectangle, Point},
    };

    let output_size = Size::<i32, Physical>::from((w, h));

    match backend.bind() {
        Err(_) => RenderResult::NotReady,
        Ok((renderer, mut target)) => {
            // Upload tapety jako tekstury przy pierwszym render (lazy)
            if texture.is_none() {
                if let Some((data, tw, th)) = wallpaper {
                    // import_memory(data, format, size: Size<i32,Buffer>, flipped)
                    let size = Size::<i32, Buffer>::from((*tw as i32, *th as i32));
                    match renderer.import_memory(data, DrmFourcc::Abgr8888, size, false) {
                        Ok(tex) => {
                            *texture = Some(tex);
                        }
                        Err(e) => warn!("Wallpaper texture upload failed: {}", e),
                    }
                }
            }

            let mut failed = false;
            match renderer.render(&mut target, output_size, Transform::Normal) {
                Ok(mut frame) => {
                    if let Some(tex) = texture.as_ref() {
                        // Renderuj tapetę jako tło (stretch do rozmiaru okna)
                        // src: Rectangle<f64, Buffer> — obszar tekstury (UV)
                        let src = Rectangle::<f64, smithay::utils::Buffer>::new(
                            smithay::utils::Point::from((0.0f64, 0.0f64)),
                                                                                smithay::utils::Size::from((
                                                                                    tex.width()  as f64,
                                                                                                            tex.height() as f64,
                                                                                )),
                        );
                        // dst: Rectangle<i32, Physical> — gdzie rysować na ekranie
                        let dst = Rectangle::<i32, Physical>::new(
                            smithay::utils::Point::from((0i32, 0i32)),
                                                                  output_size,
                        );
                        frame.render_texture_from_to(
                            tex,
                            src,
                            dst,
                            &[damage],          // damage
                            &[damage],          // clip_damage
                            Transform::Normal,
                            1.0f32,
                            None,               // Option<&GlesTexProgram>
                            &[],                // &[Uniform<'_>]
                        ).unwrap_or_else(|e| warn!("Wallpaper render: {}", e));
                    } else {
                        // Fallback: solid color tło #0d1117
                        frame.clear(
                            [0.051f32, 0.067, 0.090, 1.0].into(),
                                    &[damage],
                        ).ok();
                    }
                    // TODO: renderuj powierzchnie klientów Wayland
                    if let Err(e) = frame.finish() {
                        error!("Frame finish: {}", e);
                        failed = true;
                    }
                }
                Err(e) => { warn!("Render error: {}", e); failed = true; }
            }
            drop(target);
            if failed { RenderResult::RenderFailed } else { RenderResult::Ok }
        }
    }
}
