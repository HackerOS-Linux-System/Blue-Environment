#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const DB_JSON: &str = include_str!("../../resources/linguist/languages.json");

pub const MAX_FILES: usize = 100_000;
pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const TIME_BUDGET: Duration = Duration::from_secs(6);
const HEAD_BYTES: usize = 64 * 1024;
const FALLBACK_COLOR: &str = "#8f8fa3";

// ── types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LangType {
    Programming,
    Markup,
    Data,
    Prose,
}

impl LangType {
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "markup" => Self::Markup,
            "data" => Self::Data,
            "prose" => Self::Prose,
            _ => Self::Programming,
        }
    }
    fn counts_by_default(self) -> bool {
        matches!(self, Self::Programming | Self::Markup)
    }
}

#[derive(Debug, Clone)]
pub struct LangDef {
    pub name: String,
    pub kind: LangType,
    pub color: String,
    pub custom: bool,
    /// Project icon id for the HackerOS languages (`hsharp`, `hackerscript`, `hackerlang`).
    pub brand: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DbLang {
    n: String,
    t: String,
    #[serde(default)]
    c: String,
    #[serde(default)]
    e: Vec<String>,
    #[serde(default)]
    f: Vec<String>,
    #[serde(default)]
    i: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Db {
    languages: Vec<DbLang>,
    #[serde(default)]
    popular: Vec<String>,
    #[serde(default)]
    vendored: Vec<String>,
    #[serde(default)]
    documentation: Vec<String>,
}

/// A user / project supplied language (`languages.json`).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomLanguage {
    pub name: String,
    #[serde(default = "default_type", rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
    #[serde(default)]
    pub interpreters: Vec<String>,
}

fn default_type() -> String {
    "programming".to_string()
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CustomFile {
    List(Vec<CustomLanguage>),
    Wrapped { languages: Vec<CustomLanguage> },
}

// ── registry ───────────────────────────────────────────────────────────────

pub struct Registry {
    langs: Vec<LangDef>,
    by_ext: HashMap<String, Vec<usize>>,
    by_filename: HashMap<String, usize>,
    by_interpreter: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    popular_rank: HashMap<usize, usize>,
    vendored: Vec<Regex>,
    documentation: Vec<Regex>,
}

/// Where ambiguity is resolved by taste rather than by content.
const PREFERRED: &[(&str, &str)] = &[
    (".h", "C"),
    (".m", "Objective-C"),
    (".pl", "Perl"),
    (".pm", "Perl"),
    (".ts", "TypeScript"),
    (".r", "R"),
    (".v", "Verilog"),
    (".sql", "SQL"),
    (".md", "Markdown"),
    (".json", "JSON"),
    (".yml", "YAML"),
    (".yaml", "YAML"),
    (".txt", "Text"),
    (".cs", "C#"),
    (".fs", "F#"),
    (".rs", "Rust"),
    (".cls", "TeX"),
    (".html", "HTML"),
    (".xml", "XML"),
    (".tsx", "TSX"),
    (".gd", "GDScript"),
    (".lua", "Lua"),
    (".php", "PHP"),
    (".sh", "Shell"),
    (".toml", "TOML"),
];

/// The HackerOS family: same names, colours and icons as bit.io.
fn builtin_custom() -> Vec<(CustomLanguage, Option<&'static str>)> {
    let mk = |name: &str, kind: &str, color: &str, ext: &[&str], files: &[&str]| CustomLanguage {
        name: name.into(),
        kind: kind.into(),
        color: color.into(),
        extensions: ext.iter().map(|s| s.to_string()).collect(),
        filenames: files.iter().map(|s| s.to_string()).collect(),
        interpreters: Vec::new(),
    };
    vec![
        (mk("H#", "programming", "#9c1120", &[".h#"], &[]), Some("hsharp")),
        (mk("H# Interface", "programming", "#c23b4e", &[".h#i"], &[]), Some("hsharp")),
        (mk("HackerScript", "programming", "#4b4b55", &[".hcs"], &[]), Some("hackerscript")),
        (mk("Hacker Lang", "programming", "#8250df", &[".hl"], &[]), Some("hackerlang")),
        (mk("hk", "markup", "#ffffff", &[".hk"], &["Bit.hk"]), None),
        (mk("Blue", "markup", "#1e6fff", &[".blue"], &[]), None),
    ]
}

fn normalize_ext(e: &str) -> Option<String> {
    let e = e.trim().to_lowercase();
    let e = e.trim_start_matches('*');
    if e.is_empty() || e.len() > 32 || e.contains('/') || e.contains('\\') || e.contains(char::is_whitespace) {
        return None;
    }
    Some(if e.starts_with('.') { e.to_string() } else { format!(".{e}") })
}

fn valid_color(c: &str) -> Option<String> {
    let c = c.trim();
    let hex = c.strip_prefix('#')?;
    ((hex.len() == 3 || hex.len() == 6) && hex.chars().all(|ch| ch.is_ascii_hexdigit())).then(|| c.to_lowercase())
}

/// Parses a `languages.json` (array or `{ "languages": [...] }`) defensively:
/// bad entries are dropped, never fatal.
pub fn parse_custom_languages(json: &str) -> Vec<CustomLanguage> {
    let parsed: Option<Vec<CustomLanguage>> = match serde_json::from_str::<CustomFile>(json) {
        Ok(CustomFile::List(v)) | Ok(CustomFile::Wrapped { languages: v }) => Some(v),
        Err(_) => None,
    };
    parsed
        .unwrap_or_default()
        .into_iter()
        .take(200)
        .filter_map(|mut l| {
            l.name = l.name.trim().to_string();
            if l.name.is_empty() || l.name.len() > 64 {
                return None;
            }
            l.extensions = l.extensions.iter().filter_map(|e| normalize_ext(e)).take(64).collect();
            l.filenames.retain(|f| !f.is_empty() && f.len() <= 128 && !f.contains('/'));
            l.filenames.truncate(64);
            l.interpreters.truncate(16);
            Some(l)
        })
        .collect()
}

impl Registry {
    fn base() -> Registry {
        let db: Db = serde_json::from_str(DB_JSON).unwrap_or(Db { languages: Vec::new(), popular: Vec::new(), vendored: Vec::new(), documentation: Vec::new() });
        let mut r = Registry {
            langs: Vec::new(),
            by_ext: HashMap::new(),
            by_filename: HashMap::new(),
            by_interpreter: HashMap::new(),
            by_name: HashMap::new(),
            popular_rank: HashMap::new(),
            vendored: compile_all(&db.vendored),
            documentation: compile_all(&db.documentation),
        };
        for l in &db.languages {
            let idx = r.langs.len();
            r.langs.push(LangDef {
                name: l.n.clone(),
                kind: LangType::parse(&l.t),
                color: valid_color(&l.c).unwrap_or_else(|| FALLBACK_COLOR.to_string()),
                custom: false,
                brand: None,
            });
            r.by_name.insert(l.n.to_lowercase(), idx);
            for e in &l.e {
                r.by_ext.entry(e.clone()).or_default().push(idx);
            }
            for f in &l.f {
                r.by_filename.entry(f.clone()).or_insert(idx);
            }
            for i in &l.i {
                r.by_interpreter.entry(i.clone()).or_insert(idx);
            }
        }
        for (rank, name) in db.popular.iter().enumerate() {
            if let Some(&idx) = r.by_name.get(&name.to_lowercase()) {
                r.popular_rank.insert(idx, rank);
            }
        }
        r
    }

    /// Adds languages that take precedence over everything already known for
    /// the extensions / filenames / interpreters they claim.
    fn add_custom(&mut self, defs: &[(CustomLanguage, Option<&str>)]) {
        for (d, brand) in defs {
            let idx = match self.by_name.get(&d.name.to_lowercase()) {
                Some(&i) => {
                    // Redefining a known language replaces its look, keeps its claims.
                    self.langs[i].custom = true;
                    if let Some(c) = valid_color(&d.color) {
                        self.langs[i].color = c;
                    }
                    self.langs[i].kind = LangType::parse(&d.kind);
                    i
                }
                None => {
                    let i = self.langs.len();
                    self.langs.push(LangDef {
                        name: d.name.clone(),
                        kind: LangType::parse(&d.kind),
                        color: valid_color(&d.color).unwrap_or_else(|| FALLBACK_COLOR.to_string()),
                        custom: true,
                        brand: brand.map(|b| b.to_string()),
                    });
                    self.by_name.insert(d.name.to_lowercase(), i);
                    i
                }
            };
            if let Some(b) = brand {
                self.langs[idx].brand = Some((*b).to_string());
            }
            for e in &d.extensions {
                if let Some(e) = normalize_ext(e) {
                    self.by_ext.insert(e, vec![idx]); // exclusive: overrides the database
                }
            }
            for f in &d.filenames {
                self.by_filename.insert(f.clone(), idx);
            }
            for i in &d.interpreters {
                self.by_interpreter.insert(i.clone(), idx);
            }
        }
    }

    pub fn with_custom(extra: &[CustomLanguage]) -> Registry {
        let mut r = Registry::base();
        let builtin = builtin_custom();
        let b: Vec<(CustomLanguage, Option<&str>)> = builtin.into_iter().collect();
        r.add_custom(&b);
        let e: Vec<(CustomLanguage, Option<&str>)> = extra.iter().cloned().map(|d| (d, None)).collect();
        r.add_custom(&e);
        r
    }

    #[cfg(test)]
    pub fn language_count(&self) -> usize {
        self.langs.len()
    }

    pub fn def(&self, idx: usize) -> &LangDef {
        &self.langs[idx]
    }

    pub fn find_by_name(&self, name: &str) -> Option<usize> {
        self.by_name.get(&name.trim().to_lowercase()).copied()
    }

    fn pick_ambiguous(&self, ext: &str, cands: &[usize], head: &str) -> usize {
        if let Some(i) = heuristic(ext, head).and_then(|n| self.find_by_name(n)) {
            if cands.contains(&i) {
                return i;
            }
        }
        if let Some((_, pref)) = PREFERRED.iter().find(|(e, _)| *e == ext) {
            if let Some(i) = self.find_by_name(pref) {
                if cands.contains(&i) {
                    return i;
                }
            }
        }
        cands.iter().copied().min_by_key(|i| (self.popular_rank.get(i).copied().unwrap_or(usize::MAX), *i)).unwrap_or(cands[0])
    }

    /// Language of a file from its name (+ content for ambiguous cases).
    pub fn detect(&self, file_name: &str, head: &str) -> Option<usize> {
        if let Some(&i) = self.by_filename.get(file_name) {
            return Some(i);
        }
        let lower = file_name.to_lowercase();
        // Longest matching extension first: `a.d.ts` → ".d.ts" then ".ts".
        let mut search_from = 0;
        while let Some(rel) = lower[search_from..].find('.') {
            let dot = search_from + rel;
            if dot > 0 || lower.len() > 1 {
                let ext = &lower[dot..];
                if ext.len() > 1 {
                    if let Some(c) = self.by_ext.get(ext) {
                        return Some(if c.len() == 1 { c[0] } else { self.pick_ambiguous(ext, c, head) });
                    }
                }
            }
            search_from = dot + 1;
        }
        // No extension match: shebang.
        let first = head.lines().next().unwrap_or("");
        if let Some(rest) = first.strip_prefix("#!") {
            let mut parts = rest.split_whitespace();
            let prog = parts.next().unwrap_or("");
            let mut name = prog.rsplit('/').next().unwrap_or("");
            if name == "env" {
                name = parts.find(|p| !p.starts_with('-')).unwrap_or("");
            }
            if let Some(&i) = self.by_interpreter.get(name) {
                return Some(i);
            }
            let stripped = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
            if let Some(&i) = self.by_interpreter.get(stripped) {
                return Some(i);
            }
        }
        None
    }
}

fn compile_all(patterns: &[String]) -> Vec<Regex> {
    // Patterns are Ruby regexps; the few that use Ruby-only syntax simply fail
    // to compile here and are skipped.
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
}

/// Content heuristics for the extensions every language wants.
fn heuristic(ext: &str, head: &str) -> Option<&'static str> {
    match ext {
        ".h" => {
            if head.contains("@interface") || head.contains("@implementation") || head.contains("#import <Foundation") || head.contains("#import <UIKit") {
                Some("Objective-C")
            } else if cpp_re().is_match(head) {
                Some("C++")
            } else {
                Some("C")
            }
        }
        ".m" => {
            if head.contains("@interface") || head.contains("@implementation") || head.contains("#import") || head.contains("@end") {
                Some("Objective-C")
            } else if matlab_re().is_match(head) {
                Some("MATLAB")
            } else {
                Some("Objective-C")
            }
        }
        ".pl" => {
            if head.contains("use strict") || head.contains("use warnings") || head.contains("my $") || head.starts_with("#!/usr/bin/perl") {
                Some("Perl")
            } else if prolog_re().is_match(head) {
                Some("Prolog")
            } else {
                Some("Perl")
            }
        }
        ".ts" => {
            let t = head.trim_start();
            if t.starts_with("<?xml") || t.starts_with("<TS") || t.starts_with("<!DOCTYPE TS") {
                Some("XML")
            } else {
                Some("TypeScript")
            }
        }
        ".r" => {
            if head.trim_start().to_lowercase().starts_with("rebol") {
                Some("Rebol")
            } else {
                Some("R")
            }
        }
        ".v" => {
            if coq_re().is_match(head) {
                Some("Coq")
            } else {
                Some("Verilog")
            }
        }
        _ => None,
    }
}

fn cpp_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?m)\b(namespace\s+\w+|template\s*<|class\s+\w+\s*[:{]|public:|private:|protected:|std::|#include\s*<(iostream|string|vector|map|memory|algorithm|cstdint|cstdio)>)").unwrap_or_else(|_| never())
    })
}
fn matlab_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s*(function\s|%[%\s]|end\s*$)").unwrap_or_else(|_| never()))
}
fn prolog_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^(:-|\?-|[a-z]\w*\(.*\)\s*:-)").unwrap_or_else(|_| never()))
}
fn coq_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^\s*(Require|Definition|Lemma|Theorem|Inductive|Fixpoint)\b").unwrap_or_else(|_| never()))
}
/// A regex that matches nothing — the fallback if one of the literals above
/// ever failed to compile (they are covered by tests, so this is belt and braces).
fn never() -> Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| match Regex::new("[^\\s\\S]") {
        Ok(r) => r,
        Err(_) => match Regex::new("a^") {
            Ok(r) => r,
            Err(e) => {
                tracing::error!("regex engine unusable: {e}");
                std::process::abort()
            }
        },
    })
    .clone()
}

