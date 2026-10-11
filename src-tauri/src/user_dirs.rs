use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Klucze w stałej kolejności: ta sama co `UserDirNames` w `engine.rs`
/// i `UserDirNames` w `Blue-Installer/types.ts`.
pub const KEYS: [&str; 8] = [
    "desktop", "documents", "downloads", "music", "pictures", "videos", "templates", "public",
];

/// Nazwa zmiennej w `user-dirs.dirs` dla danego klucza.
fn xdg_var(key: &str) -> &'static str {
    match key {
        "desktop" => "XDG_DESKTOP_DIR",
        "documents" => "XDG_DOCUMENTS_DIR",
        "downloads" => "XDG_DOWNLOAD_DIR",
        "music" => "XDG_MUSIC_DIR",
        "pictures" => "XDG_PICTURES_DIR",
        "videos" => "XDG_VIDEOS_DIR",
        "templates" => "XDG_TEMPLATES_DIR",
        "public" => "XDG_PUBLICSHARE_DIR",
        _ => "",
    }
}

/// Domyślne (angielskie) nazwy — to, co kod powłoki zakładał dotąd na sztywno.
pub fn english_name(key: &str) -> &'static str {
    match key {
        "desktop" => "Desktop",
        "documents" => "Documents",
        "downloads" => "Downloads",
        "music" => "Music",
        "pictures" => "Pictures",
        "videos" => "Videos",
        "templates" => "Templates",
        "public" => "Public",
        _ => "",
    }
}

/// Zlokalizowane nazwy: (prefiks języka, [desktop, documents, downloads,
/// music, pictures, videos, templates, public]). Kolejność odpowiada [`KEYS`].
/// Lustro tabeli `USER_DIRS_BY_LANG` z instalatora — przy dodawaniu języka
/// zaktualizuj obie.
pub const LOCALIZED: &[(&str, [&str; 8])] = &[
    ("en", ["Desktop", "Documents", "Downloads", "Music", "Pictures", "Videos", "Templates", "Public"]),
    ("pl", ["Pulpit", "Dokumenty", "Pobrane", "Muzyka", "Obrazy", "Wideo", "Szablony", "Publiczny"]),
    ("de", ["Schreibtisch", "Dokumente", "Downloads", "Musik", "Bilder", "Videos", "Vorlagen", "Öffentlich"]),
    ("fr", ["Bureau", "Documents", "Téléchargements", "Musique", "Images", "Vidéos", "Modèles", "Public"]),
    ("es", ["Escritorio", "Documentos", "Descargas", "Música", "Imágenes", "Vídeos", "Plantillas", "Público"]),
    ("it", ["Scrivania", "Documenti", "Scaricati", "Musica", "Immagini", "Video", "Modelli", "Pubblici"]),
    ("pt", ["Área de Trabalho", "Documentos", "Transferências", "Música", "Imagens", "Vídeos", "Modelos", "Público"]),
    ("nl", ["Bureaublad", "Documenten", "Downloads", "Muziek", "Afbeeldingen", "Video\u{2019}s", "Sjablonen", "Openbaar"]),
    ("sv", ["Skrivbord", "Dokument", "Hämtningar", "Musik", "Bilder", "Videoklipp", "Mallar", "Publikt"]),
    ("ru", ["Рабочий стол", "Документы", "Загрузки", "Музыка", "Изображения", "Видео", "Шаблоны", "Общедоступные"]),
    ("uk", ["Стільниця", "Документи", "Завантаження", "Музика", "Зображення", "Відео", "Шаблони", "Спільнодоступні"]),
    ("cs", ["Plocha", "Dokumenty", "Stažené", "Hudba", "Obrázky", "Videa", "Šablony", "Veřejné"]),
    ("hu", ["Asztal", "Dokumentumok", "Letöltések", "Zene", "Képek", "Videók", "Sablonok", "Nyilvános"]),
    ("tr", ["Masaüstü", "Belgeler", "İndirilenler", "Müzik", "Resimler", "Videolar", "Şablonlar", "Genel"]),
    ("ja", ["デスクトップ", "ドキュメント", "ダウンロード", "音楽", "画像", "ビデオ", "テンプレート", "公開"]),
    ("zh", ["桌面", "文档", "下载", "音乐", "图片", "视频", "模板", "公共"]),
];

