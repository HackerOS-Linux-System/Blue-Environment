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

/// Tabela nazw katalogów dla wszystkich języków — JEDEN plik JSON współdzielony
/// z frontendem (`src/lib/data/userDirNames.json`, z którego korzysta też
/// instalator). Wczytywany w czasie kompilacji, więc nie ma duplikatu do
/// pilnowania: nowy język dopisujesz tylko w JSON-ie.
const NAMES_JSON: &str = include_str!("../../src/lib/data/userDirNames.json");

#[derive(serde::Deserialize)]
struct LangEntry {
    lang: String,
    names: HashMap<String, String>,
}

/// (prefiks języka, nazwy w kolejności [`KEYS`]) — w kolejności z pliku.
type Table = Vec<(String, Vec<String>)>;

fn table() -> &'static Table {
    static TABLE: std::sync::OnceLock<Table> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let entries: Vec<LangEntry> =
            serde_json::from_str(NAMES_JSON).expect("userDirNames.json is valid (checked by a unit test)");
        entries
            .into_iter()
            .map(|e| {
                let names = KEYS.iter().map(|k| e.names.get(*k).cloned().unwrap_or_default()).collect();
                (e.lang, names)
            })
            .collect()
    })
}

/// Zlokalizowane nazwy dla języka (`pl`) w kolejności [`KEYS`], jeśli znany.
pub fn names_for_lang(lang: &str) -> Option<&'static [String]> {
    table().iter().find(|(l, _)| l == lang).map(|(_, n)| n.as_slice())
}

