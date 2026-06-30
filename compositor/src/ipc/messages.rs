use serde::{Deserialize, Serialize};
use crate::state::WindowInfo;

/// Messages FROM compositor TO shell (Tauri frontend).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CompositorMessage {
    Ready             { socket: String },
    WindowList        { windows: Vec<WindowInfo> },
    WindowFocused     { id: u64 },
    WindowOpened      { window: WindowInfo },
    WindowClosed      { id: u64 },
    WindowUpdated     { window: WindowInfo },
    ToggleStartMenu,
    ToggleFullscreenMenu,
    WorkspaceSwitched { index: usize, count: usize },
    SwitcherState     { visible: bool, index: usize },
    ScreenLocked      { locked: bool },
    OutputChanged     { name: String, connected: bool, width: u32, height: u32 },
    IdleChanged       { idle: bool },
    ScreenshotReady   { path: String },
    Error             { message: String },
}

/// Messages FROM shell TO compositor.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ShellMessage {
    FocusWindow            { id: u64 },
    CloseWindow            { id: u64 },
    KillWindow             { id: u64 },
    SwitchWorkspace        { index: usize },
    MoveWindowToWorkspace  { id: u64, workspace: usize },
    ToggleMaximize         { id: u64 },
    MinimizeWindow         { id: u64 },
    RestoreWindow          { id: u64 },
    SetFullscreen          { id: u64, fullscreen: bool },
    TileWindow             { id: u64, position: String },
    GetWindowList,
    SetWorkspaceCount      { count: usize },
    SetDpmsTimeout         { seconds: u64 },
    LockScreen,
    TakeScreenshot         { path: String, mode: String },
    SetKeyboardLayout      { layout: String, variant: Option<String> },
    SetCursor              { theme: String, size: u32 },
    ReloadConfig,
}