/// Jeden katalog użytkownika — to, co dostaje frontend.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserDirEntry {
    /// `desktop`, `documents`, `downloads`, `music`, `pictures`, `videos`,
    /// `templates` albo `public`.
    pub key: String,
    /// Pełna, bezwzględna ścieżka.
    pub path: String,
    /// Ścieżka w konwencji frontendu (`HOME/Pobrane`) albo `None`, gdy katalog
    /// leży poza katalogiem domowym (np. na osobnej partycji).
    pub home_path: Option<String>,
    /// Nazwa katalogu do wyświetlenia (ostatni człon ścieżki, np. `Pobrane`).
    pub name: String,
    /// Czy katalog faktycznie istnieje na dysku.
    pub exists: bool,
}

/// Prefiks języka z wartości typu `pl_PL.UTF-8`, `pl`, `C.UTF-8` → `pl`/`c`.
pub fn lang_prefix(locale: &str) -> String {
    locale
        .split(|c| c == '_' || c == '.' || c == '@' || c == '-')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

/// Locale z zmiennych środowiskowych w kolejności, w jakiej respektuje je
/// glibc: `LC_ALL` > `LC_MESSAGES` > `LANG`.
pub fn locale_from_env() -> String {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return v;
            }
        }
    }
    String::new()
}

/// `$XDG_CONFIG_HOME` albo `~/.config`.
pub fn config_home(home: &Path) -> PathBuf {
    match std::env::var_os("XDG_CONFIG_HOME") {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => PathBuf::from(v),
        _ => home.join(".config"),
    }
}

/// Parsuje zawartość `user-dirs.dirs`. Zwraca klucz → bezwzględna ścieżka.
///
/// Zgodnie ze specyfikacją `xdg-user-dirs`: wartość równa katalogowi
/// domowemu oznacza „wyłączone" (taki wpis jest pomijany), a wartości są
/// w cudzysłowie i mogą zaczynać się od `$HOME`.
pub fn parse_user_dirs(content: &str, home: &Path) -> HashMap<String, PathBuf> {
    let mut out = HashMap::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else { continue };
        let name = name.trim();
        let Some(key) = KEYS.iter().find(|k| xdg_var(k) == name) else { continue };

        let mut v = value.trim();
        // Obetnij cudzysłów (xdg-user-dirs zawsze go pisze) i ewentualny
        // komentarz po zamykającym cudzysłowie.
        if let Some(rest) = v.strip_prefix('"') {
            v = rest.split('"').next().unwrap_or("");
        } else if let Some(rest) = v.strip_prefix('\'') {
            v = rest.split('\'').next().unwrap_or("");
        } else {
            v = v.split_whitespace().next().unwrap_or("");
        }
        if v.is_empty() {
            continue;
        }

        let path = if v == "$HOME" || v == "${HOME}" {
            home.to_path_buf()
        } else if let Some(rest) = v.strip_prefix("$HOME/").or_else(|| v.strip_prefix("${HOME}/")) {
            home.join(rest)
        } else if v.starts_with('/') {
            PathBuf::from(v)
        } else {
            // Ścieżka względna nie jest dozwolona przez specyfikację.
            continue;
        };
        // Zgodnie ze specyfikacją: katalog = $HOME oznacza „wyłączone".
        if path == home {
            continue;
        }
        out.insert((*key).to_string(), path);
    }
    out
}

/// Kolejność prób przy zgadywaniu nazwy katalogu bez pliku konfiguracyjnego:
/// najpierw język systemu, potem angielski, potem wszystkie pozostałe.
fn candidate_names(key: &str, lang: &str) -> Vec<&'static str> {
    let Some(idx) = KEYS.iter().position(|k| *k == key) else { return Vec::new() };
    let mut out: Vec<&'static str> = Vec::new();
    let mut push = |n: &'static str| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    if let Some((_, names)) = LOCALIZED.iter().find(|(l, _)| *l == lang) {
        push(names[idx]);
    }
    push(english_name(key));
    for (_, names) in LOCALIZED {
        push(names[idx]);
    }
    out
}

fn to_entry(key: &str, path: PathBuf, home: &Path) -> UserDirEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| english_name(key).to_string());
    let home_path = path
        .strip_prefix(home)
        .ok()
        .and_then(|rel| rel.to_str())
        .map(|rel| format!("HOME/{}", rel));
    UserDirEntry {
        key: key.to_string(),
        exists: path.is_dir(),
        home_path,
        name,
        path: path.to_string_lossy().into_owned(),
    }
}