/// Domyślne (angielskie) nazwy — to, co kod powłoki zakładał dotąd na sztywno.
pub fn english_name(key: &str) -> &'static str {
    let Some(idx) = KEYS.iter().position(|k| *k == key) else { return "" };
    names_for_lang("en").map(|n| n[idx].as_str()).unwrap_or("")
}

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
        if !n.is_empty() && !out.contains(&n) {
            out.push(n);
        }
    };
    if let Some(names) = names_for_lang(lang) {
        push(names[idx].as_str());
    }
    push(english_name(key));
    for (_, names) in table() {
        push(names[idx].as_str());
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
            let fallback = names_for_lang(&lang)
                .and_then(|names| KEYS.iter().position(|k| k == key).map(|i| names[i].as_str()))
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

// ─────────────────────────────────────────────────────────────────────────
// Obserwowanie zmian na żywo
// ─────────────────────────────────────────────────────────────────────────

/// Uruchamia wątek, który co `interval` sprawdza katalogi użytkownika i woła
/// `on_change` z nową listą, gdy cokolwiek się zmieniło: `user-dirs.dirs`
/// został przepisany (np. przez `xdg-user-dirs-update` po zmianie języka
/// systemu), katalog zmienił nazwę, został utworzony albo usunięty.
///
/// Odpytywanie zamiast `inotify`: to osiem wywołań `stat` co kilka sekund
/// (pomijalny koszt), działa identycznie na każdym systemie plików i nie
/// wymaga nowej zależności. Pierwsza lista jest tylko zapamiętywana (frontend
/// i tak pobiera ją sam przy starcie).
pub fn start_watcher<F>(interval: std::time::Duration, on_change: F)
where
    F: Fn(Vec<UserDirEntry>) + Send + 'static,
{
    let Some(home) = dirs::home_dir() else { return };
    let config = config_home(&home);
    watch(home, config, locale_from_env(), interval, on_change);
}

/// To samo z jawnymi ścieżkami (testowalne bez dotykania `$HOME` procesu).
fn watch<F>(home: PathBuf, config_dir: PathBuf, locale: String, interval: std::time::Duration, on_change: F)
where
    F: Fn(Vec<UserDirEntry>) + Send + 'static,
{
    let _ = std::thread::Builder::new().name("user-dirs-watch".into()).spawn(move || {
        let mut last = resolve_all(&home, &config_dir, &locale);
        loop {
            std::thread::sleep(interval);
            let now = resolve_all(&home, &config_dir, &locale);
            if now != last {
                last = now.clone();
                on_change(now);
            }
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────
// Migracja istniejącego systemu na nazwy zlokalizowane
// ─────────────────────────────────────────────────────────────────────────

/// Co zrobić z jednym katalogiem.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationAction {
    /// Nazwa już jest docelowa — nic do roboty.
    Keep,
    /// Katalog istnieje pod starą nazwą, docelowa jest wolna → zmieniamy nazwę.
    Rename,
    /// Żaden z nich nie istnieje → tworzymy katalog o nazwie docelowej.
    Create,
    /// Istnieją OBA (stary i docelowy) — nie scalamy automatycznie, bo można
    /// by coś nadpisać. Zostaje po staremu; człowiek decyduje.
    Conflict,
    /// Katalog leży poza katalogiem domowym (np. osobna partycja) — nie ruszamy.
    External,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MigrationItem {
    pub key: String,
    pub from: String,
    pub to: String,
    pub action: MigrationAction,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MigrationPlan {
    pub lang: String,
    pub items: Vec<MigrationItem>,
}

impl MigrationPlan {
    /// Czy jest cokolwiek do zrobienia (poza `Keep`/`Conflict`/`External`).
    pub fn has_work(&self) -> bool {
        self.items.iter().any(|i| matches!(i.action, MigrationAction::Rename | MigrationAction::Create))
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MigrationResult {
    pub renamed: usize,
    pub created: usize,
    pub skipped: usize,
    /// Komunikaty o błędach pojedynczych katalogów (reszta migracji idzie dalej).
    pub errors: Vec<String>,
}

/// Plan migracji do nazw języka `lang` (np. `pl`). `None`, gdy język nieznany.
pub fn plan_migration(home: &Path, config_dir: &Path, locale: &str, lang: &str) -> Option<MigrationPlan> {
    let target = names_for_lang(lang)?;
    let current = resolve_all(home, config_dir, locale);
    let items = KEYS
        .iter()
        .enumerate()
        .map(|(i, key)| {
            let cur = current.iter().find(|e| e.key == *key).expect("resolve_all returns every key");
            let to_path = home.join(&target[i]);
            let from = cur.path.clone();
            let to = to_path.to_string_lossy().into_owned();
            let action = if cur.home_path.is_none() {
                MigrationAction::External
            } else if PathBuf::from(&cur.path) == to_path {
                MigrationAction::Keep
            } else if cur.exists && to_path.is_dir() {
                MigrationAction::Conflict
            } else if cur.exists {
                MigrationAction::Rename
            } else if to_path.exists() {
                // Brak starego, ale docelowa nazwa jest zajęta czymś, co nie jest katalogiem.
                MigrationAction::Conflict
            } else {
                MigrationAction::Create
            };
            MigrationItem { key: (*key).to_string(), from, to, action }
        })
        .collect();
    Some(MigrationPlan { lang: lang.to_string(), items })
}

/// Zwraca treść `user-dirs.dirs` z podmienionymi wpisami `updates`
/// (klucz → bezwzględna ścieżka). Inne linie (komentarze, nieznane zmienne)
/// zostają nietknięte; brakujące wpisy są dopisywane. Ścieżki wewnątrz
/// katalogu domowego zapisujemy jako `$HOME/...`, tak jak `xdg-user-dirs`.
pub fn render_user_dirs(existing: &str, updates: &[(String, PathBuf)], home: &Path) -> String {
    let value = |p: &Path| -> String {
        match p.strip_prefix(home) {
            Ok(rel) => format!("\"$HOME/{}\"", rel.to_string_lossy()),
            Err(_) => format!("\"{}\"", p.to_string_lossy()),
        }
    };
    let mut remaining: Vec<&(String, PathBuf)> = updates.iter().collect();
    let mut out = String::new();
    for line in existing.lines() {
        let name = line.trim().split('=').next().unwrap_or("").trim();
        if let Some(pos) = remaining.iter().position(|(k, _)| xdg_var(k) == name) {
            let (k, p) = remaining.remove(pos);
            out.push_str(&format!("{}={}\n", xdg_var(k), value(p)));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (k, p) in remaining {
        out.push_str(&format!("{}={}\n", xdg_var(k), value(p)));
    }
    out
}

/// Wykonuje plan: zmienia nazwy / tworzy katalogi, potem atomowo (plik
/// tymczasowy + `rename`) przepisuje `user-dirs.dirs` i `user-dirs.locale`.
/// Katalogi z konfliktem i zewnętrzne zostają, z dotychczasowym wpisem.
pub fn apply_migration(home: &Path, config_dir: &Path, plan: &MigrationPlan) -> MigrationResult {
    let mut res = MigrationResult { renamed: 0, created: 0, skipped: 0, errors: Vec::new() };
    let mut updates: Vec<(String, PathBuf)> = Vec::new();

    for item in &plan.items {
        let to = PathBuf::from(&item.to);
        match item.action {
            MigrationAction::Rename => match std::fs::rename(&item.from, &to) {
                Ok(()) => { res.renamed += 1; updates.push((item.key.clone(), to)); }
                Err(e) => res.errors.push(format!("{} → {}: {e}", item.from, item.to)),
            },
            MigrationAction::Create => match std::fs::create_dir_all(&to) {
                Ok(()) => { res.created += 1; updates.push((item.key.clone(), to)); }
                Err(e) => res.errors.push(format!("{}: {e}", item.to)),
            },
            MigrationAction::Keep => updates.push((item.key.clone(), to)),
            MigrationAction::Conflict | MigrationAction::External => res.skipped += 1,
        }
    }

    if !updates.is_empty() {
        let file = config_dir.join("user-dirs.dirs");
        let existing = std::fs::read_to_string(&file).unwrap_or_default();
        let body = render_user_dirs(&existing, &updates, home);
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(config_dir)?;
            let tmp = config_dir.join(".user-dirs.dirs.blue-tmp");
            std::fs::write(&tmp, body)?;
            std::fs::rename(&tmp, &file)?;
            // Zgodne z `xdg-user-dirs-update`: zapamiętany język, żeby nie
            // "naprawiał" katalogów przy następnym logowaniu.
            std::fs::write(config_dir.join("user-dirs.locale"), format!("{}\n", plan.lang))
        };
        if let Err(e) = write() {
            res.errors.push(format!("user-dirs.dirs: {e}"));
        }
    }
    res
}

/// Plan migracji dla frontendu. `lang` = prefiks języka (`pl`); `None` →
/// język z locale systemu.
#[tauri::command(async)]
pub fn plan_user_dirs_migration(lang: Option<String>) -> Result<MigrationPlan, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let locale = locale_from_env();
    let lang = lang.map(|l| lang_prefix(&l)).filter(|l| !l.is_empty()).unwrap_or_else(|| lang_prefix(&locale));
    plan_migration(&home, &config_home(&home), &locale, &lang)
        .ok_or_else(|| format!("Unsupported language: {lang}"))
}

/// Wykonuje migrację. Plan jest liczony od nowa po stronie backendu (frontend
/// podaje tylko język), więc nie da się go podsunąć spreparowanego.
#[tauri::command(async)]
pub fn apply_user_dirs_migration(lang: String) -> Result<MigrationResult, String> {
    let home = dirs::home_dir().ok_or("No home directory")?;
    let locale = locale_from_env();
    let lang = lang_prefix(&lang);
    let plan = plan_migration(&home, &config_home(&home), &locale, &lang)
        .ok_or_else(|| format!("Unsupported language: {lang}"))?;
    Ok(apply_migration(&home, &config_home(&home), &plan))
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
    fn shared_table_is_complete_and_sane() {
        let entries: Vec<LangEntry> = serde_json::from_str(NAMES_JSON).unwrap();
        assert!(entries.len() >= 16);
        assert_eq!(entries[0].lang, "en");
        for e in &entries {
            for key in KEYS {
                let n = e.names.get(key).unwrap_or_else(|| panic!("{}: missing {key}", e.lang));
                assert!(!n.is_empty() && !n.contains('/') && !n.contains('"') && !n.contains('$'), "{}: {key}", e.lang);
            }
            assert_eq!(e.names.len(), KEYS.len(), "{}: extra keys", e.lang);
        }
        assert_eq!(english_name("downloads"), "Downloads");
        assert_eq!(names_for_lang("pl").unwrap()[2], "Pobrane");
    }

    #[test]
    fn plans_rename_create_conflict_and_keep() {
        let home = tmp("plan");
        let cfg = home.join(".config");
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::create_dir_all(home.join("Dokumenty")).unwrap(); // conflict
        fs::create_dir_all(home.join("Pulpit")).unwrap();
        fs::write(cfg.join("user-dirs.dirs"), "XDG_DESKTOP_DIR=\"$HOME/Pulpit\"\nXDG_DOCUMENTS_DIR=\"$HOME/Documents\"\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nXDG_MUSIC_DIR=\"/mnt/dane/Music\"\n").unwrap();
        let plan = plan_migration(&home, &cfg, "en_US.UTF-8", "pl").unwrap();
        let act = |k: &str| plan.items.iter().find(|i| i.key == k).unwrap().action.clone();
        assert_eq!(act("desktop"), MigrationAction::Keep);
        assert_eq!(act("documents"), MigrationAction::Conflict);
        assert_eq!(act("downloads"), MigrationAction::Rename);
        assert_eq!(act("music"), MigrationAction::External);
        assert_eq!(act("videos"), MigrationAction::Create);
        assert!(plan.has_work());
        assert!(plan_migration(&home, &cfg, "en", "xx").is_none());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn render_replaces_adds_and_keeps_other_lines() {
        let home = PathBuf::from("/home/ala");
        let existing = "# my comment\nXDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\nFOO=\"bar\"\n";
        let out = render_user_dirs(existing, &[
            ("downloads".into(), home.join("Pobrane")),
            ("music".into(), PathBuf::from("/mnt/x/Muzyka")),
        ], &home);
        assert!(out.contains("# my comment\n"));
        assert!(out.contains("FOO=\"bar\"\n"));
        assert!(out.contains("XDG_DOWNLOAD_DIR=\"$HOME/Pobrane\"\n"));
        assert!(!out.contains("Downloads"));
        assert!(out.contains("XDG_MUSIC_DIR=\"/mnt/x/Muzyka\"\n"));
    }

    #[test]
    fn apply_migrates_end_to_end_and_is_idempotent() {
        let home = tmp("apply");
        let cfg = home.join(".config");
        fs::create_dir_all(&cfg).unwrap();
        fs::create_dir_all(home.join("Downloads")).unwrap();
        fs::write(home.join("Downloads/a.txt"), "x").unwrap();
        fs::write(cfg.join("user-dirs.dirs"), "XDG_DOWNLOAD_DIR=\"$HOME/Downloads\"\n").unwrap();

        let plan = plan_migration(&home, &cfg, "en_US.UTF-8", "pl").unwrap();
        let res = apply_migration(&home, &cfg, &plan);
        assert!(res.errors.is_empty(), "{:?}", res.errors);
        assert_eq!(res.renamed, 1);
        assert!(home.join("Pobrane/a.txt").is_file(), "files must survive the rename");
        assert!(!home.join("Downloads").exists());
        assert!(home.join("Wideo").is_dir(), "missing folders are created");
        let conf = fs::read_to_string(cfg.join("user-dirs.dirs")).unwrap();
        assert!(conf.contains("XDG_DOWNLOAD_DIR=\"$HOME/Pobrane\""));
        assert_eq!(fs::read_to_string(cfg.join("user-dirs.locale")).unwrap().trim(), "pl");

        // Second run: nothing left to do.
        let again = plan_migration(&home, &cfg, "en_US.UTF-8", "pl").unwrap();
        assert!(!again.has_work());
        // The file manager now resolves the localized folder.
        let dl = resolve_all(&home, &cfg, "en_US.UTF-8").into_iter().find(|e| e.key == "downloads").unwrap();
        assert_eq!(dl.name, "Pobrane");
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn apply_leaves_conflicts_untouched() {
        let home = tmp("conflict");
        let cfg = home.join(".config");
        fs::create_dir_all(home.join("Documents")).unwrap();
        fs::create_dir_all(home.join("Dokumenty")).unwrap();
        fs::write(home.join("Documents/keep.txt"), "x").unwrap();
        let plan = plan_migration(&home, &cfg, "en_US.UTF-8", "pl").unwrap();
        let res = apply_migration(&home, &cfg, &plan);
        assert!(res.skipped >= 1);
        assert!(home.join("Documents/keep.txt").is_file());
        let _ = fs::remove_dir_all(&home);
    }

    #[test]
    fn watcher_reports_changes() {
        use std::sync::mpsc;
        let home = tmp("watch");
        let (tx, rx) = mpsc::channel();
        watch(home.clone(), home.join(".config"), String::new(), std::time::Duration::from_millis(30), move |d| { let _ = tx.send(d); });
        std::thread::sleep(std::time::Duration::from_millis(100));
        fs::create_dir_all(home.join("Downloads")).unwrap();
        let got = rx.recv_timeout(std::time::Duration::from_secs(3)).expect("a change event");
        assert!(got.iter().any(|e| e.key == "downloads" && e.exists));
        let _ = fs::remove_dir_all(&home);
    }
}
