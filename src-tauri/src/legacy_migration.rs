use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const OLD_APP_IDENTIFIER: &str = "org.legendaryos.blue-environment";
pub const NEW_APP_IDENTIFIER: &str = "org.hackeros.blue-environment";

/// Wynik pojedynczej próby migracji (głównie na potrzeby testów i logów).
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nie było czego migrować (brak starego katalogu).
    NothingToDo,
    /// Nowy katalog już istnieje — nic nie ruszamy (nie nadpisujemy danych).
    TargetExists,
    /// Stary katalog jest już symlinkiem (migracja była zrobiona wcześniej).
    AlreadyLinked,
    /// Przeniesiono.
    Moved,
}

/// Przenosi `old` → `new` i zostawia symlink `old` → `new`.
pub fn migrate_dir(old: &Path, new: &Path) -> io::Result<Outcome> {
    // `symlink_metadata`, nie `exists`: martwy symlink też ma być rozpoznany.
    let old_meta = match fs::symlink_metadata(old) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Outcome::NothingToDo),
        Err(e) => return Err(e),
    };
    if old_meta.file_type().is_symlink() {
        return Ok(Outcome::AlreadyLinked);
    }
    if !old_meta.is_dir() {
        return Ok(Outcome::NothingToDo);
    }
    if fs::symlink_metadata(new).is_ok() {
        return Ok(Outcome::TargetExists);
    }
    if let Some(parent) = new.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(old, new)?;
    #[cfg(unix)]
    {
        // Brak symlinka to nie błąd — dane i tak są już w nowym miejscu.
        if let Err(e) = std::os::unix::fs::symlink(new, old) {
            tracing::warn!("[migration] nie udało się utworzyć symlinka {} -> {}: {e}", old.display(), new.display());
        }
    }
    Ok(Outcome::Moved)
}

/// Pary (stary, nowy) do przeniesienia dla danego katalogu domowego i
/// katalogów XDG. Wydzielone, żeby dało się to przetestować bez dotykania
/// prawdziwego `$HOME`.
pub fn plan(
    home: &Path,
    data_dir: Option<PathBuf>,
    config_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
) -> Vec<(PathBuf, PathBuf)> {
    let mut pairs = vec![(
        home.join(".legendaryos/Blue-Environment"),
        home.join(".hackeros/Blue-Environment"),
    )];
    for base in [data_dir, config_dir, cache_dir].into_iter().flatten() {
        pairs.push((base.join(OLD_APP_IDENTIFIER), base.join(NEW_APP_IDENTIFIER)));
    }
    pairs
}

/// Punkt wejścia — wołany z `main()`. Nigdy nie panikuje.
pub fn run() {
    let Some(home) = dirs::home_dir() else { return };
    for (old, new) in plan(&home, dirs::data_dir(), dirs::config_dir(), dirs::cache_dir()) {
        match migrate_dir(&old, &new) {
            Ok(Outcome::Moved) => tracing::info!("[migration] {} -> {}", old.display(), new.display()),
            Ok(Outcome::TargetExists) => tracing::warn!(
                "[migration] pomijam {}: {} już istnieje (nie nadpisuję)", old.display(), new.display()
            ),
            Ok(_) => {}
            Err(e) => tracing::warn!("[migration] {} -> {} nie powiodło się: {e}", old.display(), new.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("blue-mig-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn moves_and_leaves_symlink() {
        let root = tmp("move");
        let old = root.join(".legendaryos/Blue-Environment");
        let new = root.join(".hackeros/Blue-Environment");
        fs::create_dir_all(old.join("apps/foo")).unwrap();
        fs::write(old.join("apps/foo/icon.png"), b"x").unwrap();

        assert_eq!(migrate_dir(&old, &new).unwrap(), Outcome::Moved);
        assert!(new.join("apps/foo/icon.png").exists());
        // stara ścieżka nadal działa (przez symlink)
        assert!(old.join("apps/foo/icon.png").exists());
        assert!(fs::symlink_metadata(&old).unwrap().file_type().is_symlink());
        // drugie wywołanie jest bezpieczne
        assert_eq!(migrate_dir(&old, &new).unwrap(), Outcome::AlreadyLinked);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn never_overwrites_existing_target() {
        let root = tmp("exists");
        let old = root.join("old");
        let new = root.join("new");
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("a"), b"old").unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(new.join("a"), b"new").unwrap();

        assert_eq!(migrate_dir(&old, &new).unwrap(), Outcome::TargetExists);
        assert_eq!(fs::read(new.join("a")).unwrap(), b"new");
        assert_eq!(fs::read(old.join("a")).unwrap(), b"old");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn nothing_to_do_when_old_missing() {
        let root = tmp("missing");
        assert_eq!(migrate_dir(&root.join("nope"), &root.join("new")).unwrap(), Outcome::NothingToDo);
        assert!(!root.join("new").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creates_missing_parent_of_new() {
        let root = tmp("parent");
        let old = root.join("a/old");
        let new = root.join("deep/er/new");
        fs::create_dir_all(&old).unwrap();
        assert_eq!(migrate_dir(&old, &new).unwrap(), Outcome::Moved);
        assert!(new.is_dir());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn plan_covers_apps_and_all_three_tauri_dirs() {
        let home = PathBuf::from("/home/u");
        let p = plan(&home, Some("/home/u/.local/share".into()), Some("/home/u/.config".into()), Some("/home/u/.cache".into()));
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].1, PathBuf::from("/home/u/.hackeros/Blue-Environment"));
        assert!(p[1].0.ends_with("org.legendaryos.blue-environment"));
        assert!(p[3].1.ends_with("org.hackeros.blue-environment"));
    }
}
