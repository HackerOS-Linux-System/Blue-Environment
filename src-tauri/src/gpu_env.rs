use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Virtual,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpuInfo {
    pub vendor: Vendor,
    /// Kernel driver bound to the card (`nvidia`, `nouveau`, `amdgpu`, `i915`, `virtio-pci` …).
    pub driver: String,
}

/// PCI vendor id → vendor.
pub fn vendor_from_pci_id(id: &str) -> Vendor {
    match id.trim().trim_start_matches("0x").to_ascii_lowercase().as_str() {
        "10de" => Vendor::Nvidia,
        "1002" | "1022" => Vendor::Amd,
        "8086" => Vendor::Intel,
        // virtio, VMware, VirtualBox, QEMU/Bochs, Hyper-V
        "1af4" | "15ad" | "80ee" | "1234" | "1414" => Vendor::Virtual,
        _ => Vendor::Other,
    }
}

/// Reads every `/sys/class/drm/cardN` (skipping connectors like `card0-HDMI-A-1`).
pub fn detect_gpus(sys_class_drm: &Path) -> Vec<GpuInfo> {
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(sys_class_drm) else { return out };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("card") && n[4..].chars().all(|c| c.is_ascii_digit()))
        .collect();
    names.sort();
    for n in names {
        let dev = sys_class_drm.join(&n).join("device");
        let Ok(vendor_id) = fs::read_to_string(dev.join("vendor")) else { continue };
        let driver = fs::read_link(dev.join("driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
            .unwrap_or_default();
        out.push(GpuInfo { vendor: vendor_from_pci_id(&vendor_id), driver });
    }
    out
}

/// Environment variables to set for the detected hardware. Pure: the caller decides what to apply.
pub fn flags_for(gpus: &[GpuInfo], nvidia_modeset: bool) -> Vec<(&'static str, &'static str)> {
    let proprietary_nvidia = gpus.iter().any(|g| g.vendor == Vendor::Nvidia && g.driver == "nvidia");
    if proprietary_nvidia {
        return if nvidia_modeset {
            vec![("WEBKIT_DISABLE_DMABUF_RENDERER", "1"), ("WEBKIT_FORCE_COMPOSITING_MODE", "1")]
        } else {
            vec![("WEBKIT_DISABLE_DMABUF_RENDERER", "1"), ("WEBKIT_DISABLE_COMPOSITING_MODE", "1")]
        };
    }
    // A real (non-virtual) GPU with an open driver is fine; only virtual-only setups need help.
    let only_virtual = !gpus.is_empty() && gpus.iter().all(|g| g.vendor == Vendor::Virtual);
    let no_gpu = gpus.is_empty();
    if only_virtual || no_gpu {
        return vec![("WEBKIT_DISABLE_DMABUF_RENDERER", "1")];
    }
    Vec::new()
}

fn nvidia_modeset_enabled() -> bool {
    fs::read_to_string("/sys/module/nvidia_drm/parameters/modeset")
        .map(|s| s.trim().eq_ignore_ascii_case("y"))
        .unwrap_or(false)
}

/// Detects the GPU and exports the WebKit workaround variables. Returns what was set (for logging).
pub fn apply() -> Vec<(String, String)> {
    let mode = std::env::var("BLUE_GPU_WORKAROUNDS").unwrap_or_default().to_ascii_lowercase();
    if mode == "off" || mode == "0" {
        return Vec::new();
    }
    let gpus = detect_gpus(Path::new("/sys/class/drm"));
    let mut applied = Vec::new();
    for (k, v) in flags_for(&gpus, nvidia_modeset_enabled()) {
        // Respect anything the user (or a session script) already decided.
        if std::env::var_os(k).is_none() {
            std::env::set_var(k, v);
            applied.push((k.to_string(), v.to_string()));
        }
    }
    if mode == "verbose" || !applied.is_empty() {
        eprintln!("[blue-gpu] detected {gpus:?}; applied {applied:?}");
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpu(v: Vendor, d: &str) -> GpuInfo {
        GpuInfo { vendor: v, driver: d.into() }
    }

    #[test]
    fn pci_vendor_ids() {
        assert_eq!(vendor_from_pci_id("0x10de\n"), Vendor::Nvidia);
        assert_eq!(vendor_from_pci_id("0x1002"), Vendor::Amd);
        assert_eq!(vendor_from_pci_id("0x8086"), Vendor::Intel);
        assert_eq!(vendor_from_pci_id("0x1af4"), Vendor::Virtual);
        assert_eq!(vendor_from_pci_id("0xabcd"), Vendor::Other);
    }

    #[test]
    fn proprietary_nvidia_with_modeset() {
        let f = flags_for(&[gpu(Vendor::Nvidia, "nvidia")], true);
        assert!(f.contains(&("WEBKIT_DISABLE_DMABUF_RENDERER", "1")));
        assert!(f.contains(&("WEBKIT_FORCE_COMPOSITING_MODE", "1")));
    }

    #[test]
    fn proprietary_nvidia_without_modeset_uses_software_compositing() {
        let f = flags_for(&[gpu(Vendor::Intel, "i915"), gpu(Vendor::Nvidia, "nvidia")], false);
        assert!(f.contains(&("WEBKIT_DISABLE_COMPOSITING_MODE", "1")));
        assert!(!f.iter().any(|(k, _)| *k == "WEBKIT_FORCE_COMPOSITING_MODE"));
    }

    #[test]
    fn open_drivers_need_nothing() {
        assert!(flags_for(&[gpu(Vendor::Nvidia, "nouveau")], false).is_empty());
        assert!(flags_for(&[gpu(Vendor::Amd, "amdgpu")], true).is_empty());
        assert!(flags_for(&[gpu(Vendor::Intel, "i915")], false).is_empty());
    }

    #[test]
    fn virtual_gpus_disable_dmabuf() {
        assert_eq!(flags_for(&[gpu(Vendor::Virtual, "virtio-pci")], false), vec![("WEBKIT_DISABLE_DMABUF_RENDERER", "1")]);
    }
}