// ── gitattributes / gitignore globbing ─────────────────────────────────────

/// Translates a gitignore / gitattributes glob to a regex over `/`-separated
/// relative paths. A pattern also matches everything beneath a matching directory.
pub fn glob_to_regex(pattern: &str) -> Option<Regex> {
    let mut pat = pattern.trim();
    if pat.is_empty() {
        return None;
    }
    let dir_only = pat.ends_with('/');
    pat = pat.trim_end_matches('/');
    let anchored = pat.starts_with('/') || pat.trim_start_matches('/').contains('/');
    let pat = pat.trim_start_matches('/');
    let c: Vec<char> = pat.chars().collect();
    let mut re = String::from("^");
    if !anchored {
        re.push_str("(?:.*/)?");
    }
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '*' => {
                if i + 1 < c.len() && c[i + 1] == '*' {
                    let at_start = i == 0 || c[i - 1] == '/';
                    let followed_by_slash = i + 2 < c.len() && c[i + 2] == '/';
                    if at_start && followed_by_slash {
                        re.push_str("(?:.*/)?");
                        i += 3;
                        continue;
                    }
                    if at_start && i + 2 == c.len() {
                        re.push_str(".*");
                        i += 2;
                        continue;
                    }
                    re.push_str(".*");
                    i += 2;
                    continue;
                }
                re.push_str("[^/]*");
            }
            '?' => re.push_str("[^/]"),
            '[' => {
                if let Some(end) = c[i + 1..].iter().position(|&x| x == ']') {
                    let inner: String = c[i + 1..i + 1 + end].iter().collect();
                    let inner = inner.strip_prefix('!').map(|r| format!("^{r}")).unwrap_or(inner);
                    re.push('[');
                    re.push_str(&inner.replace('\\', "\\\\"));
                    re.push(']');
                    i += end + 2;
                    continue;
                }
                re.push_str("\\[");
            }
            ch => re.push_str(&regex::escape(&ch.to_string())),
        }
        i += 1;
    }
    re.push_str(if dir_only { "/.*$" } else { "(?:/.*)?$" });
    Regex::new(&re).ok()
}

