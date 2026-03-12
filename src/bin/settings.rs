use blue_environment::{
    config::BlueConfig,
    ipc::{self, IpcMessage},
};
use slint::{ComponentHandle, SharedString};

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let app = BlueSettings::new()?;

    // Wczytaj config
    let cfg = BlueConfig::load_or_default();
    app.set_current_wallpaper(SharedString::from(cfg.wallpaper.as_str()));

    // Callback dla zmiany tapety
    {
        let app_w = app.as_weak();
        app.on_wallpaper_selected(move |path| {
            let Some(a) = app_w.upgrade() else { return };
            let mut cfg = BlueConfig::load_or_default();
            cfg.wallpaper = path.to_string();
            if cfg.save().is_ok() {
                ipc::send(&IpcMessage::SetWallpaper { path: path.to_string() });
            }
        });
    }

    app.run()?;
    Ok(())
}
