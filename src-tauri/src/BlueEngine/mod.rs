use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// ───────────────────────── model projektu ─────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default = "default_color")]
    pub color: String,
    /// "rect" | "circle"
    #[serde(default = "default_shape")]
    pub shape: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub kind: String,
    #[serde(default)]
    pub params: HashMap<String, String>,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: u32,
    /// "exec" | "true" | "false"
    #[serde(default = "default_pin", rename = "fromPin")]
    pub from_pin: String,
    pub to: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Blueprint {
    #[serde(default)]
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    #[serde(default = "default_w")]
    pub width: u32,
    #[serde(default = "default_h")]
    pub height: u32,
    #[serde(default = "default_bg")]
    pub background: String,
    #[serde(default)]
    pub entities: Vec<Entity>,
    #[serde(default)]
    pub blueprint: Blueprint,
}

fn default_color() -> String { "#3b82f6".into() }
fn default_shape() -> String { "rect".into() }
fn default_pin() -> String { "exec".into() }
fn default_w() -> u32 { 800 }
fn default_h() -> u32 { 600 }
fn default_bg() -> String { "#0f172a".into() }

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

// ───────────────────────── walidacja i pomocnicze ─────────────────────────

fn is_ident(s: &str) -> bool {
    !s.is_empty() && s.len() <= 48 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_number(s: &str) -> bool {
    s.trim().parse::<f64>().is_ok()
}

fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

fn rs_str(s: &str) -> String {
    format!("{:?}", s)
}

fn param<'a>(n: &'a Node, k: &str) -> &'a str {
    n.params.get(k).map(|s| s.as_str()).unwrap_or("")
}

/// Wartość: liczba, `$zmienna` albo tekst.
fn js_val(s: &str) -> Result<String, String> {
    let s = s.trim();
    if let Some(v) = s.strip_prefix('$') {
        if !is_ident(v) {
            return Err(format!("nieprawidłowa nazwa zmiennej: {s}"));
        }
        Ok(format!("vars[{}]", js_str(v)))
    } else if is_number(s) {
        Ok(format!("({})", s.parse::<f64>().unwrap()))
    } else {
        Ok(js_str(s))
    }
}

fn rs_val(s: &str) -> Result<String, String> {
    let s = s.trim();
    if let Some(v) = s.strip_prefix('$') {
        if !is_ident(v) {
            return Err(format!("nieprawidłowa nazwa zmiennej: {s}"));
        }
        Ok(format!("var(&vars, {})", rs_str(v)))
    } else if is_number(s) {
        Ok(format!("{:?}f32", s.parse::<f32>().unwrap()))
    } else {
        Err(format!("cel Rust obsługuje tylko liczby i zmienne liczbowe, a nie tekst „{s}”"))
    }
}

const OPS: &[&str] = &["==", "!=", "<", "<=", ">", ">="];

fn is_event(kind: &str) -> bool {
    matches!(kind, "event_begin_play" | "event_tick" | "event_key_down" | "event_key_held")
}

fn validate(p: &Project) -> Result<(), String> {
    let mut names = HashSet::new();
    for e in &p.entities {
        if !is_ident(&e.name) {
            return Err(format!("Nieprawidłowa nazwa obiektu „{}” (litery, cyfry, _)", e.name));
        }
        if !names.insert(e.name.clone()) {
            return Err(format!("Zduplikowana nazwa obiektu „{}”", e.name));
        }
        if !e.color.chars().all(|c| c.is_ascii_alphanumeric() || c == '#') {
            return Err(format!("Nieprawidłowy kolor obiektu „{}”", e.name));
        }
    }
    let ids: HashSet<u32> = p.blueprint.nodes.iter().map(|n| n.id).collect();
    if ids.len() != p.blueprint.nodes.len() {
        return Err("Węzły mają zduplikowane identyfikatory".into());
    }
    for ed in &p.blueprint.edges {
        if !ids.contains(&ed.from) || !ids.contains(&ed.to) {
            return Err("Połączenie wskazuje nieistniejący węzeł".into());
        }
    }
    for n in &p.blueprint.nodes {
        for key in ["actor"] {
            if let Some(a) = n.params.get(key) {
                if !a.is_empty() && !names.contains(a) {
                    return Err(format!("Węzeł {} odwołuje się do nieistniejącego obiektu „{a}”", n.id));
                }
            }
        }
    }
    Ok(())
}