#[derive(Debug, Clone, Default, PartialEq)]
struct AttrOverride {
    language: Option<String>,
    vendored: Option<bool>,
    generated: Option<bool>,
    documentation: Option<bool>,
    detectable: Option<bool>,
}

struct AttrRule {
    re: Regex,
    attrs: AttrOverride,
}

fn parse_attr_bool(token: &str, name: &str) -> Option<bool> {
    if token == name {
        Some(true)
    } else if token == format!("-{name}") || token == format!("!{name}") {
        Some(false)
    } else if let Some(v) = token.strip_prefix(&format!("{name}=")) {
        Some(!matches!(v.to_ascii_lowercase().as_str(), "false" | "0" | "no"))
    } else {
        None
    }
}

fn parse_gitattributes(text: &str) -> Vec<AttrRule> {
    let mut rules = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(pattern) = parts.next() else { continue };
        let mut a = AttrOverride::default();
        for tok in parts {
            if let Some(v) = tok.strip_prefix("linguist-language=") {
                a.language = Some(v.to_string());
            }
            if let Some(b) = parse_attr_bool(tok, "linguist-vendored") {
                a.vendored = Some(b);
            }
            if let Some(b) = parse_attr_bool(tok, "linguist-generated") {
                a.generated = Some(b);
            }
            if let Some(b) = parse_attr_bool(tok, "linguist-documentation") {
                a.documentation = Some(b);
            }
            if let Some(b) = parse_attr_bool(tok, "linguist-detectable") {
                a.detectable = Some(b);
            }
        }
        if a != AttrOverride::default() {
            if let Some(re) = glob_to_regex(pattern) {
                rules.push(AttrRule { re, attrs: a });
            }
        }
    }
    rules
}

