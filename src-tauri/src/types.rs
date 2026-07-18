use serde::{Serialize, Deserialize};

// ── Structs ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: String,
    pub mime_type: String,
    pub modified: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SystemStats {
    pub cpu: f32,
    pub ram: f32,
    pub battery: f32,
    pub is_charging: bool,
    pub volume: i32,
    pub brightness: i32,
    pub wifi_ssid: String,
    pub kernel: String,
    pub session_type: String,
    pub net_rx_mb: f32,
    pub net_tx_mb: f32,
    pub disk_read_mb: f32,
    pub disk_write_mb: f32,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProcessEntry {
    pub pid: String,
    pub name: String,
    pub cpu: f32,
    pub memory: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub in_use: bool,
    pub bssid: String,
    pub frequency: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BluetoothDevice {
    pub name: String,
    pub mac: String,
    pub device_type: String,
    pub connected: bool,
    pub paired: bool,
    pub trusted: bool,
    pub battery: Option<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AudioSink {
    pub id: u32,
    pub name: String,
    pub description: String,
    pub volume: f32,
    pub muted: bool,
    pub is_default: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct PowerProfile {
    pub name: String,
    pub active: bool,
    pub icon: Option<String>,
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ClipboardItem {
    pub id: String,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub message: String,
    pub app_id: Option<String>,
    pub timestamp: u64,
    pub read: bool,
    pub icon: Option<String>,
    pub actions: Option<Vec<Action>>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Action {
    pub label: String,
    pub action: String,
}
