use lofty::file::TaggedFileExt;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "wma", "mka", "ape", "wv", "aiff", "aif", "mpc",
];
const MAX_DEPTH: usize = 16;
const MAX_TRACKS: usize = 100_000;
const COVER_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart", "albumartsmall"];
const COVER_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const MAX_COVER_BYTES: usize = 8 * 1024 * 1024;
const THUMB_PX: u32 = 320;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MusicTrack {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    /// 0 = unknown
    pub track_no: u32,
    pub disc_no: u32,
    pub year: u32,
    pub genre: String,
    pub duration_secs: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct CacheEntry {
    mtime: u64,
    size: u64,
    track: MusicTrack,
}
type Cache = HashMap<String, CacheEntry>;

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub tracks: Vec<MusicTrack>,
    /// Folders that could not be read (missing / permission).
    pub missing_folders: Vec<String>,
    /// How many tracks were read from the cache without touching their tags.
    pub reused: usize,
}

fn config_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".config/Blue-Environment/blue-music")
}
fn art_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")).join(".cache/Blue-Environment/blue-music/art")
}

fn expand(path: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if path == "HOME" || path == "~" { home }
    else if let Some(rest) = path.strip_prefix("HOME/").or_else(|| path.strip_prefix("~/")) { home.join(rest) }
    else { PathBuf::from(path) }
}

fn is_audio(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).map(|e| AUDIO_EXTS.contains(&e.to_lowercase().as_str())).unwrap_or(false)
}

fn clean(s: Option<impl AsRef<str>>) -> String {
    s.map(|v| v.as_ref().trim().to_string()).unwrap_or_default()
}

fn file_stem(path: &Path) -> String {
    path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Reads one file's tags. Never fails: an unreadable file still gets a track named after its file.
fn read_track(path: &Path) -> MusicTrack {
    let mut t = MusicTrack { path: path.to_string_lossy().to_string(), title: file_stem(path), ..Default::default() };
    let Ok(probe) = Probe::open(path) else { return t };
    let Ok(tagged) = probe.read() else { return t };
    t.duration_secs = tagged.properties().duration().as_secs_f64();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        let title = clean(tag.title());
        if !title.is_empty() { t.title = title; }
        t.artist = clean(tag.artist());
        t.album = clean(tag.album());
        t.genre = clean(tag.genre());
        t.album_artist = clean(tag.get_string(&ItemKey::AlbumArtist));
        t.track_no = tag.track().unwrap_or(0);
        t.disc_no = tag.disk().unwrap_or(0);
        t.year = tag.year().unwrap_or(0);
    }
    t
}

