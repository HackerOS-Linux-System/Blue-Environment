use serde::Serialize;
use std::process::{Command, Stdio};
use std::time::Instant;
use tauri::{AppHandle, Emitter};

// ── Runtime detection (Wine / Proton / umu) ─────────────────────────────

#[derive(Serialize, Clone, Debug, Default)]
pub struct RuntimeStatus {
    pub wine_available: bool,
    pub wine_version: Option<String>,
    /// Standalone Proton binary found directly on PATH (rare outside a
    /// Steam install, but some distros package one).
    pub proton_available: bool,
    pub proton_path: Option<String>,
    /// Any Proton version installed under a real Steam library
    /// (`steamapps/common/Proton*`) — the common case for anyone who has
    /// ever installed a Proton-compatible game through Steam, even if
    /// they never use Steam to launch this one.
    pub steam_proton_versions: Vec<SteamProtonInstall>,
    /// `umu-run` — the modern, Steam-independent Proton launcher used by
    /// Lutris/Heroic and friends. Preferred over a raw Proton binary
    /// when available, since it correctly sets up the compatibility
    /// environment without needing a real Steam install at all.
    pub umu_available: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct SteamProtonInstall {
    pub name: String,
    pub path: String,
}

fn which(bin: &str) -> Option<String> {
    Command::new("which").arg(bin).output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn find_steam_proton_installs() -> Vec<SteamProtonInstall> {
    let Some(home) = dirs::home_dir() else { return vec![] };
    let candidates = [
        home.join(".steam/steam/steamapps/common"),
        home.join(".local/share/Steam/steamapps/common"),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common"),
    ];
    let mut found = vec![];
    for dir in candidates {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("Proton") {
                let bin = entry.path().join("proton");
                if bin.exists() {
                    found.push(SteamProtonInstall { name, path: bin.to_string_lossy().to_string() });
                }
            }
        }
    }
    found
}

/// This is genuinely worth checking every time rather than caching: Wine
/// and Proton are both things a user installs/uninstalls/updates
/// entirely outside Blue Play, so a stale "yes it's available" from an
/// earlier session would just produce a confusing launch failure later.
#[tauri::command]
pub async fn bpg_detect_runtimes() -> RuntimeStatus {
    let wine_version = Command::new("wine").arg("--version").output().ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    let proton_path = which("proton");
    let steam_proton_versions = find_steam_proton_installs();

    RuntimeStatus {
        wine_available: wine_version.is_some(),
        wine_version,
        proton_available: proton_path.is_some(),
        proton_path,
        steam_proton_versions,
        umu_available: which("umu-run").is_some(),
    }
}

// ── Launching + background playtime tracking ────────────────────────────
//
// "Runs in the background like Google Play Games" is scoped honestly
// here to what's actually deliverable: once a game is launched, Blue
// Play doesn't need to stay open or focused to keep tracking it — this
// spawns the process and hands off to a detached background thread that
// just waits on it, so playtime keeps accumulating correctly regardless
// of what the user does with the Blue Play window (minimize it, switch
// to another app, close the window entirely — the launched game and the
// tracking thread are independent of the UI). When the process exits,
// a `blue-play://game-exited` event carries the session's playtime back
// to whichever Blue Play window is listening, updating the library
// without the user having to do anything. This is NOT a persistent
// system service surviving a full Blue Play/session restart — that
// would need a proper daemon (like BEDM), which is a real, separate
// follow-up if always-on tracking across restarts turns out to matter.

#[derive(Serialize, Clone, Debug)]
pub struct LaunchResult {
    pub launched: bool,
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
pub struct GameExitedPayload {
    pub game_id: String,
    pub playtime_seconds: u64,
    pub exit_success: bool,
}

fn spawn_and_track(app: AppHandle, game_id: String, cmd: String, args: Vec<String>, envs: Vec<(String, String)>) -> LaunchResult {
    let mut command = Command::new(&cmd);
    command.args(&args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    for (k, v) in envs {
        command.env(k, v);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => return LaunchResult { launched: false, error: Some(format!("Failed to launch '{cmd}': {e}")) },
    };

    std::thread::spawn(move || {
        let start = Instant::now();
        let status = child.wait();
        let playtime_seconds = start.elapsed().as_secs();
        let exit_success = status.map(|s| s.success()).unwrap_or(false);
        let _ = app.emit("blue-play://game-exited", GameExitedPayload { game_id, playtime_seconds, exit_success });
    });

    LaunchResult { launched: true, error: None }
}

/// Launches a native Linux game binary directly — no runtime layer
/// needed, it's already an ELF the kernel can run.
#[tauri::command]
pub async fn bpg_launch_native(app: AppHandle, game_id: String, exec_path: String, args: Vec<String>) -> LaunchResult {
    if !std::path::Path::new(&exec_path).exists() {
        return LaunchResult { launched: false, error: Some(format!("'{exec_path}' does not exist")) };
    }
    spawn_and_track(app, game_id, exec_path, args, vec![])
}

/// Launches a Windows `.exe` through Wine, Proton, or umu-run — whichever
/// `runtime` the caller picked from what `bpg_detect_runtimes()` reported
/// as actually available. Each game gets its own persistent prefix
/// (`~/.local/share/blue-play/prefixes/<game_id>/`) so save games and
/// per-title Windows registry state don't bleed between different games,
/// the same way Lutris/Heroic isolate each title.
#[tauri::command]
pub async fn bpg_launch_windows(app: AppHandle, game_id: String, exe_path: String, runtime: String, runtime_path: Option<String>) -> LaunchResult {
    if !std::path::Path::new(&exe_path).exists() {
        return LaunchResult { launched: false, error: Some(format!("'{exe_path}' does not exist")) };
    }
    let Some(home) = dirs::home_dir() else {
        return LaunchResult { launched: false, error: Some("Could not determine home directory".into()) };
    };
    let prefix = home.join(".local/share/blue-play/prefixes").join(&game_id);
    let _ = std::fs::create_dir_all(&prefix);

    let (cmd, cmd_args, envs): (String, Vec<String>, Vec<(String, String)>) = match runtime.as_str() {
        "wine" => (
            "wine".to_string(), vec![exe_path.clone()],
            vec![("WINEPREFIX".to_string(), prefix.to_string_lossy().to_string())],
        ),
        "umu" => (
            "umu-run".to_string(), vec![exe_path.clone()],
            vec![
                ("WINEPREFIX".to_string(), prefix.to_string_lossy().to_string()),
                ("GAMEID".to_string(), format!("blueplay-{game_id}")),
            ],
        ),
        "proton" => {
            let Some(proton) = runtime_path else {
                return LaunchResult { launched: false, error: Some("No Proton path given".into()) };
            };
            // Proton needs both compat vars set even for a bare `run` —
            // STEAM_COMPAT_CLIENT_INSTALL_PATH just needs *a* Steam-shaped
            // directory to exist; it doesn't have to be a real, currently
            // installed Steam client for `proton run` on an arbitrary exe.
            let steam_install = home.join(".steam/steam");
            (
                proton, vec!["run".to_string(), exe_path.clone()],
                vec![
                    ("STEAM_COMPAT_DATA_PATH".to_string(), prefix.to_string_lossy().to_string()),
                    ("STEAM_COMPAT_CLIENT_INSTALL_PATH".to_string(), steam_install.to_string_lossy().to_string()),
                ],
            )
        }
        other => return LaunchResult { launched: false, error: Some(format!("Unknown runtime: {other}")) },
    };

    spawn_and_track(app, game_id, cmd, cmd_args, envs)
}
