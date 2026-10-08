use base64::Engine as _;
use ffmpeg_sidecar::command::FfmpegCommand;
use ffmpeg_sidecar::event::FfmpegEvent;
use serde::{Deserialize, Serialize};
use std::process::Command;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub path: String,
    /// Początek fragmentu w pliku źródłowym (s).
    pub start: f64,
    /// Długość fragmentu źródłowego (s).
    pub duration: f64,
    #[serde(default = "one")]
    pub speed: f64,
    #[serde(default = "one")]
    pub volume: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default = "yes")]
    pub has_audio: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextOverlay {
    pub text: String,
    /// Czas na osi wynikowej (s).
    pub start: f64,
    pub end: f64,
    /// Pozycja środka tekstu w % szerokości/wysokości.
    #[serde(default = "half")]
    pub x: f64,
    #[serde(default = "ninety")]
    pub y: f64,
    #[serde(default = "default_size")]
    pub size: u32,
    #[serde(default = "white")]
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Music {
    pub path: String,
    #[serde(default = "half")]
    pub volume: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub texts: Vec<TextOverlay>,
    pub music: Option<Music>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    #[serde(default = "default_crf")]
    pub crf: u32,
    pub output: String,
}

fn one() -> f64 { 1.0 }
fn half() -> f64 { 0.5 }
fn ninety() -> f64 { 0.9 }
fn yes() -> bool { true }
fn default_size() -> u32 { 48 }
fn white() -> String { "white".into() }
fn default_crf() -> u32 { 23 }

/// Znaki specjalne ffmpeg w wartościach `drawtext`.
pub fn escape_drawtext(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            ':' => o.push_str("\\:"),
            '\'' => o.push_str("\u{2019}"), // typograficzny apostrof — unika cytowania
            '%' => o.push_str("\\%"),
            '\n' => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

/// `atempo` akceptuje 0.5–2.0, więc większe/mniejsze tempo to łańcuch.
fn atempo_chain(speed: f64) -> String {
    let mut parts = Vec::new();
    let mut s = speed;
    while s > 2.0 { parts.push("atempo=2.0".to_string()); s /= 2.0; }
    while s < 0.5 { parts.push("atempo=0.5".to_string()); s /= 0.5; }
    parts.push(format!("atempo={s:.4}"));
    parts.join(",")
}

fn safe_color(c: &str) -> String {
    let ok = c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '#' || ch == '@' || ch == '.');
    if ok && !c.is_empty() { c.to_string() } else { "white".into() }
}

pub fn build_ffmpeg_args(req: &ExportRequest) -> Result<Vec<String>, String> {
    if req.clips.is_empty() {
        return Err("Oś czasu jest pusta".into());
    }
    if req.width < 16 || req.height < 16 || req.width > 7680 || req.height > 4320 || req.fps == 0 || req.fps > 120 {
        return Err("Nieprawidłowa rozdzielczość lub liczba klatek".into());
    }
    let (w, h, fps) = (req.width, req.height, req.fps);
    let mut args: Vec<String> = vec!["-y".into(), "-hide_banner".into()];
    for c in &req.clips {
        args.push("-i".into());
        args.push(c.path.clone());
    }
    if let Some(m) = &req.music {
        args.push("-i".into());
        args.push(m.path.clone());
    }

    let mut f: Vec<String> = Vec::new();
    let mut concat_in = String::new();
    for (i, c) in req.clips.iter().enumerate() {
        if c.duration <= 0.0 || c.speed <= 0.0 {
            return Err(format!("Klip {} ma nieprawidłową długość lub tempo", i + 1));
        }
        let out_len = c.duration / c.speed;
        let mut v = format!(
            "[{i}:v]trim=start={:.3}:duration={:.3},setpts=(PTS-STARTPTS)/{:.4},scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps}",
            c.start, c.duration, c.speed
        );
        if c.fade_in > 0.0 { v.push_str(&format!(",fade=t=in:st=0:d={:.3}", c.fade_in)); }
        if c.fade_out > 0.0 { v.push_str(&format!(",fade=t=out:st={:.3}:d={:.3}", (out_len - c.fade_out).max(0.0), c.fade_out)); }
        v.push_str(&format!("[v{i}]"));
        f.push(v);

        if c.has_audio {
            let mut a = format!(
                "[{i}:a]atrim=start={:.3}:duration={:.3},asetpts=PTS-STARTPTS,{},volume={:.3}",
                c.start, c.duration, atempo_chain(c.speed), c.volume
            );
            if c.fade_in > 0.0 { a.push_str(&format!(",afade=t=in:st=0:d={:.3}", c.fade_in)); }
            if c.fade_out > 0.0 { a.push_str(&format!(",afade=t=out:st={:.3}:d={:.3}", (out_len - c.fade_out).max(0.0), c.fade_out)); }
            a.push_str(&format!("[a{i}]"));
            f.push(a);
        } else {
            f.push(format!("anullsrc=r=48000:cl=stereo,atrim=duration={out_len:.3}[a{i}]"));
        }
        concat_in.push_str(&format!("[v{i}][a{i}]"));
    }
    f.push(format!("{concat_in}concat=n={}:v=1:a=1[vc][ac]", req.clips.len()));

    // Nakładki tekstowe.
    let mut vlabel = "vc".to_string();
    for (k, t) in req.texts.iter().enumerate() {
        let next = format!("vt{k}");
        f.push(format!(
            "[{vlabel}]drawtext=text='{}':fontsize={}:fontcolor={}:x=(w*{:.3}-text_w/2):y=(h*{:.3}-text_h/2):enable='between(t,{:.3},{:.3})':borderw=2:bordercolor=black@0.6[{next}]",
            escape_drawtext(&t.text), t.size.clamp(8, 400), safe_color(&t.color), t.x.clamp(0.0, 1.0), t.y.clamp(0.0, 1.0), t.start, t.end
        ));
        vlabel = next;
    }

    // Muzyka w tle.
    let mut alabel = "ac".to_string();
    if let Some(m) = &req.music {
        let mi = req.clips.len();
        f.push(format!("[{mi}:a]volume={:.3}[mus]", m.volume.clamp(0.0, 2.0)));
        f.push("[ac][mus]amix=inputs=2:duration=first:dropout_transition=0[aout]".into());
        alabel = "aout".into();
    }

    args.extend(["-filter_complex".into(), f.join(";")]);
    args.extend(["-map".into(), format!("[{vlabel}]"), "-map".into(), format!("[{alabel}]")]);
    args.extend([
        "-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(),
        "-crf".into(), req.crf.clamp(10, 40).to_string(), "-pix_fmt".into(), "yuv420p".into(),
        "-c:a".into(), "aac".into(), "-b:a".into(), "192k".into(),
        "-movflags".into(), "+faststart".into(),
    ]);
    args.push(req.output.clone());
    Ok(args)
}

/// Całkowity czas osi (s) — do obliczania procentów postępu.
pub fn timeline_length(req: &ExportRequest) -> f64 {
    req.clips.iter().map(|c| c.duration / c.speed.max(0.01)).sum()
}

fn parse_clock(s: &str) -> Option<f64> {
    let p: Vec<&str> = s.trim().split(':').collect();
    if p.len() != 3 { return None; }
    Some(p[0].parse::<f64>().ok()? * 3600.0 + p[1].parse::<f64>().ok()? * 60.0 + p[2].parse::<f64>().ok()?)
}

// ───────────────────────── komendy ─────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub has_audio: bool,
    pub has_video: bool,
}