/// Rozwiązuje wszystkie katalogi użytkownika.
///
/// * `home` — katalog domowy,
/// * `config_dir` — katalog z `user-dirs.dirs`,
/// * `locale` — locale systemu/użytkownika (`pl_PL.UTF-8`), używane tylko
///   do ustalenia kolejności zgadywania, gdy plik konfiguracyjny nie istnieje.
pub fn resolve_all(home: &Path, config_dir: &Path, locale: &str) -> Vec<UserDirEntry> {
    let configured = std::fs::read_to_string(config_dir.join("user-dirs.dirs"))
        .map(|c| parse_user_dirs(&c, home))
        .unwrap_or_default();
    let lang = lang_prefix(locale);

    KEYS.iter()
        .map(|key| {
            if let Some(p) = configured.get(*key) {
                return to_entry(key, p.clone(), home);
            }
            // Brak wpisu: szukamy istniejącego katalogu o znanej nazwie.
            for name in candidate_names(key, &lang) {
                let p = home.join(name);
                if p.is_dir() {
                    return to_entry(key, p, home);
                }
            }
            // Nic nie istnieje: nazwa z języka systemu (albo angielska).
            let fallback = LOCALIZED
                .iter()
                .find(|(l, _)| *l == lang)
                .and_then(|(_, names)| KEYS.iter().position(|k| k == key).map(|i| names[i]))
                .unwrap_or_else(|| english_name(key));
            to_entry(key, home.join(fallback), home)
        })
        .collect()
}

/// Mapuje „starą" ścieżkę względem katalogu domowego (`Downloads/plik.zip`)
/// na prawdziwą, jeśli:
///
/// * pierwszy człon to angielska nazwa któregoś z katalogów XDG,
/// * taki katalog **nie istnieje** (jeśli istnieje, użytkownik go ma i nic
///   nie przekierowujemy),
/// * a skonfigurowany/wykryty katalog tego typu istnieje i leży gdzie indziej.
///
/// Zwraca `None`, gdy nie ma czego mapować — wtedy wywołujący zostaje przy
/// oryginalnej ścieżce (i ewentualnym błędzie „Path does not exist").
pub fn remap_legacy(home: &Path, config_dir: &Path, locale: &str, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim_start_matches('/');
    let (first, rest) = match rel.split_once('/') {
        Some((f, r)) => (f, Some(r)),
        None => (rel, None),
    };
    let key = KEYS.iter().find(|k| english_name(k) == first)?;
    if home.join(first).is_dir() {
        return None;
    }
    let entry = resolve_all(home, config_dir, locale)
        .into_iter()
        .find(|e| e.key == *key)?;
    if !entry.exists {
        return None;
    }
    let base = PathBuf::from(&entry.path);
    if base == home.join(first) {
        return None;
    }
    Some(match rest {
        Some(r) if !r.is_empty() => base.join(r),
        _ => base,
    })
}

/// Wersja używająca prawdziwego środowiska (`$HOME`, `$XDG_CONFIG_HOME`,
/// `$LANG`) — dla `resolve_path` w Eksploratorze.
pub fn remap_legacy_env(rel: &str) -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    remap_legacy(&home, &config_home(&home), &locale_from_env(), rel)
}

/// Pierwszy istniejący katalog danego typu albo `~/<angielska nazwa>`.
/// Dla modułów backendu, które dotąd składały `home.join("Pictures")`.
pub fn dir_for(key: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    resolve_all(&home, &config_home(&home), &locale_from_env())
        .into_iter()
        .find(|e| e.key == key)
        .map(|e| PathBuf::from(e.path))
        .unwrap_or_else(|| home.join(english_name(key)))
}