fn next_of<'a>(bp: &'a Blueprint, id: u32, pin: &str) -> Option<&'a Edge> {
    bp.edges.iter().find(|e| e.from == id && e.from_pin == pin)
}

fn node_by_id(bp: &Blueprint, id: u32) -> Option<&Node> {
    bp.nodes.iter().find(|n| n.id == id)
}

// ───────────────────────── generator JS ─────────────────────────

fn js_chain(bp: &Blueprint, start: Option<u32>, indent: usize, seen: &mut HashSet<u32>, out: &mut String) -> Result<(), String> {
    let pad = "  ".repeat(indent);
    let mut cur = start;
    while let Some(id) = cur {
        if !seen.insert(id) {
            return Err(format!("Pętla w grafie przy węźle {id} (użyj zdarzenia Tick zamiast cofania połączeń)"));
        }
        let n = node_by_id(bp, id).ok_or("brak węzła")?;
        match n.kind.as_str() {
            "print" => out.push_str(&format!("{pad}print({});\n", js_val(param(n, "text"))?)),
            "set_var" => {
                if !is_ident(param(n, "name")) { return Err(format!("Węzeł {id}: zła nazwa zmiennej")); }
                out.push_str(&format!("{pad}vars[{}] = {};\n", js_str(param(n, "name")), js_val(param(n, "value"))?));
            }
            "add_var" => {
                if !is_ident(param(n, "name")) { return Err(format!("Węzeł {id}: zła nazwa zmiennej")); }
                let k = js_str(param(n, "name"));
                out.push_str(&format!("{pad}vars[{k}] = (vars[{k}] || 0) + {};\n", js_val(param(n, "value"))?));
            }
            "move" => out.push_str(&format!(
                "{pad}actors[{}].x += {} * dt; actors[{}].y += {} * dt;\n",
                js_str(param(n, "actor")), js_val(param(n, "dx"))?, js_str(param(n, "actor")), js_val(param(n, "dy"))?)),
            "set_position" => out.push_str(&format!(
                "{pad}actors[{}].x = {}; actors[{}].y = {};\n",
                js_str(param(n, "actor")), js_val(param(n, "x"))?, js_str(param(n, "actor")), js_val(param(n, "y"))?)),
            "set_color" => out.push_str(&format!("{pad}actors[{}].color = {};\n", js_str(param(n, "actor")), js_str(param(n, "color")))),
            "branch" => {
                let op = param(n, "op");
                if !OPS.contains(&op) { return Err(format!("Węzeł {id}: nieznany operator „{op}”")); }
                let js_op = match op { "==" => "===", "!=" => "!==", o => o };
                out.push_str(&format!("{pad}if ({} {js_op} {}) {{\n", js_val(param(n, "left"))?, js_val(param(n, "right"))?));
                js_chain(bp, next_of(bp, id, "true").map(|e| e.to), indent + 1, &mut seen.clone(), out)?;
                out.push_str(&format!("{pad}}} else {{\n"));
                js_chain(bp, next_of(bp, id, "false").map(|e| e.to), indent + 1, &mut seen.clone(), out)?;
                out.push_str(&format!("{pad}}}\n"));
                return Ok(()); // gałęzie same prowadzą dalej
            }
            k if is_event(k) => return Err(format!("Zdarzenie {id} nie może być celem połączenia")),
            other => return Err(format!("Nieznany węzeł „{other}”")),
        }
        cur = next_of(bp, id, "exec").map(|e| e.to);
    }
    Ok(())
}

