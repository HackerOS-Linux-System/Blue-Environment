pub mod udev;
pub mod winit;

use tracing::{info, warn};
use crate::config::{BlueConfig, RenderBackend};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendType {
    Vulkan,
    OpenGL,
}

pub fn detect(config: &BlueConfig) -> BackendType {
    match config.render_backend {
        RenderBackend::Vulkan => { info!("Backend: Vulkan (from config)"); BackendType::Vulkan }
        RenderBackend::OpenGL => { info!("Backend: OpenGL (from config)"); BackendType::OpenGL }
        RenderBackend::Auto   => {
            if vulkan_available() {
                info!("Backend auto: Vulkan");
                BackendType::Vulkan
            } else {
                warn!("Backend auto: OpenGL (Vulkan not found)");
                BackendType::OpenGL
            }
        }
    }
}

fn vulkan_available() -> bool {
    let loader_ok = [
        "/usr/lib/libvulkan.so.1",
        "/usr/lib/libvulkan.so",
        "/usr/lib64/libvulkan.so.1",
        "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
        "/usr/lib/aarch64-linux-gnu/libvulkan.so.1",
    ]
    .iter()
    .any(|p| std::path::Path::new(p).exists());

    if !loader_ok {
        return false;
    }

    [
        "/usr/share/vulkan/icd.d/",
        "/etc/vulkan/icd.d/",
        "/usr/local/share/vulkan/icd.d/",
    ]
    .iter()
    .any(|dir| {
        std::fs::read_dir(dir)
        .map(|e| e.flatten().any(|f| f.path().extension().map_or(false, |x| x == "json")))
        .unwrap_or(false)
    })
}