fn mtime_of(meta: &std::fs::Metadata) -> u64 {
    meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// Scans `folders` (recursively) and returns every audio file with its tags.
pub fn scan_folders(folders: &[PathBuf], cache_file: &Path) -> ScanResult {
    let mut result = ScanResult::default();
    let cache: Cache = std::fs::read_to_string(cache_file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

    // 1) walk
    let mut files: Vec<(PathBuf, u64, u64)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for folder in folders {
        if !folder.is_dir() { result.missing_folders.push(folder.to_string_lossy().to_string()); continue; }
        let walker = walkdir::WalkDir::new(folder).follow_links(false).max_depth(MAX_DEPTH).into_iter()
            .filter_entry(|e| e.depth() == 0 || !e.file_name().to_string_lossy().starts_with('.'));
        for entry in walker.flatten() {
            if !entry.file_type().is_file() || !is_audio(entry.path()) { continue; }
            let Ok(meta) = entry.metadata() else { continue };
            let p = entry.path().to_path_buf();
            if !seen.insert(p.clone()) { continue; } // overlapping folders → list a file once
            files.push((p, mtime_of(&meta), meta.len()));
            if files.len() >= MAX_TRACKS { break; }
        }
    }

    // 2) split into cached / needs-reading
    let mut tracks: Vec<MusicTrack> = Vec::with_capacity(files.len());
    let mut todo: Vec<(PathBuf, u64, u64)> = Vec::new();
    for (p, mtime, size) in files {
        match cache.get(p.to_string_lossy().as_ref()) {
            Some(c) if c.mtime == mtime && c.size == size => { tracks.push(c.track.clone()); result.reused += 1; }
            _ => todo.push((p, mtime, size)),
        }
    }

    // 3) read the rest in parallel
    let fresh: Mutex<Vec<(MusicTrack, u64, u64)>> = Mutex::new(Vec::with_capacity(todo.len()));
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 8).min(todo.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..workers {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((p, mtime, size)) = todo.get(i) else { break };
                // a malformed file must not take the whole scan down
                let t = std::panic::catch_unwind(|| read_track(p))
                    .unwrap_or_else(|_| MusicTrack { path: p.to_string_lossy().to_string(), title: file_stem(p), ..Default::default() });
                fresh.lock().unwrap().push((t, *mtime, *size));
            });
        }
    });

    // 4) persist the cache (only files that still exist) and return
    let mut new_cache: Cache = HashMap::new();
    for (p, mtime, size) in tracks.iter().filter_map(|t| cache.get(&t.path).map(|c| (t.path.clone(), c.mtime, c.size))) {
        if let Some(c) = cache.get(&p) { new_cache.insert(p, CacheEntry { mtime, size, track: c.track.clone() }); }
    }
    for (t, mtime, size) in fresh.into_inner().unwrap() {
        new_cache.insert(t.path.clone(), CacheEntry { mtime, size, track: t.clone() });
        tracks.push(t);
    }
    if let Some(parent) = cache_file.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_string(&new_cache) {
        // write-then-rename so a crash never leaves a half-written cache
        let tmp = cache_file.with_extension("tmp");
        if std::fs::write(&tmp, json).is_ok() { let _ = std::fs::rename(&tmp, cache_file); }
    }
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    result.tracks = tracks;
    result
}

// ── Cover art ────────────────────────────────────────────────────────────

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes { h ^= *b as u64; h = h.wrapping_mul(0x100000001b3); }
    h
}

/// Embedded picture (front cover preferred) or a folder image (`cover.jpg`, `folder.png`…) next to the track.
fn find_cover(track: &Path) -> Option<Vec<u8>> {
    if let Ok(probe) = Probe::open(track) {
        if let Ok(tagged) = probe.read() {
            let pics = tagged.primary_tag().into_iter().chain(tagged.tags().iter()).flat_map(|t| t.pictures().iter()).collect::<Vec<_>>();
            let best = pics.iter().find(|p| p.pic_type() == PictureType::CoverFront).or_else(|| pics.first());
            if let Some(p) = best { if !p.data().is_empty() { return Some(p.data().to_vec()); } }
        }
    }
    let dir = track.parent()?;
    let mut found: Option<(usize, PathBuf)> = None;
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        let (Some(stem), Some(ext)) = (p.file_stem().and_then(|s| s.to_str()), p.extension().and_then(|s| s.to_str())) else { continue };
        let (stem, ext) = (stem.to_lowercase(), ext.to_lowercase());
        if !COVER_EXTS.contains(&ext.as_str()) { continue; }
        if let Some(rank) = COVER_NAMES.iter().position(|n| *n == stem) {
            if found.as_ref().map_or(true, |(r, _)| rank < *r) { found = Some((rank, p)); }
        }
    }
    found.and_then(|(_, p)| std::fs::read(p).ok())
}