pub fn codegen_js(p: &Project) -> Result<Vec<GeneratedFile>, String> {
    validate(p)?;
    let bp = &p.blueprint;
    let mut begin = String::new();
    let mut tick = String::new();
    let mut key_down = String::new();
    let mut key_held = String::new();
    for n in bp.nodes.iter().filter(|n| is_event(&n.kind)) {
        let first = next_of(bp, n.id, "exec").map(|e| e.to);
        let mut body = String::new();
        js_chain(bp, first, 2, &mut HashSet::from([n.id]), &mut body)?;
        match n.kind.as_str() {
            "event_begin_play" => begin.push_str(&body),
            "event_tick" => tick.push_str(&body),
            "event_key_down" => key_down.push_str(&format!("  if (code === {}) {{\n{}  }}\n", js_str(param(n, "key")), body)),
            _ => key_held.push_str(&format!("    if (keys.has({})) {{\n{}    }}\n", js_str(param(n, "key")), body)),
        }
    }
    let actors: Vec<String> = p.entities.iter().map(|e| format!(
        "  {}: {{ x: {}, y: {}, w: {}, h: {}, color: {}, shape: {} }}",
        js_str(&e.name), e.x, e.y, e.w, e.h, js_str(&e.color), js_str(&e.shape))).collect();

    let game = format!(r#"'use strict';
// Wygenerowane przez Blue Engine z grafu Blueprintów — edytuj graf, nie ten plik.
const canvas = document.getElementById('game');
const ctx = canvas.getContext('2d');
const actors = {{
{actors}
}};
const vars = {{}};
const keys = new Set();
function print(t) {{
  console.log(t);
  try {{ parent.postMessage({{ type: 'blue-engine-log', text: String(t) }}, '*'); }} catch (_) {{}}
}}
function onBeginPlay() {{
{begin}}}
function onTick(dt) {{
{tick}{key_held_wrapped}}}
function onKeyDown(code) {{
{key_down}}}
addEventListener('keydown', (e) => {{ if (!e.repeat) {{ keys.add(e.code); onKeyDown(e.code); }} if (e.code.startsWith('Arrow') || e.code === 'Space') e.preventDefault(); }});
addEventListener('keyup', (e) => keys.delete(e.code));
function draw() {{
  ctx.fillStyle = {bg};
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  for (const a of Object.values(actors)) {{
    ctx.fillStyle = a.color;
    if (a.shape === 'circle') {{ ctx.beginPath(); ctx.ellipse(a.x + a.w / 2, a.y + a.h / 2, a.w / 2, a.h / 2, 0, 0, Math.PI * 2); ctx.fill(); }}
    else ctx.fillRect(a.x, a.y, a.w, a.h);
  }}
}}
let last = performance.now();
function frame(now) {{
  const dt = Math.min(0.05, (now - last) / 1000);
  last = now;
  onTick(dt);
  draw();
  requestAnimationFrame(frame);
}}
onBeginPlay();
requestAnimationFrame(frame);
"#,
        actors = actors.join(",\n"),
        begin = indent_block(&begin, 0),
        tick = indent_block(&tick, 0),
        key_held_wrapped = if key_held.is_empty() { String::new() } else { key_held.clone() },
        key_down = key_down,
        bg = js_str(&p.background),
    );

    let html = format!(r#"<!doctype html>
<html lang="pl"><head><meta charset="utf-8"><title>{title}</title>
<style>html,body{{margin:0;background:#000;display:flex;align-items:center;justify-content:center;height:100%}}canvas{{max-width:100%;max-height:100%}}</style>
</head><body><canvas id="game" width="{w}" height="{h}"></canvas><script src="game.js"></script></body></html>
"#, title = html_escape(&p.name), w = p.width, h = p.height);

    Ok(vec![
        GeneratedFile { path: "index.html".into(), content: html },
        GeneratedFile { path: "game.js".into(), content: game },
    ])
}

fn indent_block(s: &str, _n: usize) -> String { s.to_string() }

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ───────────────────────── generator Rust (macroquad) ─────────────────────────

fn mq_key(code: &str) -> Result<String, String> {
    let k = match code {
        "ArrowLeft" => "Left".to_string(),
        "ArrowRight" => "Right".to_string(),
        "ArrowUp" => "Up".to_string(),
        "ArrowDown" => "Down".to_string(),
        "Space" => "Space".to_string(),
        "Enter" => "Enter".to_string(),
        "Escape" => "Escape".to_string(),
        c if c.starts_with("Key") && c.len() == 4 => c[3..].to_string(),
        c if c.starts_with("Digit") && c.len() == 6 => format!("Key{}", &c[5..]),
        other => return Err(format!("Klawisz „{other}” nie jest obsługiwany w celu Rust")),
    };
    Ok(format!("KeyCode::{k}"))
}

fn rs_chain(bp: &Blueprint, start: Option<u32>, indent: usize, seen: &mut HashSet<u32>, out: &mut String) -> Result<(), String> {
    let pad = "    ".repeat(indent);
    let mut cur = start;
    while let Some(id) = cur {
        if !seen.insert(id) {
            return Err(format!("Pętla w grafie przy węźle {id}"));
        }
        let n = node_by_id(bp, id).ok_or("brak węzła")?;
        match n.kind.as_str() {
            "print" => {
                let t = param(n, "text");
                if t.starts_with('$') || is_number(t) {
                    out.push_str(&format!("{pad}println!(\"{{}}\", {});\n", rs_val(t)?));
                } else {
                    out.push_str(&format!("{pad}println!(\"{{}}\", {});\n", rs_str(t)));
                }
            }
            "set_var" => {
                if !is_ident(param(n, "name")) { return Err(format!("Węzeł {id}: zła nazwa zmiennej")); }
                out.push_str(&format!("{pad}vars.insert({}.to_string(), {});\n", rs_str(param(n, "name")), rs_val(param(n, "value"))?));
            }
            "add_var" => {
                if !is_ident(param(n, "name")) { return Err(format!("Węzeł {id}: zła nazwa zmiennej")); }
                out.push_str(&format!("{pad}*vars.entry({}.to_string()).or_insert(0.0) += {};\n", rs_str(param(n, "name")), rs_val(param(n, "value"))?));
            }
            "move" => out.push_str(&format!(
                "{pad}if let Some(a) = actors.get_mut({}) {{ a.x += {} * dt; a.y += {} * dt; }}\n",
                rs_str(param(n, "actor")), rs_val(param(n, "dx"))?, rs_val(param(n, "dy"))?)),
            "set_position" => out.push_str(&format!(
                "{pad}if let Some(a) = actors.get_mut({}) {{ a.x = {}; a.y = {}; }}\n",
                rs_str(param(n, "actor")), rs_val(param(n, "x"))?, rs_val(param(n, "y"))?)),
            "set_color" => out.push_str(&format!(
                "{pad}if let Some(a) = actors.get_mut({}) {{ a.color = parse_color({}); }}\n",
                rs_str(param(n, "actor")), rs_str(param(n, "color")))),
            "branch" => {
                let op = param(n, "op");
                if !OPS.contains(&op) { return Err(format!("Węzeł {id}: nieznany operator „{op}”")); }
                out.push_str(&format!("{pad}if {} {op} {} {{\n", rs_val(param(n, "left"))?, rs_val(param(n, "right"))?));
                rs_chain(bp, next_of(bp, id, "true").map(|e| e.to), indent + 1, &mut seen.clone(), out)?;
                out.push_str(&format!("{pad}}} else {{\n"));
                rs_chain(bp, next_of(bp, id, "false").map(|e| e.to), indent + 1, &mut seen.clone(), out)?;
                out.push_str(&format!("{pad}}}\n"));
                return Ok(());
            }
            k if is_event(k) => return Err(format!("Zdarzenie {id} nie może być celem połączenia")),
            other => return Err(format!("Nieznany węzeł „{other}”")),
        }
        cur = next_of(bp, id, "exec").map(|e| e.to);
    }
    Ok(())
}

pub fn codegen_rust(p: &Project) -> Result<Vec<GeneratedFile>, String> {
    validate(p)?;
    let bp = &p.blueprint;
    let (mut begin, mut tick, mut down, mut held) = (String::new(), String::new(), String::new(), String::new());
    for n in bp.nodes.iter().filter(|n| is_event(&n.kind)) {
        let first = next_of(bp, n.id, "exec").map(|e| e.to);
        let mut body = String::new();
        rs_chain(bp, first, 3, &mut HashSet::from([n.id]), &mut body)?;
        match n.kind.as_str() {
            "event_begin_play" => begin.push_str(&body.replace("            ", "        ")),
            "event_tick" => tick.push_str(&body.replace("            ", "        ")),
            "event_key_down" => down.push_str(&format!("        if is_key_pressed({}) {{\n{}        }}\n", mq_key(param(n, "key"))?, body)),
            _ => held.push_str(&format!("        if is_key_down({}) {{\n{}        }}\n", mq_key(param(n, "key"))?, body)),
        }
    }
    let actors: Vec<String> = p.entities.iter().map(|e| format!(
        "        ({}.to_string(), Actor {{ x: {:?}f32, y: {:?}f32, w: {:?}f32, h: {:?}f32, color: parse_color({}), circle: {} }}),",
        rs_str(&e.name), e.x as f32, e.y as f32, e.w as f32, e.h as f32, rs_str(&e.color), e.shape == "circle")).collect();

    let main = format!(r#"// Wygenerowane przez Blue Engine z grafu Blueprintów — edytuj graf, nie ten plik.
use macroquad::prelude::*;
use std::collections::HashMap;

struct Actor {{ x: f32, y: f32, w: f32, h: f32, color: Color, circle: bool }}

fn parse_color(s: &str) -> Color {{
    let h = s.trim_start_matches('#');
    if h.len() == 6 {{
        if let Ok(v) = u32::from_str_radix(h, 16) {{
            return Color::from_rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255);
        }}
    }}
    match s {{ "red" => RED, "green" => GREEN, "blue" => BLUE, "yellow" => YELLOW, "white" => WHITE, "black" => BLACK, _ => GRAY }}
}}

fn var(vars: &HashMap<String, f32>, k: &str) -> f32 {{ vars.get(k).copied().unwrap_or(0.0) }}

fn window_conf() -> Conf {{
    Conf {{ window_title: {title}.to_owned(), window_width: {w}, window_height: {h}, ..Default::default() }}
}}

#[macroquad::main(window_conf)]
async fn main() {{
    let mut actors: HashMap<String, Actor> = HashMap::from([
{actors}
    ]);
    let mut vars: HashMap<String, f32> = HashMap::new();
    let bg = parse_color({bg});
    {{
        let dt = 0.0f32;
        let _ = dt;
{begin}    }}
    loop {{
        let dt = get_frame_time();
{tick}{held}{down}        clear_background(bg);
        for a in actors.values() {{
            if a.circle {{ draw_ellipse(a.x + a.w / 2.0, a.y + a.h / 2.0, a.w / 2.0, a.h / 2.0, 0.0, a.color); }}
            else {{ draw_rectangle(a.x, a.y, a.w, a.h, a.color); }}
        }}
        next_frame().await;
    }}
}}
"#, title = rs_str(&p.name), w = p.width, h = p.height, actors = actors.join("\n"), bg = rs_str(&p.background),
        begin = begin, tick = tick, held = held, down = down);

    let crate_name: String = p.name.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let crate_name = if crate_name.is_empty() || crate_name.starts_with(|c: char| c.is_ascii_digit()) { format!("game_{crate_name}") } else { crate_name };
    let cargo = format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nmacroquad = \"0.4\"\n\n[profile.release]\nopt-level = 3\nlto = true\n");
    Ok(vec![
        GeneratedFile { path: "Cargo.toml".into(), content: cargo },
        GeneratedFile { path: "src/main.rs".into(), content: main },
    ])
}

pub fn generate(p: &Project, target: &str) -> Result<Vec<GeneratedFile>, String> {
    match target {
        "web" => codegen_js(p),
        "linux" | "windows" => codegen_rust(p),
        other => Err(format!("Nieznany cel: {other}")),
    }
}

// ───────────────────────── komendy ─────────────────────────

fn project_dir(dir: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(dir);
    if !p.is_absolute() || dir.contains("..") {
        return Err("Wymagana bezwzględna ścieżka bez „..”".into());
    }
    Ok(p)
}

fn target_subdir(target: &str) -> &'static str {
    if target == "web" { "web" } else { "native" }
}

