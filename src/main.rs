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
pub mod csd;          // ← nowy moduł dekoracji okien

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

    let cfg = config::BlueConfig::load_or_default();
    session::setup_environment();
    session::ensure_dbus();

    let backend = compositor::backend::detect(&cfg);
    info!("Backend: {:?}", backend);

    if let Err(e) = compositor::run(cfg, backend) {
        error!("Compositor error: {}", e);
    }

    info!("Sesja zakończona");
    Ok(())
}
