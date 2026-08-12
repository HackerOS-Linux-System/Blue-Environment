use crate::{ai, packages};

#[tauri::command]
pub async fn get_dnf_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_dnf_packages).await.unwrap_or_default()
}

#[tauri::command]
pub async fn get_flatpak_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_flatpak_packages).await.unwrap_or_default()
}

#[tauri::command]
pub async fn get_appimage_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(packages::get_appimage_packages).await.unwrap_or_default()
}

#[tauri::command]
pub async fn install_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_dnf(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn remove_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_dnf(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn update_dnf_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_dnf(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn install_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_flatpak(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn remove_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_flatpak(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn update_flatpak_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_flatpak(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn install_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::install_appimage(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn remove_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::remove_appimage(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn update_appimage(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || packages::update_appimage(&pkg_id)).await.unwrap_or(false))
}

#[tauri::command]
pub async fn get_native_packages() -> Vec<ai::PackageInfo> {
    tokio::task::spawn_blocking(|| {
        match packages::detect_backend() {
            packages::PkgBackend::Apt        => packages::get_apt_packages(),
            packages::PkgBackend::Pacman     => packages::get_pacman_packages(),
            packages::PkgBackend::Zypper     => packages::get_zypper_packages(),
            packages::PkgBackend::RpmOstree  => packages::get_rpm_ostree_packages(),
            _                                => packages::get_dnf_packages(),
        }
    }).await.unwrap_or_default()
}

#[tauri::command]
pub async fn get_detected_backend() -> String {
    format!("{:?}", packages::detect_backend())
}

#[tauri::command]
pub async fn install_native_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || {
        match packages::detect_backend() {
            packages::PkgBackend::Apt        => packages::install_apt(&pkg_id),
            packages::PkgBackend::Pacman     => packages::install_pacman(&pkg_id),
            packages::PkgBackend::Zypper     => packages::install_zypper(&pkg_id),
            packages::PkgBackend::RpmOstree  => packages::install_rpm_ostree(&pkg_id),
            _                                => packages::install_dnf(&pkg_id),
        }
    }).await.unwrap_or(false))
}

#[tauri::command]
pub async fn remove_native_package(pkg_id: String) -> Result<bool, String> {
    Ok(tokio::task::spawn_blocking(move || {
        match packages::detect_backend() {
            packages::PkgBackend::Apt        => packages::remove_apt(&pkg_id),
            packages::PkgBackend::Pacman     => packages::remove_pacman(&pkg_id),
            packages::PkgBackend::Zypper     => packages::remove_zypper(&pkg_id),
            packages::PkgBackend::RpmOstree  => packages::remove_rpm_ostree(&pkg_id),
            _                                => packages::remove_dnf(&pkg_id),
        }
    }).await.unwrap_or(false))
}

#[tauri::command]
pub async fn get_bootc_status() -> Option<packages::BootcStatus> {
    tokio::task::spawn_blocking(packages::get_bootc_status).await.unwrap_or(None)
}

#[tauri::command]
pub async fn bootc_upgrade() -> bool {
    tokio::task::spawn_blocking(packages::bootc_upgrade).await.unwrap_or(false)
}

#[tauri::command]
pub async fn bootc_switch_image(image: String) -> bool {
    tokio::task::spawn_blocking(move || packages::bootc_switch(&image)).await.unwrap_or(false)
}

#[tauri::command]
pub async fn rpm_ostree_upgrade() -> bool {
    tokio::task::spawn_blocking(packages::rpm_ostree_upgrade).await.unwrap_or(false)
}