#[tauri::command]
pub fn engine_generate(project: Project, target: String) -> Result<Vec<GeneratedFile>, String> {
    generate(&project, &target)
}

#[tauri::command]
pub fn engine_save_project(dir: String, project: Project) -> Result<(), String> {
    let d = project_dir(&dir)?;
    std::fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?;
    let tmp = d.join("blue-engine.json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, d.join("blue-engine.json")).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn engine_load_project(dir: String) -> Result<Project, String> {
    let d = project_dir(&dir)?;
    let s = std::fs::read_to_string(d.join("blue-engine.json")).map_err(|e| format!("Brak projektu: {e}"))?;
    serde_json::from_str(&s).map_err(|e| format!("Uszkodzony projekt: {e}"))
}

fn write_files(root: &Path, files: &[GeneratedFile]) -> Result<(), String> {
    for f in files {
        let path = root.join(&f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, &f.content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[derive(Serialize)]
pub struct BuildResult {
    pub output: String,
}

async fn stream_cmd(app: &AppHandle, mut cmd: Command) -> Result<bool, String> {
    cmd.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let out = child.stdout.take().ok_or("brak stdout")?;
    let err = child.stderr.take().ok_or("brak stderr")?;
    let a1 = app.clone();
    let a2 = app.clone();
    let t1 = tokio::spawn(async move {
        let mut l = BufReader::new(out).lines();
        while let Ok(Some(line)) = l.next_line().await { let _ = a1.emit("engine:log", line); }
    });
    let t2 = tokio::spawn(async move {
        let mut l = BufReader::new(err).lines();
        while let Ok(Some(line)) = l.next_line().await { let _ = a2.emit("engine:log", line); }
    });
    let status = child.wait().await.map_err(|e| e.to_string())?;
    let _ = tokio::join!(t1, t2);
    Ok(status.success())
}

#[tauri::command]
pub async fn engine_build(app: AppHandle, dir: String, project: Project, target: String) -> Result<BuildResult, String> {
    let root = project_dir(&dir)?.join("build").join(target_subdir(&target));
    let files = generate(&project, &target)?;
    write_files(&root, &files)?;
    if target == "web" {
        return Ok(BuildResult { output: root.join("index.html").to_string_lossy().into_owned() });
    }
    if crate::backend::find_in_path("cargo").is_none() {
        return Err("Nie znaleziono `cargo` — zainstaluj Rust (rustup), aby budować natywnie.".into());
    }
    let mut cmd = Command::new("cargo");
    cmd.current_dir(&root).args(["build", "--release"]);
    if target == "windows" {
        cmd.args(["--target", "x86_64-pc-windows-gnu"]);
    }
    let _ = app.emit("engine:log", format!("▶ cargo build --release ({target})"));
    if !stream_cmd(&app, cmd).await? {
        return Err(if target == "windows" {
            "Budowanie nie powiodło się. Dla Windows potrzebne: `rustup target add x86_64-pc-windows-gnu` oraz mingw-w64.".into()
        } else {
            "Budowanie nie powiodło się (szczegóły w konsoli).".into()
        });
    }
    let crate_name = files[0].content.lines().find_map(|l| l.strip_prefix("name = \"")).and_then(|l| l.strip_suffix('"')).unwrap_or("game").to_string();
    let bin = if target == "windows" {
        root.join(format!("target/x86_64-pc-windows-gnu/release/{crate_name}.exe"))
    } else {
        root.join(format!("target/release/{crate_name}"))
    };
    Ok(BuildResult { output: bin.to_string_lossy().into_owned() })
}

#[tauri::command]
pub fn engine_run(output: String) -> Result<(), String> {
    let p = PathBuf::from(&output);
    if !p.is_absolute() || !p.exists() {
        return Err("Najpierw zbuduj projekt".into());
    }
    if output.ends_with(".html") {
        std::process::Command::new("xdg-open").arg(&p).spawn().map(|_| ()).map_err(|e| e.to_string())
    } else if output.ends_with(".exe") {
        Err("Plik .exe uruchomisz na Windows (albo przez Wine)".into())
    } else {
        std::process::Command::new(&p).current_dir(p.parent().unwrap_or(Path::new("/"))).spawn().map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: u32, kind: &str, params: &[(&str, &str)]) -> Node {
        Node { id, kind: kind.into(), params: params.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), x: 0.0, y: 0.0 }
    }
    fn edge(from: u32, pin: &str, to: u32) -> Edge { Edge { from, from_pin: pin.into(), to } }

    fn sample() -> Project {
        Project {
            name: "Demo".into(), width: 640, height: 480, background: "#000000".into(),
            entities: vec![Entity { name: "player".into(), x: 10.0, y: 20.0, w: 32.0, h: 32.0, color: "#ff0000".into(), shape: "rect".into() }],
            blueprint: Blueprint {
                nodes: vec![
                    node(1, "event_begin_play", &[]),
                    node(2, "print", &[("text", "Start!")]),
                    node(3, "event_key_held", &[("key", "ArrowRight")]),
                    node(4, "move", &[("actor", "player"), ("dx", "200"), ("dy", "0")]),
                    node(5, "event_tick", &[]),
                    node(6, "branch", &[("left", "$score"), ("op", ">="), ("right", "3")]),
                    node(7, "set_color", &[("actor", "player"), ("color", "green")]),
                    node(8, "add_var", &[("name", "score"), ("value", "1")]),
                ],
                edges: vec![edge(1, "exec", 2), edge(3, "exec", 4), edge(5, "exec", 8), edge(8, "exec", 6), edge(6, "true", 7)],
            },
        }
    }

    #[test]
    fn js_codegen_has_all_parts() {
        let f = codegen_js(&sample()).unwrap();
        let js = &f.iter().find(|x| x.path == "game.js").unwrap().content;
        assert!(js.contains("print(\"Start!\");"));
        assert!(js.contains("actors[\"player\"].x += (200) * dt;"));
        assert!(js.contains("if (keys.has(\"ArrowRight\"))"));
        assert!(js.contains("vars[\"score\"] = (vars[\"score\"] || 0) + (1);"));
        assert!(js.contains("if (vars[\"score\"] >= (3)) {"));
        assert!(js.contains("actors[\"player\"].color = \"green\";"));
        assert!(f.iter().any(|x| x.path == "index.html" && x.content.contains("width=\"640\"")));
    }

    #[test]
    fn rust_codegen_has_all_parts() {
        let f = codegen_rust(&sample());
        // print z tekstem jest dozwolony, ale `set_color` z nazwą koloru też — brak błędu:
        let f = f.unwrap();
        let rs = &f.iter().find(|x| x.path == "src/main.rs").unwrap().content;
        assert!(rs.contains("is_key_down(KeyCode::Right)"));
        assert!(rs.contains("a.x += 200.0f32 * dt"));
        assert!(rs.contains("if var(&vars, \"score\") >= 3.0f32 {"));
        assert!(rs.contains("macroquad::main(window_conf)"));
        assert!(f.iter().any(|x| x.path == "Cargo.toml" && x.content.contains("name = \"demo\"")));
    }

    #[test]
    fn detects_cycles_and_bad_refs() {
        let mut p = sample();
        p.blueprint.edges.push(edge(2, "exec", 2));
        assert!(codegen_js(&p).unwrap_err().contains("Pętla"));
        let mut p = sample();
        p.blueprint.nodes.push(node(9, "move", &[("actor", "ghost"), ("dx", "1"), ("dy", "1")]));
        assert!(codegen_js(&p).unwrap_err().contains("nieistniejącego"));
    }

    #[test]
    fn rejects_injection() {
        let mut p = sample();
        p.entities[0].name = "x\"];alert(1);//".into();
        assert!(codegen_js(&p).is_err());
        let mut p = sample();
        p.blueprint.nodes[1].params.insert("text".into(), "\"); alert(1); (\"".into());
        let js = codegen_js(&p).unwrap();
        // tekst trafia do JSON-owego literału — cudzysłowy są escapowane
        assert!(js.iter().any(|f| f.content.contains("\\\"); alert(1)")));
    }

    #[test]
    fn rust_target_rejects_text_values_and_unknown_keys() {
        let mut p = sample();
        p.blueprint.nodes[3].params.insert("dx".into(), "szybko".into());
        assert!(codegen_rust(&p).unwrap_err().contains("tylko liczby"));
        assert!(mq_key("F13").is_err());
        assert_eq!(mq_key("KeyA").unwrap(), "KeyCode::A");
    }

    #[test]
    fn path_validation() {
        assert!(project_dir("relative/path").is_err());
        assert!(project_dir("/home/u/../etc").is_err());
        assert!(project_dir("/home/u/game").is_ok());
    }
}
