use smithay::backend::udev::UdevEvent;
use tracing::info;
use crate::{state::BlueState, ipc};

/// Handle a UdevEvent (device add/remove/change) — called from the udev source.
pub fn handle_udev_event(state: &mut BlueState, event: UdevEvent) {
    match event {
        UdevEvent::Added { device_id: _, path } => {
            info!("UdevEvent::Added {:?}", path);
        }
        UdevEvent::Changed { device_id: _ } => {
            info!("UdevEvent::Changed");
            rescan_outputs(state);
        }
        UdevEvent::Removed { device_id: _ } => {
            info!("UdevEvent::Removed");
        }
    }
}

/// Re-scan connector status and update the output list accordingly.
pub fn rescan_outputs(state: &mut BlueState) {
    // Enumerate currently attached outputs
    let current_names: Vec<String> = state.space.outputs().map(|o| o.name()).collect();

    // Ask the kernel what is connected right now via wlr-randr / xrandr
    let connected = list_connected_connectors();

    // Notify shell of changes
    let clients = state.clients.clone();
    for name in &connected {
        if !current_names.contains(name) {
            info!("Output connected: {}", name);
            ipc::broadcast(&clients, &ipc::CompositorMessage::OutputChanged {
                name: name.clone(), connected: true, width: 0, height: 0,
            });
        }
    }
    for name in &current_names {
        if !connected.contains(name) {
            info!("Output disconnected: {}", name);
            ipc::broadcast(&clients, &ipc::CompositorMessage::OutputChanged {
                name: name.clone(), connected: false, width: 0, height: 0,
            });
        }
    }
}

fn list_connected_connectors() -> Vec<String> {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg("wlr-randr 2>/dev/null | grep ' connected' | awk '{print $1}' || xrandr 2>/dev/null | grep ' connected' | awk '{print $1}'")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    out.lines().map(String::from).filter(|s| !s.is_empty()).collect()
}