fn attrs_for(rules: &[AttrRule], rel: &str) -> AttrOverride {
    let mut out = AttrOverride::default();
    for r in rules.iter().filter(|r| r.re.is_match(rel)) {
        // later lines win, attribute by attribute
        if r.attrs.language.is_some() {
            out.language = r.attrs.language.clone();
        }
        out.vendored = r.attrs.vendored.or(out.vendored);
        out.generated = r.attrs.generated.or(out.generated);
        out.documentation = r.attrs.documentation.or(out.documentation);
        out.detectable = r.attrs.detectable.or(out.detectable);
    }
    out
}

// ── generated-file heuristics ──────────────────────────────────────────────

fn generated_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(^|/)(package-lock\.json|npm-shrinkwrap\.json|yarn\.lock|pnpm-lock\.yaml|bun\.lockb?|cargo\.lock|composer\.lock|gemfile\.lock|poetry\.lock|pipfile\.lock|go\.sum|flake\.lock|packages\.lock\.json|mix\.lock|pubspec\.lock)$|\.min\.(js|css)$|\.(map|snap)$|\.pb\.go$|_pb2(_grpc)?\.py$|\.g\.dart$|\.designer\.cs$|\.generated\.\w+$|(^|/)\.svelte-kit/|(^|/)__generated__/",
        )
        .unwrap_or_else(|_| never())
    })
}

// ── report ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LanguageShare {
    pub name: String,
    pub bytes: u64,
    pub files: u32,
    pub percent: f64,
    pub color: String,
    #[serde(rename = "type")]
    pub kind: LangType,
    pub custom: bool,
    pub brand: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Excluded {
    pub vendored: u32,
    pub generated: u32,
    pub documentation: u32,
    pub binary: u32,
    pub data_or_prose: u32,
    pub unknown: u32,
    pub too_large: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LinguistReport {
    pub root: String,
    pub total_bytes: u64,
    pub counted_files: u32,
    pub scanned_files: u32,
    pub languages: Vec<LanguageShare>,
    pub excluded: Excluded,
    pub truncated: bool,
    /// `git` (exact GitHub view) or `walk` (directory walk with .gitignore).
    pub source: String,
    pub elapsed_ms: u64,
    pub custom_languages: usize,
}

// ── file enumeration ───────────────────────────────────────────────────────

const ALWAYS_SKIP_DIRS: &[&str] = &[".git", ".hg", ".svn", "node_modules", ".venv", "venv", "__pycache__", ".mypy_cache", ".pytest_cache", ".gradle", ".idea", ".cache", ".next", ".nuxt"];

fn git_files(root: &Path) -> Option<Vec<String>> {
    let out = Command::new("git").arg("-C").arg(root).args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let mut v: Vec<String> = out.stdout.split(|&b| b == 0).filter(|s| !s.is_empty()).filter_map(|s| String::from_utf8(s.to_vec()).ok()).collect();
    v.sort();
    v.dedup();
    Some(v)
}

struct IgnoreRule {
    re: Regex,
    negate: bool,
}

fn parse_gitignore(text: &str) -> Vec<IgnoreRule> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (negate, pat) = match l.strip_prefix('!') {
                Some(r) => (true, r),
                None => (false, l),
            };
            glob_to_regex(pat).map(|re| IgnoreRule { re, negate })
        })
        .collect()
}