/// Writes a ≤320px JPEG thumbnail (or the original if it can't be decoded) into the art cache and
/// returns its path. The same bytes always map to the same file name, so albums share one file.
fn cache_cover(bytes: &[u8], art_dir: &Path) -> Option<PathBuf> {
    if bytes.is_empty() || bytes.len() > MAX_COVER_BYTES { return None; }
    std::fs::create_dir_all(art_dir).ok()?;
    let key = fnv1a(bytes);
    let jpg = art_dir.join(format!("{key:016x}.jpg"));
    if jpg.exists() { return Some(jpg); }
    if let Ok(img) = image::load_from_memory(bytes) {
        let thumb = if img.width() > THUMB_PX || img.height() > THUMB_PX { img.thumbnail(THUMB_PX, THUMB_PX) } else { img };
        let mut out = Vec::new();
        let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82);
        if thumb.to_rgb8().write_with_encoder(enc).is_ok() && std::fs::write(&jpg, &out).is_ok() { return Some(jpg); }
    }
    // undecodable (e.g. WebP): keep the original if it is reasonably small
    if bytes.len() <= 1_500_000 {
        let raw = art_dir.join(format!("{key:016x}.img"));
        if std::fs::write(&raw, bytes).is_ok() { return Some(raw); }
    }
    None
}

// ── Tauri commands ───────────────────────────────────────────────────────

/// Scans the given folders (`HOME/…`, `~/…` or absolute) and returns the library.
#[tauri::command(async)]
pub fn music_scan(folders: Vec<String>) -> ScanResult {
    let folders: Vec<PathBuf> = folders.iter().map(|f| expand(f)).collect();
    scan_folders(&folders, &config_dir().join("library-cache.json"))
}