#[tauri::command]
pub fn studio_check() -> bool {
    crate::backend::find_in_path("ffmpeg").is_some() && crate::backend::find_in_path("ffprobe").is_some()
}

#[tauri::command]
pub async fn studio_probe(path: String) -> Result<MediaInfo, String> {
    tokio::task::spawn_blocking(move || {
        let out = Command::new("ffprobe")
            .args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", &path])
            .output()
            .map_err(|e| format!("ffprobe: {e}"))?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
        }
        parse_probe(&String::from_utf8_lossy(&out.stdout))
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn parse_probe(json: &str) -> Result<MediaInfo, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let streams = v["streams"].as_array().cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s["codec_type"] == "video");
    let has_audio = streams.iter().any(|s| s["codec_type"] == "audio");
    let duration = v["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let fps = video
        .and_then(|s| s["avg_frame_rate"].as_str())
        .and_then(|r| r.split_once('/'))
        .and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok().filter(|d| *d != 0.0)?))
        .unwrap_or(30.0);
    Ok(MediaInfo {
        duration,
        width: video.and_then(|s| s["width"].as_u64()).unwrap_or(0) as u32,
        height: video.and_then(|s| s["height"].as_u64()).unwrap_or(0) as u32,
        fps,
        has_audio,
        has_video: video.is_some(),
    })
}