fn is_ignored(rules: &[IgnoreRule], rel: &str) -> bool {
    let mut ignored = false;
    for r in rules {
        if r.re.is_match(rel) {
            ignored = !r.negate;
        }
    }
    ignored
}

fn walk_files(root: &Path, deadline: Instant, truncated: &mut bool) -> Vec<String> {
    let root_ignore = fs::read_to_string(root.join(".gitignore")).map(|t| parse_gitignore(&t)).unwrap_or_default();
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, String)> = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, rel_dir)) = stack.pop() {
        if Instant::now() > deadline || out.len() >= MAX_FILES {
            *truncated = true;
            break;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if rel_dir.is_empty() { name.clone() } else { format!("{rel_dir}/{name}") };
            if ft.is_dir() {
                if ALWAYS_SKIP_DIRS.contains(&name.as_str()) || is_ignored(&root_ignore, &format!("{rel}/x")) && is_ignored(&root_ignore, &rel) {
                    continue;
                }
                stack.push((entry.path(), rel));
            } else if ft.is_file() && !is_ignored(&root_ignore, &rel) {
                out.push(rel);
            }
        }
    }
    out.sort();
    out
}

// ── scanning ───────────────────────────────────────────────────────────────

fn read_head(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let mut f = fs::File::open(path).ok()?;
    let mut buf = vec![0u8; limit];
    let n = f.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(buf)
}

fn looks_binary(head: &[u8]) -> bool {
    head.iter().take(8000).any(|&b| b == 0)
}

/// Loads `<root>/.blue/languages.json` and the per-user file.
pub fn load_custom_languages(root: &Path) -> Vec<CustomLanguage> {
    let mut all = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if let Ok(t) = fs::read_to_string(home.join(".config/Blue-Environment/blue-code/languages.json")) {
            all.extend(parse_custom_languages(&t));
        }
    }
    if let Ok(t) = fs::read_to_string(root.join(".blue/languages.json")) {
        all.extend(parse_custom_languages(&t)); // project last = wins
    }
    all
}

/// Scans `root` and returns the language breakdown.
pub fn scan(root: &Path) -> Result<LinguistReport, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    let started = Instant::now();
    let deadline = started + TIME_BUDGET;
    let custom = load_custom_languages(root);
    let registry = Registry::with_custom(&custom);
    let attr_rules = fs::read_to_string(root.join(".gitattributes")).map(|t| parse_gitattributes(&t)).unwrap_or_default();

    let mut truncated = false;
    let (files, source) = match git_files(root) {
        Some(f) => (f, "git"),
        None => (walk_files(root, deadline, &mut truncated), "walk"),
    };

    let mut ex = Excluded::default();
    let mut agg: BTreeMap<usize, (u64, u32)> = BTreeMap::new();
    let mut scanned = 0u32;

    for rel in files.iter() {
        if scanned as usize >= MAX_FILES || Instant::now() > deadline {
            truncated = true;
            break;
        }
        scanned += 1;
        let attrs = attrs_for(&attr_rules, rel);
        let name = rel.rsplit('/').next().unwrap_or(rel);

        let vendored = attrs.vendored.unwrap_or_else(|| registry.vendored.iter().any(|r| r.is_match(rel)));
        if vendored {
            ex.vendored += 1;
            continue;
        }
        let documentation = attrs.documentation.unwrap_or_else(|| registry.documentation.iter().any(|r| r.is_match(rel)));
        if documentation {
            ex.documentation += 1;
            continue;
        }
        let generated = attrs.generated.unwrap_or_else(|| generated_re().is_match(rel));
        if generated {
            ex.generated += 1;
            continue;
        }

        let path = root.join(rel);
        let Ok(meta) = fs::metadata(&path) else { continue }; // deleted but still indexed
        if !meta.is_file() || meta.len() == 0 {
            continue;
        }
        if meta.len() > MAX_FILE_BYTES {
            ex.too_large += 1;
            continue;
        }

        // Content is only read in full when it can change the answer (ambiguous
        // extension, or no extension at all → shebang); otherwise just enough to
        // tell text from binary.
        let forced = attrs.language.as_deref().and_then(|n| registry.find_by_name(n));
        let need_full_head = forced.is_none() && (registry.detect(name, "").is_none() || registry.by_ext_ambiguous(name));
        let head_bytes = read_head(&path, if need_full_head { HEAD_BYTES } else { 8000 }).unwrap_or_default();
        if looks_binary(&head_bytes) {
            ex.binary += 1;
            continue;
        }
        let head = String::from_utf8_lossy(&head_bytes);

        let idx = forced.or_else(|| registry.detect(name, &head));
        let Some(idx) = idx else {
            ex.unknown += 1;
            continue;
        };
        let def = registry.def(idx);
        let counted = attrs.detectable.unwrap_or_else(|| def.kind.counts_by_default());
        if !counted {
            ex.data_or_prose += 1;
            continue;
        }
        let e = agg.entry(idx).or_insert((0, 0));
        e.0 += meta.len();
        e.1 += 1;
    }

    let total: u64 = agg.values().map(|(b, _)| *b).sum();
    let mut languages: Vec<LanguageShare> = agg
        .into_iter()
        .map(|(idx, (bytes, files))| {
            let d = registry.def(idx);
            LanguageShare {
                name: d.name.clone(),
                bytes,
                files,
                percent: if total == 0 { 0.0 } else { bytes as f64 * 100.0 / total as f64 },
                color: d.color.clone(),
                kind: d.kind,
                custom: d.custom,
                brand: d.brand.clone(),
            }
        })
        .collect();
    languages.sort_by(|a, b| b.bytes.cmp(&a.bytes).then_with(|| a.name.cmp(&b.name)));

    Ok(LinguistReport {
        root: root.to_string_lossy().into_owned(),
        total_bytes: total,
        counted_files: languages.iter().map(|l| l.files).sum(),
        scanned_files: scanned,
        languages,
        excluded: ex,
        truncated,
        source: source.to_string(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        custom_languages: custom.len(),
    })
}

