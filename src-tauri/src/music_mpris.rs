use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use zbus::object_server::SignalContext;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};
use zbus::{interface, Connection};

pub const BUS_NAME: &str = "org.mpris.MediaPlayer2.blue_music";
pub const OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "cmd", content = "value", rename_all = "camelCase")]
pub enum MprisCommand {
    Play,
    Pause,
    PlayPause,
    Stop,
    Next,
    Previous,
    /// Relative seek, microseconds.
    Seek(i64),
    /// Absolute position, microseconds.
    SetPosition(i64),
    SetVolume(f64),
    SetShuffle(bool),
    /// "None" | "Track" | "Playlist"
    SetLoop(String),
    Raise,
}

#[derive(Clone, Debug)]
pub struct MprisState {
    pub has_track: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// `file://…` or `https://…` URL of the cover art (MPRIS clients can't read `data:` URLs).
    pub art_url: String,
    pub length_us: i64,
    pub position_us: i64,
    pub volume: f64,
    pub shuffle: bool,
    pub loop_status: String,
    pub can_next: bool,
    pub can_prev: bool,
    updated: Instant,
    /// Bumped on every track change → distinct `mpris:trackid`.
    track_serial: u64,
}

impl Default for MprisState {
    fn default() -> Self {
        Self {
            has_track: false, playing: false, title: String::new(), artist: String::new(), album: String::new(),
            art_url: String::new(), length_us: 0, position_us: 0, volume: 1.0, shuffle: false,
            loop_status: "None".into(), can_next: false, can_prev: false, updated: Instant::now(), track_serial: 0,
        }
    }
}

impl MprisState {
    fn playback_status(&self) -> &'static str {
        if !self.has_track { "Stopped" } else if self.playing { "Playing" } else { "Paused" }
    }
    fn position_now(&self) -> i64 {
        let extra = if self.playing { self.updated.elapsed().as_micros() as i64 } else { 0 };
        let p = self.position_us.saturating_add(extra);
        if self.length_us > 0 { p.clamp(0, self.length_us) } else { p.max(0) }
    }
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let mut m: HashMap<String, OwnedValue> = HashMap::new();
        let put = |m: &mut HashMap<String, OwnedValue>, k: &str, v: Value<'_>| {
            if let Ok(o) = OwnedValue::try_from(v) { m.insert(k.to_string(), o); }
        };
        let id = format!("/org/blue/music/track/{}", self.track_serial);
        if let Ok(p) = ObjectPath::try_from(id) { put(&mut m, "mpris:trackid", Value::ObjectPath(p)); }
        if !self.has_track { return m; }
        put(&mut m, "mpris:length", Value::I64(self.length_us));
        put(&mut m, "xesam:title", Value::from(self.title.clone()));
        if !self.artist.is_empty() { put(&mut m, "xesam:artist", Value::from(vec![self.artist.clone()])); }
        if !self.album.is_empty() { put(&mut m, "xesam:album", Value::from(self.album.clone())); }
        if !self.art_url.is_empty() { put(&mut m, "mpris:artUrl", Value::from(self.art_url.clone())); }
        m
    }
}

type Callback = Arc<dyn Fn(MprisCommand) + Send + Sync>;
type Shared = Arc<Mutex<MprisState>>;

struct Root { on_command: Callback }

#[interface(name = "org.mpris.MediaPlayer2")]
impl Root {
    fn raise(&self) { (self.on_command)(MprisCommand::Raise); }
    fn quit(&self) {}
    #[zbus(property)] fn can_quit(&self) -> bool { false }
    #[zbus(property)] fn can_raise(&self) -> bool { true }
    #[zbus(property)] fn has_track_list(&self) -> bool { false }
    #[zbus(property)] fn identity(&self) -> String { "Blue Music".into() }
    #[zbus(property)] fn supported_uri_schemes(&self) -> Vec<String> { vec!["file".into()] }
    #[zbus(property)] fn supported_mime_types(&self) -> Vec<String> { vec![] }
}

struct Player { state: Shared, on_command: Callback }

