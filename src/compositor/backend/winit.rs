use anyhow::{anyhow, Result};
use std::sync::Arc;
use smithay::{
    backend::{
        renderer::{Frame, Renderer},
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

    // smithay winit::init tworzy okno w bieżącej sesji (X11 lub Wayland)
    let (mut winit_backend, mut winit_loop) = winit::init::<smithay::backend::renderer::gles::GlesRenderer>()
    .map_err(|e| anyhow!("winit init failed: {}", e))?;

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

    let loop_signal = event_loop.get_signal();
    let mut state = BlueState::new(
        &mut display,
        event_loop.handle(),
                                   loop_signal,
                                   config,
                                   backend_type,
    )?;
    state.outputs.push(output);

    // Uruchom panel i app_menu podłączone do naszego socket
    {
        let wd = socket_name.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(500));
            std::env::set_var("WAYLAND_DISPLAY", &wd);
            crate::panel::launch(wd);
        });
    }
    {
        let wd = socket_name.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(600));
            std::env::set_var("WAYLAND_DISPLAY", &wd);
            crate::app_menu::launch(wd);
        });
    }

    info!("Blue Environment ready (dev mode)");

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
                               WinitEvent::Input(e) => {
                                   input_events.push(e);
                               }
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

                       // ── 4. Obsłuż input (split borrow przez raw ptr) ──
                       for event in input_events {
                           // SAFETY: input_state i pozostałe pola BlueState są rozłącznymi polami
                           let ptr: *mut crate::input::InputState = &mut state.input_state;
                           unsafe { (*ptr).handle(event, state); }
                       }

                       // ── 5. Render ─────────────────────────────────
                       if need_redraw || !egl_ready {
                           let w = current_size.w as i32;
                           let h = current_size.h as i32;
                           // smithay 0.6: Rectangle::new((x,y).into(), (w,h).into())
                           let damage = Rectangle::new(
                               (0, 0).into(),
                                                       Size::<i32, Physical>::from((w, h)),
                           );

                           // smithay 0.6: bind() trzyma &mut winit_backend przez cały czas życia
                           // Result<(&mut Renderer, Target)>. Jedyny sposób żeby potem wywołać
                           // submit() to użyć raw ptr — borrow checker nie widzi że render już skończył.
                           let render_result = do_render(&mut winit_backend, damage, w, h);
                           match render_result {
                               RenderResult::NotReady(_) => {
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
                                   // Nie submitujemy — błąd rysowania, ale surface jest OK
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
// Osobna funkcja gwarantuje że borrow z bind() kończy się przed powrotem,
// co pozwala wywołać submit() na winit_backend po jej zakończeniu.

enum RenderResult {
    NotReady(u32),   // EGL surface jeszcze nie gotowa, retry count
    Ok,              // render udany, można submitować
    RenderFailed,    // bind OK ale rysowanie się nie powiodło
}

fn do_render(
    backend: &mut smithay::backend::winit::WinitGraphicsBackend<
    smithay::backend::renderer::gles::GlesRenderer,
    >,
    damage:  smithay::utils::Rectangle<i32, smithay::utils::Physical>,
    w:       i32,
    h:       i32,
) -> RenderResult {
    use smithay::{
        backend::renderer::{Frame, Renderer},
        utils::{Size, Physical, Transform},
    };

    let output_size = Size::<i32, Physical>::from((w, h));

    // bind() trzyma borrow na `backend` przez życie zwróconego Result
    // — ta funkcja kończy się PRZED submit(), więc borrow jest zwolniony
    match backend.bind() {
        Err(_) => {
            // Zwróć aktualny retry count — caller go inkrementuje
            RenderResult::NotReady(0)
        }
        Ok((renderer, mut target)) => {
            let mut failed = false;
            match renderer.render(&mut target, output_size, Transform::Normal) {
                Ok(mut frame) => {
                    frame.clear(
                        [0.051f32, 0.067, 0.090, 1.0].into(),
                                &[damage],
                    ).ok();
                    // TODO: renderuj okna z workspaces
                    if let Err(e) = frame.finish() {
                        tracing::error!("Frame finish: {}", e);
                        failed = true;
                    }
                }
                Err(e) => {
                    tracing::warn!("Render error: {}", e);
                    failed = true;
                }
            }
            // (renderer, target) — drop tutaj, borrow kończy się przed return
            drop(target);
            if failed { RenderResult::RenderFailed } else { RenderResult::Ok }
        }
    }
}