impl Registry {
    /// True when `name`'s extension has several candidates (content decides).
    fn by_ext_ambiguous(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        let mut from = 0;
        while let Some(rel) = lower[from..].find('.') {
            let dot = from + rel;
            if let Some(c) = self.by_ext.get(&lower[dot..]) {
                return c.len() > 1;
            }
            from = dot + 1;
        }
        false
    }
}

/// Template written by "Customize languages…".
pub const PROJECT_TEMPLATE: &str = r##"{
  "languages": [
    {
      "name": "MyLang",
      "type": "programming",
      "color": "#ff5fa2",
      "extensions": [".ml1", ".mylang"],
      "filenames": ["Mylangfile"],
      "interpreters": ["mylang"]
    }
  ]
}
"##;

/// Creates `<root>/.blue/languages.json` from the template if missing; returns its path.
pub fn init_project_config(root: &Path) -> Result<PathBuf, String> {
    if !root.is_dir() {
        return Err(format!("{} is not a folder", root.display()));
    }
    let dir = root.join(".blue");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join("languages.json");
    if !file.exists() {
        fs::write(&file, PROJECT_TEMPLATE).map_err(|e| e.to_string())?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static N: AtomicUsize = AtomicUsize::new(0);

    fn project(files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("linguist-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
        let _ = fs::remove_dir_all(&dir);
        for (p, content) in files {
            let full = dir.join(p);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(full, content).unwrap();
        }
        dir
    }

    fn share<'a>(r: &'a LinguistReport, name: &str) -> Option<&'a LanguageShare> {
        r.languages.iter().find(|l| l.name == name)
    }

    #[test]
    fn database_covers_github_languages() {
        let r = Registry::with_custom(&[]);
        assert!(r.language_count() > 800, "got {}", r.language_count());
        for (file, lang) in [("main.rs", "Rust"), ("app.py", "Python"), ("index.ts", "TypeScript"), ("Main.java", "Java"), ("a.go", "Go"), ("x.kt", "Kotlin"), ("style.scss", "SCSS"), ("x.zig", "Zig"), ("a.nim", "Nim"), ("Dockerfile", "Dockerfile"), ("Makefile", "Makefile")] {
            let i = r.detect(file, "").unwrap_or_else(|| panic!("no language for {file}"));
            assert_eq!(r.def(i).name, lang, "{file}");
        }
    }

    #[test]
    fn hackeros_languages_are_built_in_with_bit_io_colours_and_brands() {
        let r = Registry::with_custom(&[]);
        for (file, name, color, brand) in [
            ("main.h#", "H#", "#9c1120", Some("hsharp")),
            ("api.h#i", "H# Interface", "#c23b4e", Some("hsharp")),
            ("tool.hcs", "HackerScript", "#4b4b55", Some("hackerscript")),
            ("main.hl", "Hacker Lang", "#8250df", Some("hackerlang")),
            ("Bit.hk", "hk", "#ffffff", None),
            ("settings.hk", "hk", "#ffffff", None),
        ] {
            let i = r.detect(file, "").unwrap_or_else(|| panic!("{file} not detected"));
            let d = r.def(i);
            assert_eq!((d.name.as_str(), d.color.as_str(), d.brand.as_deref(), d.custom), (name, color, brand, true), "{file}");
        }
    }

    #[test]
    fn hs_is_haskell_not_hackerscript() {
        // bit.io: HackerScript is `.hcs`; `.hs` stays Haskell.
        let r = Registry::with_custom(&[]);
        assert_eq!(r.def(r.detect("Main.hs", "").unwrap()).name, "Haskell");
    }

    #[test]
    fn longest_extension_wins() {
        let r = Registry::with_custom(&[]);
        let i = r.detect("types.d.ts", "").unwrap();
        assert_eq!(r.def(i).name, "TypeScript");
        assert!(r.detect("foo.test.hl", "").is_some_and(|i| r.def(i).name == "Hacker Lang"));
    }

    #[test]
    fn ambiguous_extensions_use_content() {
        let r = Registry::with_custom(&[]);
        let name = |f: &str, c: &str| r.def(r.detect(f, c).unwrap()).name.clone();
        assert_eq!(name("a.h", "#include <stdio.h>\nint main(void);"), "C");
        assert_eq!(name("a.h", "#include <vector>\nnamespace x { class A {}; }"), "C++");
        assert_eq!(name("a.h", "#import <Foundation/Foundation.h>\n@interface A : NSObject @end"), "Objective-C");
        assert_eq!(name("a.pl", "use strict;\nmy $x = 1;"), "Perl");
        assert_eq!(name("a.pl", "parent(a, b).\nancestor(X, Y) :- parent(X, Y)."), "Prolog");
        assert_eq!(name("a.ts", "<?xml version=\"1.0\"?><TS></TS>"), "XML");
        assert_eq!(name("a.ts", "export const x: number = 1;"), "TypeScript");
    }

    #[test]
    fn shebang_detects_extensionless_scripts() {
        let r = Registry::with_custom(&[]);
        assert_eq!(r.def(r.detect("deploy", "#!/usr/bin/env python3\nprint(1)").unwrap()).name, "Python");
        assert_eq!(r.def(r.detect("run", "#!/bin/bash\necho").unwrap()).name, "Shell");
        assert_eq!(r.def(r.detect("x", "#!/usr/bin/env node\n").unwrap()).name, "JavaScript");
        assert!(r.detect("README", "just text").is_none());
    }

    #[test]
    fn custom_languages_override_the_database_and_are_validated() {
        let json = r##"{"languages":[
            {"name":"MyLang","color":"#ff00aa","extensions":["ml1","*.ml2"],"filenames":["Mylangfile"]},
            {"name":"","extensions":["bad"]},
            {"name":"Weird","color":"not-a-colour","extensions":["a/b","","  ",".ok"]}
        ]}"##;
        let defs = parse_custom_languages(json);
        assert_eq!(defs.len(), 2, "empty name dropped");
        assert_eq!(defs[0].extensions, vec![".ml1", ".ml2"]);
        assert_eq!(defs[1].extensions, vec![".ok"], "path-like / blank extensions dropped");
        let r = Registry::with_custom(&defs);
        let i = r.detect("a.ml2", "").unwrap();
        assert_eq!((r.def(i).name.as_str(), r.def(i).color.as_str()), ("MyLang", "#ff00aa"));
        assert_eq!(r.def(r.detect("Mylangfile", "").unwrap()).name, "MyLang");
        assert_eq!(r.def(r.detect("x.ok", "").unwrap()).color, FALLBACK_COLOR, "invalid colour falls back");

        // a project can even claim an extension GitHub assigns elsewhere
        let take_rs = parse_custom_languages(r#"[{"name":"Rusty","extensions":[".rs"]}]"#);
        let r2 = Registry::with_custom(&take_rs);
        assert_eq!(r2.def(r2.detect("a.rs", "").unwrap()).name, "Rusty");
    }

    #[test]
    fn malformed_custom_config_is_ignored_not_fatal() {
        assert!(parse_custom_languages("{{{ nope").is_empty());
        assert!(parse_custom_languages("42").is_empty());
    }

    #[test]
    fn glob_translation_matches_gitignore_semantics() {
        let m = |p: &str, s: &str| glob_to_regex(p).unwrap().is_match(s);
        assert!(m("*.js", "a.js") && m("*.js", "src/deep/a.js") && !m("*.js", "a.jsx"));
        assert!(m("/build", "build/x.rs") && !m("/build", "src/build/x.rs"));
        assert!(m("docs/", "docs/a/b.md") && m("docs/**", "docs/a.md"));
        assert!(m("src/*.rs", "src/a.rs") && !m("src/*.rs", "src/x/a.rs"));
        assert!(m("**/gen/*.go", "a/b/gen/x.go") && m("**/gen/*.go", "gen/x.go"));
        assert!(m("file?.txt", "file1.txt") && !m("file?.txt", "file12.txt"));
        assert!(m("[ab].c", "a.c") && !m("[ab].c", "c.c"));
    }

    #[test]
    fn counts_bytes_like_github_and_skips_data_and_prose() {
        let p = project(&[
            ("src/main.rs", &"fn main() {}\n".repeat(100)),     // 1300
            ("web/app.ts", &"export const a = 1;\n".repeat(50)), // 1000
            ("data.json", &"{\"a\":1}\n".repeat(500)),           // data: not counted
            ("notes.md", &"# hi\n".repeat(500)),                 // prose: not counted
            ("README.md", &"# hi\n".repeat(500)),                // documentation: not counted
        ]);
        let r = scan(&p).unwrap();
        assert_eq!(r.languages.len(), 2, "{:?}", r.languages);
        assert_eq!(r.languages[0].name, "Rust");
        assert_eq!(r.languages[0].bytes, 1300);
        assert_eq!(r.languages[1].name, "TypeScript");
        assert!((r.languages.iter().map(|l| l.percent).sum::<f64>() - 100.0).abs() < 1e-9);
        assert_eq!(r.excluded.data_or_prose, 2, "json (data) + notes.md (prose)");
        assert_eq!(r.excluded.documentation, 1, "README is documentation, as on GitHub");
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn a_bit_project_shows_the_hackeros_languages() {
        let p = project(&[
            ("Bit.hk", "[package]\n-> name => app\n"),
            ("src/main.h#", &"fn main() { println(\"hi\") }\n".repeat(40)),
            ("src/api.h#i", &"fn api();\n".repeat(10)),
            ("cmd/tool.hcs", &"fun main() {}\n".repeat(20)),
            ("main.hl", &"echo hi\n".repeat(30)),
        ]);
        let r = scan(&p).unwrap();
        let names: Vec<&str> = r.languages.iter().map(|l| l.name.as_str()).collect();
        for n in ["H#", "H# Interface", "HackerScript", "Hacker Lang", "hk"] {
            assert!(names.contains(&n), "{n} missing in {names:?}");
        }
        assert_eq!(r.languages[0].name, "H#");
        assert!(r.languages.iter().filter(|l| l.custom).count() == 5);
        assert_eq!(share(&r, "H#").unwrap().brand.as_deref(), Some("hsharp"));
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn gitattributes_overrides_language_and_exclusions() {
        let p = project(&[
            (".gitattributes", "*.inc linguist-language=PHP\nlegacy/** linguist-vendored\ngen/** linguist-generated\nnotes.md linguist-detectable\ndocs/** -linguist-documentation\n"),
            ("a.inc", &"<?php echo 1; ?>\n".repeat(10)),
            ("legacy/old.c", "int x;\n"),
            ("gen/out.go", "package x\n"),
            ("notes.md", &"# notes\n".repeat(10)),
            ("docs/guide.rb", &"puts 1\n".repeat(10)), // docs/ is documentation by default; override re-includes it
        ]);
        let r = scan(&p).unwrap();
        assert!(share(&r, "PHP").is_some(), "{:?}", r.languages);
        assert!(share(&r, "C").is_none(), "vendored override");
        assert!(share(&r, "Go").is_none(), "generated override");
        assert!(share(&r, "Markdown").is_some(), "linguist-detectable includes prose");
        assert!(share(&r, "Ruby").is_some(), "-linguist-documentation re-includes docs/");
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn vendored_generated_and_binary_files_are_skipped() {
        let p = project(&[
            ("src/a.rs", "fn a() {}\n"),
            ("vendor/lib.go", "package v\n"),
            ("node_modules/x/index.js", "var a;\n"),
            ("package-lock.json", "{}"),
            ("dist/app.min.js", "var a;"),
            ("assets/blob.rs", "\0\0\0binary\0"),
        ]);
        let r = scan(&p).unwrap();
        assert_eq!(r.languages.len(), 1, "{:?}", r.languages);
        assert_eq!(r.languages[0].name, "Rust");
        // vendor/ and *.min.js are vendored (GitHub's vendor.yml); node_modules/ is never even walked; the lockfile is generated
        assert!(r.excluded.vendored == 2 && r.excluded.generated == 1 && r.excluded.binary == 1, "{:?}", r.excluded);
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn walk_mode_honours_gitignore() {
        let p = project(&[(".gitignore", "target/\n*.log\n!keep.log\n"), ("src/a.rs", "fn a() {}\n"), ("target/debug/gen.rs", "fn g() {}\n")]);
        let r = scan(&p).unwrap();
        // not a git repo → walk
        if r.source == "walk" {
            assert_eq!(r.languages.len(), 1);
            assert_eq!(r.languages[0].bytes, 10);
        }
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn project_config_file_defines_custom_languages_end_to_end() {
        let p = project(&[
            (".blue/languages.json", r##"[{"name":"Sparkle","color":"#abcdef","extensions":[".spk"]}]"##),
            ("a.spk", &"magic\n".repeat(10)),
            ("b.rs", "fn b() {}\n"),
        ]);
        let r = scan(&p).unwrap();
        let s = share(&r, "Sparkle").expect("custom language counted");
        assert!(s.custom && s.color == "#abcdef");
        assert_eq!(r.custom_languages, 1);
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn init_project_config_writes_a_valid_template_once() {
        let p = project(&[("a.rs", "fn a() {}\n")]);
        let f = init_project_config(&p).unwrap();
        assert!(f.ends_with(".blue/languages.json"));
        assert_eq!(parse_custom_languages(&fs::read_to_string(&f).unwrap()).len(), 1, "the template must itself be valid");
        fs::write(&f, "[]").unwrap();
        init_project_config(&p).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "[]", "must not overwrite an existing file");
        let _ = fs::remove_dir_all(p);
    }

    #[test]
    fn empty_or_missing_folders_behave() {
        let p = project(&[("empty.rs", "")]);
        let r = scan(&p).unwrap();
        assert!(r.languages.is_empty() && r.total_bytes == 0);
        assert!(scan(Path::new("/definitely/not/here")).is_err());
        let _ = fs::remove_dir_all(p);
    }
}