/// Path of a cached thumbnail for the track's cover, or `None` if it has no artwork.
#[tauri::command(async)]
pub fn music_cover(path: String) -> Option<String> {
    let bytes = find_cover(&expand(&path))?;
    cache_cover(&bytes, &art_dir()).map(|p| p.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lofty::config::WriteOptions;
    use lofty::tag::{Tag, TagType};
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("blue-music-{}-{}-{}", name, std::process::id(), fnv1a(name.as_bytes())));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    /// A valid 0.5 s mono 8 kHz PCM WAV.
    fn wav_bytes() -> Vec<u8> {
        let data = vec![128u8; 4000];
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF"); v.extend_from_slice(&(36 + data.len() as u32).to_le_bytes()); v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); v.extend_from_slice(&1u16.to_le_bytes()); v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&8000u32.to_le_bytes()); v.extend_from_slice(&8000u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); v.extend_from_slice(&8u16.to_le_bytes());
        v.extend_from_slice(b"data"); v.extend_from_slice(&(data.len() as u32).to_le_bytes()); v.extend_from_slice(&data);
        v
    }

    fn write_tagged_wav(path: &Path, title: &str, artist: &str, album: &str, track: u32) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, wav_bytes()).unwrap();
        let mut tag = Tag::new(TagType::RiffInfo);
        tag.set_title(title.into()); tag.set_artist(artist.into()); tag.set_album(album.into()); tag.set_track(track);
        tag.save_to_path(path, WriteOptions::default()).unwrap();
    }

    #[test]
    fn reads_tags_and_duration() {
        let dir = tmp("tags");
        let f = dir.join("x.wav");
        write_tagged_wav(&f, "Song A", "The Band", "First Album", 3);
        let t = read_track(&f);
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.album.as_str(), t.track_no), ("Song A", "The Band", "First Album", 3));
        assert!((t.duration_secs - 0.5).abs() < 0.05, "duration {}", t.duration_secs);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn untagged_and_corrupt_files_fall_back_to_the_file_name() {
        let dir = tmp("fallback");
        fs::write(dir.join("Plain Name.mp3"), b"not really audio").unwrap();
        fs::write(dir.join("empty.flac"), b"").unwrap();
        let t = read_track(&dir.join("Plain Name.mp3"));
        assert_eq!((t.title.as_str(), t.artist.as_str(), t.duration_secs), ("Plain Name", "", 0.0));
        assert_eq!(read_track(&dir.join("empty.flac")).title, "empty");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scans_recursively_skips_hidden_and_non_audio_and_reports_missing() {
        let root = tmp("scan"); let cache = root.join("cache.json");
        write_tagged_wav(&root.join("Artist/Album/01.wav"), "One", "Artist", "Album", 1);
        write_tagged_wav(&root.join("Artist/Album/Disc 2/02.wav"), "Two", "Artist", "Album", 2);
        write_tagged_wav(&root.join(".hidden/03.wav"), "Hidden", "x", "x", 1);
        fs::write(root.join("Artist/notes.txt"), "x").unwrap();
        fs::write(root.join("Artist/cover.jpg"), b"x").unwrap();
        let res = scan_folders(&[root.clone(), root.join("Artist"), root.join("missing")], &cache); // overlapping + missing
        let titles: Vec<_> = res.tracks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, vec!["One", "Two"], "{titles:?}");
        assert_eq!(res.missing_folders.len(), 1);
        assert_eq!(res.reused, 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn second_scan_reuses_the_cache_and_rereads_only_changed_files() {
        let root = tmp("cache"); let cache = root.join("cache.json");
        let a = root.join("a.wav"); let b = root.join("b.wav");
        write_tagged_wav(&a, "A", "x", "x", 1); write_tagged_wav(&b, "B", "x", "x", 2);
        assert_eq!(scan_folders(&[root.clone()], &cache).reused, 0);
        let again = scan_folders(&[root.clone()], &cache);
        assert_eq!((again.reused, again.tracks.len()), (2, 2));
        // retag b (size changes) → only b is re-read
        write_tagged_wav(&b, "B changed with a longer title", "x", "x", 2);
        let third = scan_folders(&[root.clone()], &cache);
        assert_eq!(third.reused, 1);
        assert!(third.tracks.iter().any(|t| t.title == "B changed with a longer title"));
        // delete a → dropped from results and from the cache
        fs::remove_file(&a).unwrap();
        let fourth = scan_folders(&[root.clone()], &cache);
        assert_eq!(fourth.tracks.len(), 1);
        assert!(!fs::read_to_string(&cache).unwrap().contains("a.wav"));
        // a corrupt cache file is ignored, not fatal
        fs::write(&cache, "{{{").unwrap();
        assert_eq!(scan_folders(&[root.clone()], &cache).tracks.len(), 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn folder_cover_is_found_ranked_and_thumbnailed() {
        let root = tmp("cover");
        fs::write(root.join("t.mp3"), b"x").unwrap();
        // a real 600x600 PNG
        let img = image::RgbImage::from_fn(600, 600, |x, y| image::Rgb([(x % 255) as u8, (y % 255) as u8, 90]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        fs::write(root.join("random.png"), b"ignored").unwrap();
        fs::write(root.join("Folder.PNG"), &png).unwrap();
        fs::write(root.join("cover.jpg"), b"not decodable but ranked first").unwrap();
        assert_eq!(find_cover(&root.join("t.mp3")).unwrap(), b"not decodable but ranked first", "cover.* beats folder.*");
        fs::remove_file(root.join("cover.jpg")).unwrap();
        let bytes = find_cover(&root.join("t.mp3")).unwrap();
        assert_eq!(bytes, png);
        let art = root.join("art");
        let out = cache_cover(&bytes, &art).unwrap();
        let thumb = image::open(&out).unwrap();
        assert!(thumb.width() <= THUMB_PX && thumb.height() <= THUMB_PX, "{}x{}", thumb.width(), thumb.height());
        assert_eq!(cache_cover(&bytes, &art).unwrap(), out, "same bytes → same file");
        assert!(cache_cover(b"", &art).is_none());
        assert!(find_cover(&root.join("nope/x.mp3")).is_none());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn paths_expand_home_aliases() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand("HOME/Music"), home.join("Music"));
        assert_eq!(expand("~/Music"), home.join("Music"));
        assert_eq!(expand("/srv/music"), PathBuf::from("/srv/music"));
    }
}