impl Player {
    fn emit(&self, c: MprisCommand) { (self.on_command)(c); }
    fn lock(&self) -> std::sync::MutexGuard<'_, MprisState> { self.state.lock().unwrap_or_else(|e| e.into_inner()) }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Player {
    fn next(&self) { self.emit(MprisCommand::Next); }
    fn previous(&self) { self.emit(MprisCommand::Previous); }
    fn pause(&self) { self.emit(MprisCommand::Pause); }
    fn play_pause(&self) { self.emit(MprisCommand::PlayPause); }
    fn stop(&self) { self.emit(MprisCommand::Stop); }
    fn play(&self) { self.emit(MprisCommand::Play); }
    fn seek(&self, offset: i64) { self.emit(MprisCommand::Seek(offset)); }
    fn set_position(&self, _track_id: ObjectPath<'_>, position: i64) { self.emit(MprisCommand::SetPosition(position)); }
    fn open_uri(&self, _uri: String) {}

    #[zbus(property)] fn playback_status(&self) -> String { self.lock().playback_status().into() }
    #[zbus(property)] fn loop_status(&self) -> String { self.lock().loop_status.clone() }
    #[zbus(property)] fn set_loop_status(&mut self, v: String) { self.emit(MprisCommand::SetLoop(v)); }
    #[zbus(property)] fn rate(&self) -> f64 { 1.0 }
    #[zbus(property)] fn minimum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)] fn maximum_rate(&self) -> f64 { 1.0 }
    #[zbus(property)] fn shuffle(&self) -> bool { self.lock().shuffle }
    #[zbus(property)] fn set_shuffle(&mut self, v: bool) { self.emit(MprisCommand::SetShuffle(v)); }
    #[zbus(property)] fn metadata(&self) -> HashMap<String, OwnedValue> { self.lock().metadata() }
    #[zbus(property)] fn volume(&self) -> f64 { self.lock().volume }
    #[zbus(property)] fn set_volume(&mut self, v: f64) { self.emit(MprisCommand::SetVolume(v.clamp(0.0, 1.0))); }
    #[zbus(property(emits_changed_signal = "false"))] fn position(&self) -> i64 { self.lock().position_now() }
    #[zbus(property)] fn can_go_next(&self) -> bool { self.lock().can_next }
    #[zbus(property)] fn can_go_previous(&self) -> bool { self.lock().can_prev }
    #[zbus(property)] fn can_play(&self) -> bool { self.lock().has_track }
    #[zbus(property)] fn can_pause(&self) -> bool { self.lock().has_track }
    #[zbus(property)] fn can_seek(&self) -> bool { self.lock().has_track }
    #[zbus(property)] fn can_control(&self) -> bool { true }

    /// Emitted after a seek that wasn't a normal playback progression.
    #[zbus(signal)]
    async fn seeked(ctxt: &SignalContext<'_>, position: i64) -> zbus::Result<()>;
}

/// What the frontend sends (`music_mpris_update`).
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct MprisUpdate {
    pub has_track: bool,
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art_url: String,
    pub length_secs: f64,
    pub position_secs: f64,
    pub volume: f64,
    pub shuffle: bool,
    pub loop_status: String,
    pub can_next: bool,
    pub can_prev: bool,
    /// True when the position jumped (seek) rather than advanced → emit `Seeked`.
    pub seeked: bool,
}
impl Default for MprisUpdate {
    fn default() -> Self {
        Self { has_track: false, playing: false, title: String::new(), artist: String::new(), album: String::new(), art_url: String::new(),
            length_secs: 0.0, position_secs: 0.0, volume: 1.0, shuffle: false, loop_status: "None".into(), can_next: false, can_prev: false, seeked: false }
    }
}

fn secs_to_us(s: f64) -> i64 { if s.is_finite() && s > 0.0 { (s * 1_000_000.0) as i64 } else { 0 } }

pub struct MprisServer { conn: Connection, state: Shared }

impl MprisServer {
    /// Connects to the session bus, claims [`BUS_NAME`] and exports the two interfaces.
    pub async fn start(bus_name: &str, on_command: Callback) -> zbus::Result<Self> {
        let state: Shared = Arc::new(Mutex::new(MprisState::default()));
        let conn = zbus::connection::Builder::session()?
            .name(bus_name.to_string())?
            .serve_at(OBJECT_PATH, Root { on_command: on_command.clone() })?
            .serve_at(OBJECT_PATH, Player { state: state.clone(), on_command })?
            .build()
            .await?;
        Ok(Self { conn, state })
    }

    /// Applies a state update and emits `PropertiesChanged` for the properties that changed.
    pub async fn update(&self, u: MprisUpdate) -> zbus::Result<()> {
        let (old, new) = {
            let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let old = s.clone();
            let track_changed = u.has_track != s.has_track || u.title != s.title || u.artist != s.artist || u.album != s.album
                || u.art_url != s.art_url || secs_to_us(u.length_secs) != s.length_us;
            s.has_track = u.has_track; s.playing = u.playing && u.has_track;
            s.title = u.title; s.artist = u.artist; s.album = u.album; s.art_url = u.art_url;
            s.length_us = secs_to_us(u.length_secs); s.position_us = secs_to_us(u.position_secs);
            s.volume = u.volume.clamp(0.0, 1.0); s.shuffle = u.shuffle;
            s.loop_status = match u.loop_status.as_str() { "Track" | "Playlist" => u.loop_status, _ => "None".into() };
            s.can_next = u.can_next; s.can_prev = u.can_prev;
            s.updated = Instant::now();
            if track_changed { s.track_serial += 1; }
            (old, s.clone())
        };
        let iface_ref = self.conn.object_server().interface::<_, Player>(OBJECT_PATH).await?;
        let iface = iface_ref.get().await;
        let ctx = iface_ref.signal_context();
        if old.playback_status() != new.playback_status() { iface.playback_status_changed(ctx).await?; }
        if old.track_serial != new.track_serial { iface.metadata_changed(ctx).await?; }
        if (old.volume - new.volume).abs() > f64::EPSILON { iface.volume_changed(ctx).await?; }
        if old.shuffle != new.shuffle { iface.shuffle_changed(ctx).await?; }
        if old.loop_status != new.loop_status { iface.loop_status_changed(ctx).await?; }
        if old.can_next != new.can_next { iface.can_go_next_changed(ctx).await?; }
        if old.can_prev != new.can_prev { iface.can_go_previous_changed(ctx).await?; }
        if old.has_track != new.has_track {
            iface.can_play_changed(ctx).await?; iface.can_pause_changed(ctx).await?; iface.can_seek_changed(ctx).await?;
        }
        if u_seeked(&old, &new) { Player::seeked(ctx, new.position_us).await?; }
        Ok(())
    }
}

/// A position that moved by more than playback could explain (> 2 s of drift) is a seek.
fn u_seeked(old: &MprisState, new: &MprisState) -> bool {
    new.has_track && old.has_track && old.track_serial == new.track_serial && (new.position_us - old.position_now()).abs() > 2_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_helpers() {
        let mut s = MprisState::default();
        assert_eq!(s.playback_status(), "Stopped");
        s.has_track = true; assert_eq!(s.playback_status(), "Paused");
        s.playing = true; assert_eq!(s.playback_status(), "Playing");
        s.length_us = 10_000_000; s.position_us = 9_999_000; s.playing = false;
        assert_eq!(s.position_now(), 9_999_000);
        s.position_us = 50_000_000; assert_eq!(s.position_now(), 10_000_000, "clamped to the length");
        assert_eq!(secs_to_us(1.5), 1_500_000);
        assert_eq!(secs_to_us(f64::NAN), 0);
        assert_eq!(secs_to_us(-3.0), 0);
    }

    #[test]
    fn metadata_has_the_standard_keys() {
        let mut s = MprisState::default();
        assert!(s.metadata().contains_key("mpris:trackid") && s.metadata().len() == 1, "no track → only trackid");
        s.has_track = true; s.title = "T".into(); s.artist = "A".into(); s.album = "B".into(); s.length_us = 5; s.art_url = "file:///x.jpg".into();
        let m = s.metadata();
        for k in ["mpris:trackid", "mpris:length", "xesam:title", "xesam:artist", "xesam:album", "mpris:artUrl"] { assert!(m.contains_key(k), "{k}"); }
    }

    #[test]
    fn seek_detection() {
        let mut a = MprisState::default(); a.has_track = true; a.position_us = 1_000_000; a.track_serial = 1;
        let mut b = a.clone(); b.position_us = 1_500_000;
        assert!(!u_seeked(&a, &b), "normal progression");
        b.position_us = 60_000_000; assert!(u_seeked(&a, &b));
        b.track_serial = 2; assert!(!u_seeked(&a, &b), "a new track is not a seek");
    }

    #[test]
    fn commands_serialize_for_the_frontend() {
        assert_eq!(serde_json::to_string(&MprisCommand::PlayPause).unwrap(), r#"{"cmd":"playPause"}"#);
        assert_eq!(serde_json::to_string(&MprisCommand::Seek(5)).unwrap(), r#"{"cmd":"seek","value":5}"#);
        assert_eq!(serde_json::to_string(&MprisCommand::SetLoop("Track".into())).unwrap(), r#"{"cmd":"setLoop","value":"Track"}"#);
    }
}

// ── Tauri glue ───────────────────────────────────────────────────────────

use std::sync::OnceLock;
use tauri::Emitter;

fn server_slot() -> &'static tokio::sync::Mutex<Option<MprisServer>> {
    static SLOT: OnceLock<tokio::sync::Mutex<Option<MprisServer>>> = OnceLock::new();
    SLOT.get_or_init(|| tokio::sync::Mutex::new(None))
}

/// Registers the MPRIS player. Safe to call repeatedly. Fails softly (returns an error string)
/// on systems without a session bus.
#[tauri::command]
pub async fn music_mpris_start(app: tauri::AppHandle) -> Result<(), String> {
    let mut slot = server_slot().lock().await;
    if slot.is_some() { return Ok(()); }
    let cb: Callback = Arc::new(move |c: MprisCommand| { let _ = app.emit("music-mpris", c); });
    let server = MprisServer::start(BUS_NAME, cb).await.map_err(|e| e.to_string())?;
    *slot = Some(server);
    Ok(())
}

#[tauri::command]
pub async fn music_mpris_update(update: MprisUpdate) -> Result<(), String> {
    let slot = server_slot().lock().await;
    match slot.as_ref() { Some(s) => s.update(update).await.map_err(|e| e.to_string()), None => Ok(()) }
}

/// Releases the bus name (the Music window was closed).
#[tauri::command]
pub async fn music_mpris_stop() -> Result<(), String> {
    server_slot().lock().await.take();
    Ok(())
}