/// Miniatura klatki jako data URL (JPEG).
#[tauri::command]
pub async fn studio_thumbnail(path: String, time: f64) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let out = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", &format!("{:.3}", time.max(0.0)), "-i", &path,
                   "-frames:v", "1", "-vf", "scale=160:-1", "-f", "image2pipe", "-vcodec", "mjpeg", "-"])
            .output()
            .map_err(|e| format!("ffmpeg: {e}"))?;
        if out.stdout.is_empty() {
            return Err("nie udało się wyciąć klatki".into());
        }
        Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(out.stdout)))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize, Clone)]
struct Progress { percent: f64 }

#[tauri::command]
pub async fn studio_export(app: AppHandle, request: ExportRequest) -> Result<String, String> {
    let args = build_ffmpeg_args(&request)?;
    let total = timeline_length(&request).max(0.001);
    let output = request.output.clone();
    tokio::task::spawn_blocking(move || {
        let mut child = FfmpegCommand::new().args(&args).spawn().map_err(|e| format!("ffmpeg: {e}"))?;
        let mut last_error = String::new();
        for ev in child.iter().map_err(|e| e.to_string())? {
            match ev {
                FfmpegEvent::Progress(p) => {
                    if let Some(t) = parse_clock(&p.time) {
                        let _ = app.emit("studio:progress", Progress { percent: (t / total * 100.0).min(100.0) });
                    }
                }
                FfmpegEvent::Error(e) => last_error = e,
                FfmpegEvent::LogEOF => break,
                _ => {}
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        if status.success() {
            let _ = app.emit("studio:progress", Progress { percent: 100.0 });
            Ok(output)
        } else {
            Err(if last_error.is_empty() { "ffmpeg zakończył się błędem".into() } else { last_error })
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> ExportRequest {
        serde_json::from_str(r#"{
          "clips":[
            {"path":"a.mp4","start":1.0,"duration":4.0,"speed":2.0,"fadeIn":0.5},
            {"path":"b.mp4","start":0.0,"duration":3.0,"hasAudio":false}
          ],
          "texts":[{"text":"Cześć: 50%","start":0.0,"end":2.0}],
          "music":{"path":"m.mp3","volume":0.3},
          "width":1280,"height":720,"fps":30,"output":"out.mp4"}"#).unwrap()
    }

    #[test]
    fn builds_graph_with_all_features() {
        let a = build_ffmpeg_args(&req()).unwrap();
        let fc = &a[a.iter().position(|x| x == "-filter_complex").unwrap() + 1];
        assert!(fc.contains("concat=n=2:v=1:a=1[vc][ac]"));
        assert!(fc.contains("setpts=(PTS-STARTPTS)/2.0000"));
        assert!(fc.contains("fade=t=in:st=0:d=0.500"));
        assert!(fc.contains("anullsrc=r=48000:cl=stereo,atrim=duration=3.000[a1]"));
        assert!(fc.contains("drawtext=text='Cześć\\: 50\\%'"));
        assert!(fc.contains("[2:a]volume=0.300[mus]"));
        assert_eq!(a.last().unwrap(), "out.mp4");
        assert_eq!(a.iter().filter(|x| *x == "-i").count(), 3);
    }

    #[test]
    fn rejects_bad_input() {
        let mut r = req();
        r.clips.clear();
        assert!(build_ffmpeg_args(&r).is_err());
        let mut r = req();
        r.fps = 0;
        assert!(build_ffmpeg_args(&r).is_err());
    }

    #[test]
    fn tempo_chain_and_length() {
        assert_eq!(atempo_chain(4.0), "atempo=2.0,atempo=2.0000");
        assert!((timeline_length(&req()) - 5.0).abs() < 1e-9);
        assert_eq!(parse_clock("00:01:05.5"), Some(65.5));
    }

    #[test]
    fn parses_ffprobe_json() {
        let j = r#"{"streams":[{"codec_type":"video","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},{"codec_type":"audio"}],"format":{"duration":"12.5"}}"#;
        let m = parse_probe(j).unwrap();
        assert_eq!((m.width, m.height, m.has_audio), (1920, 1080, true));
        assert!((m.fps - 29.97).abs() < 0.01 && m.duration == 12.5);
    }
}
