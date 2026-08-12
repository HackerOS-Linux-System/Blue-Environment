use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

fn log_file_path() -> PathBuf {
    let system_dir = PathBuf::from("/var/log/Blue-Environment");
    if std::fs::create_dir_all(&system_dir).is_ok() {
        let probe = system_dir.join(".write-test");
        if std::fs::write(&probe, b"").is_ok() {
            let _ = std::fs::remove_file(&probe);
            return system_dir;
        }
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let fallback = home.join(".cache/Blue-Environment/shell/logs");
    std::fs::create_dir_all(&fallback).ok();
    fallback
}

/// Call once, at the very top of `main()`, before anything else that
/// might log. Returns a guard that must be kept alive for the process's
/// whole lifetime — dropping it flushes and closes the non-blocking
/// writer, so binding it to `_` or letting it go out of scope early
/// silently truncates the log.
pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = log_file_path();
    let using_system_path = log_dir.starts_with("/var/log");

    let file_appender = tracing_appender::rolling::never(&log_dir, "blue-environment.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("blue_environment=info,tao=warn,wry=warn")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .finish();

    if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
        // Don't panic here the way the compositor does — a second Tauri
        // dev-mode reload or a test harness calling this twice
        // shouldn't take the whole shell down, just log-visible to
        // stderr since the real subscriber is already whatever
        // installed first.
        eprintln!("logging::init: a tracing subscriber was already installed ({e}); keeping it");
    }

    if using_system_path {
        tracing::info!("Logging to {}/blue-environment.log", log_dir.display());
    } else {
        tracing::warn!(
            "/var/log/Blue-Environment not writable by this user — logging to {}/blue-environment.log instead. \
             To use the system path: sudo install -d -m 0775 -o $USER -g $USER /var/log/Blue-Environment",
            log_dir.display()
        );
    }

    guard
}
