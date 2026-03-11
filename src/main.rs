pub mod compositor;
pub mod panel;
pub mod app_menu;
pub mod session;
pub mod config;
pub mod ipc;
pub mod wallpaper;
pub mod workspace;
pub mod input;
pub mod notification;
pub mod csd;

use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
    .with_env_filter(
        EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info")),
    )
    .without_time()
    .init();

    info!("Blue Environment v{}", env!("CARGO_PKG_VERSION"));

    // ── Flaga --dev ────────────────────────────────────────
    // Z --dev: uruchamia się jako aplikacja w bieżącej sesji (winit/nested)
    // Bez --dev: uruchamia się jako pełne środowisko na TTY (DRM/KMS)
    let dev_mode = std::env::args().any(|a| a == "--dev");

    if dev_mode {
        info!("Tryb deweloperski (--dev): uruchamiam jako aplikację w bieżącej sesji");
    } else {
        info!("Tryb produkcyjny: uruchamiam jako pełne środowisko na TTY");
        // W trybie TTY upewniamy się że NIE ma aktywnego serwera Wayland/X11
        if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
            eprintln!(
                "BŁĄD: Wykryto aktywną sesję graficzną (WAYLAND_DISPLAY/DISPLAY).\n\
Aby uruchomić Blue Environment jako główne DE, wyloguj się i uruchom\n\
z TTY. Jeśli chcesz uruchomić jako aplikację, użyj flagi --dev."
            );
            std::process::exit(1);
        }
    }

    let cfg = config::BlueConfig::load_or_default();
    session::setup_environment();
    session::ensure_dbus();

    let backend = compositor::backend::detect(&cfg);
    info!("Backend: {:?}", backend);

    if let Err(e) = compositor::run(cfg, backend, dev_mode) {
        error!("Compositor error: {}", e);
    }

    info!("Sesja zakończona");
    Ok(())
}
