// BEDM — Blue Environment Display Manager
// Main daemon — manages greeter, sessions, PAM auth, VT switching
//
// Architecture:
//   bedm (daemon, runs as root)
//     ├── manages /run/bedm/bedm.sock  (IPC with greeter)
//     ├── calls PAM for authentication
//     ├── spawns user sessions (Wayland/X11)
//     └── handles VT switching

mod config;
mod ipc;
mod pam_auth;
mod session;
mod users;
mod vt;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

pub const BEDM_VERSION: &str = "1.0.0";
pub const SOCKET_PATH: &str = "/run/bedm/bedm.sock";
pub const CONFIG_PATH: &str = "/etc/bedm/bedm.toml";
pub const LOG_DIR: &str = "/var/log/bedm";
pub const RUN_DIR: &str = "/run/bedm";

#[derive(Debug, Clone)]
pub struct DaemonState {
    pub config: config::BedmConfig,
    pub active_session: Option<session::ActiveSession>,
    pub greeter_pid: Option<u32>,
}

#[tokio::main]
async fn main() {
    init_logging();
    info!("BEDM v{} starting", BEDM_VERSION);

    // Must run as root
    if unsafe { libc::getuid() } != 0 {
        eprintln!("BEDM must run as root (UID 0)");
        std::process::exit(1);
    }

    // Create runtime dirs
    setup_runtime_dirs();

    // Load configuration
    let config = config::load_config(CONFIG_PATH).unwrap_or_else(|e| {
        warn!("Config load error ({}), using defaults", e);
        config::BedmConfig::default()
    });
    info!("Config loaded: autologin={:?}", config.autologin_user);

    let state = Arc::new(Mutex::new(DaemonState {
        config: config.clone(),
        active_session: None,
        greeter_pid: None,
    }));

    // Handle SIGTERM / SIGCHLD
    setup_signals();

    // Handle autologin
    if let Some(ref user) = config.autologin_user {
        let user = user.clone();
        let session_type = config.autologin_session.clone()
            .unwrap_or_else(|| "blue-environment".to_string());
        info!("Autologin: {} -> {}", user, session_type);
        let state_clone = state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            session::launch_session(&state_clone, &user, &session_type, None).await;
        });
    } else {
        // Launch greeter
        let state_clone = state.clone();
        tokio::spawn(async move {
            launch_greeter(&state_clone).await;
        });
    }

    // Start IPC server
    ipc::run_server(state.clone()).await;
}

fn init_logging() {
    let _ = fs::create_dir_all(LOG_DIR);

    let file_appender = tracing_appender::rolling::daily(LOG_DIR, "bedm.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .init();

    // Also log to stderr for systemd journal
    eprintln!("[BEDM] Logging initialized");
}

fn setup_runtime_dirs() {
    for dir in &[RUN_DIR, "/run/bedm/sessions"] {
        fs::create_dir_all(dir).ok();
        fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).ok();
    }

    // Remove stale socket
    let _ = fs::remove_file(SOCKET_PATH);
}

fn setup_signals() {
    // We use tokio signal handling in ipc module
    // Register SIGCHLD to reap zombie processes
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_flags = libc::SA_NOCLDWAIT;
        sa.sa_sigaction = libc::SIG_DFL;
        libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());
    }
}

async fn launch_greeter(state: &Arc<Mutex<DaemonState>>) {
    let greeter_path = {
        let st = state.lock().await;
        st.config.greeter_path.clone()
            .unwrap_or_else(|| "/usr/bin/bedm-greeter".to_string())
    };

    info!("Launching greeter: {}", greeter_path);

    // The greeter runs on VT1 or whichever VT is configured
    let vt = {
        let st = state.lock().await;
        st.config.vt.unwrap_or(1)
    };

    if let Err(e) = vt::switch_to(vt) {
        warn!("VT switch failed: {} — continuing", e);
    }

    // Launch greeter process
    match tokio::process::Command::new(&greeter_path)
        .env("BEDM_SOCKET", SOCKET_PATH)
        .env("XDG_SESSION_TYPE", "wayland")
        .spawn()
    {
        Ok(mut child) => {
            let pid = child.id().unwrap_or(0);
            info!("Greeter PID: {}", pid);
            state.lock().await.greeter_pid = Some(pid);
            let _ = child.wait().await;
            info!("Greeter exited");
            state.lock().await.greeter_pid = None;
        }
        Err(e) => {
            error!("Failed to launch greeter: {}", e);
        }
    }
}