/// Zwraca wszystkie katalogi użytkownika dla frontendu (Eksplorator, okno
/// wyboru plików, skróty w dialogach „Zapisz jako").
#[tauri::command(async)]
pub fn get_user_dirs() -> Vec<UserDirEntry> {
    let Some(home) = dirs::home_dir() else { return Vec::new() };
    resolve_all(&home, &config_home(&home), &locale_from_env())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("blue-userdirs-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn parses_standard_file() {
        let home = PathBuf::from("/home/ala");
        let c = "# komentarz\nXDG_DESKTOP_DIR=\"$HOME/Pulpit\"\nXDG_DOWNLOAD_DIR=\"$HOME/Pobrane\"\nXDG_MUSIC_DIR=\"/mnt/dane/Muzyka\"\nXDG_PUBLICSHARE_DIR=\"$HOME/\"\nXDG_TEMPLATES_DIR=\"$HOME\"\nCOS_INNEGO=\"x\"\n";
        let m = parse_user_dirs(c, &home);
        assert_eq!(m["desktop"], PathBuf::from("/home/ala/Pulpit"));
        assert_eq!(m["downloads"], PathBuf::from("/home/ala/Pobrane"));
        assert_eq!(m["music"], PathBuf::from("/mnt/dane/Muzyka"));
        // $HOME (z ukośnikiem albo bez) = wyłączone.
        assert!(!m.contains_key("templates"));
        assert!(!m.contains_key("public"));
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn rejects_relative_and_empty_values() {
        let home = PathBuf::from("/home/ala");
        let m = parse_user_dirs("XDG_DESKTOP_DIR=\"Pulpit\"\nXDG_MUSIC_DIR=\"\"\n", &home);
        assert!(m.is_empty());
    }

    #[test]
    fn configured_file_wins_over_guessing() {
        let home = tmp("conf");
        let cfg = home.join(".config");
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(home.join("Pobrane")).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        fs::write(cfg.join("user-dirs.dirs"), "XDG_DOWNLOAD_DIR=\"$HOME/Pobrane\"\n").unwrap();
        let all = resolve_all(&home, &cfg, "en_US.UTF-8");
        let dl = all.iter().find(|e| e.key == "downloads").unwrap();
        assert_eq!(dl.name, "Pobrane");
        assert!(dl.exists);
        assert_eq!(dl.home_path.as_deref(), Some("HOME/Pobrane"));
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn guesses_localized_dirs_without_config() {
        let home = tmp("guess");
        fs::create_dir_all(home.join("Pobrane")).unwrap();
        fs::create_dir_all(home.join("Dokumenty")).unwrap();
        let all = resolve_all(&home, &home.join(".config"), "pl_PL.UTF-8");
        assert_eq!(all.iter().find(|e| e.key == "downloads").unwrap().name, "Pobrane");
        assert_eq!(all.iter().find(|e| e.key == "documents").unwrap().name, "Dokumenty");
        // Nieistniejący katalog dostaje nazwę z języka systemu.
        let music = all.iter().find(|e| e.key == "music").unwrap();
        assert_eq!(music.name, "Muzyka");
        assert!(!music.exists);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn guesses_foreign_language_dirs_even_if_locale_differs() {
        let home = tmp("foreign");
        fs::create_dir_all(home.join("Téléchargements")).unwrap();
        let all = resolve_all(&home, &home.join(".config"), "en_US.UTF-8");
        let dl = all.iter().find(|e| e.key == "downloads").unwrap();
        assert_eq!(dl.name, "Téléchargements");
        assert!(dl.exists);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn falls_back_to_english_without_locale() {
        let home = tmp("english");
        let all = resolve_all(&home, &home.join(".config"), "");
        let dl = all.iter().find(|e| e.key == "downloads").unwrap();
        assert_eq!(dl.name, "Downloads");
        assert!(!dl.exists);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn remaps_legacy_english_paths() {
        let home = tmp("remap");
        let cfg = home.join(".config");
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(home.join("Pobrane/sub")).unwrap();
        fs::write(cfg.join("user-dirs.dirs"), "XDG_DOWNLOAD_DIR=\"$HOME/Pobrane\"\n").unwrap();
        assert_eq!(remap_legacy(&home, &cfg, "pl_PL.UTF-8", "Downloads"), Some(home.join("Pobrane")));
        assert_eq!(remap_legacy(&home, &cfg, "pl_PL.UTF-8", "Downloads/sub/a.zip"), Some(home.join("Pobrane/sub/a.zip")));
        // Nie-XDG ścieżki nie są ruszane.
        assert_eq!(remap_legacy(&home, &cfg, "pl_PL.UTF-8", "Projekty/x"), None);
        // Gdy angielski katalog istnieje naprawdę, nic nie przekierowujemy.
        fs::create_dir_all(home.join("Downloads")).unwrap();
        assert_eq!(remap_legacy(&home, &cfg, "pl_PL.UTF-8", "Downloads"), None);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn no_remap_when_target_missing() {
        let home = tmp("noremap");
        assert_eq!(remap_legacy(&home, &home.join(".config"), "pl_PL.UTF-8", "Downloads"), None);
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn lang_prefix_works() {
        assert_eq!(lang_prefix("pl_PL.UTF-8"), "pl");
        assert_eq!(lang_prefix("de_DE@euro"), "de");
        assert_eq!(lang_prefix("zh-CN"), "zh");
        assert_eq!(lang_prefix(""), "");
    }

    #[test]
    fn localized_table_has_eight_names_and_matches_keys() {
        for (lang, names) in LOCALIZED {
            assert_eq!(names.len(), KEYS.len(), "{lang}");
            assert!(names.iter().all(|n| !n.is_empty() && !n.contains('/')), "{lang}");
        }
    }
}
