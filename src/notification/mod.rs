pub mod dbus;

use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    Low,
    Normal,
    Critical,
}

impl Default for Urgency {
    fn default() -> Self { Self::Normal }
}

impl Urgency {
    pub fn from_u8(v: u8) -> Self {
        match v { 0 => Self::Low, 2 => Self::Critical, _ => Self::Normal }
    }
    pub fn default_timeout_ms(&self) -> Option<u64> {
        match self {
            Self::Low      => Some(3_000),
            Self::Normal   => Some(5_000),
            Self::Critical => None,
        }
    }
    /// Kolor akcentu jako slint::Color (ARGB)
    pub fn accent_color(&self) -> slint::Color {
        match self {
            Self::Low      => slint::Color::from_argb_u8(0xff, 0x3a, 0x4d, 0x6a),
            Self::Normal   => slint::Color::from_argb_u8(0xff, 0x3d, 0x8e, 0xf0),
            Self::Critical => slint::Color::from_argb_u8(0xff, 0xff, 0x47, 0x57),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Notification {
    pub id:          u32,
    pub app_name:    String,
    pub app_icon:    String,
    pub summary:     String,
    pub body:        String,
    pub urgency:     Urgency,
    pub actions:     Vec<(String, String)>,
    pub expire_ms:   Option<u64>,
    pub created_at:  Instant,
    pub replaces_id: u32,
}

impl Notification {
    pub fn is_expired(&self) -> bool {
        self.expire_ms
        .map(|ms| self.created_at.elapsed() >= Duration::from_millis(ms))
        .unwrap_or(false)
    }
    pub fn age_str(&self) -> String {
        let s = self.created_at.elapsed().as_secs();
        if      s < 5    { "przed chwilą".into() }
        else if s < 60   { format!("{}s temu", s) }
        else if s < 3600 { format!("{}min temu", s / 60) }
        else             { format!("{}h temu", s / 3600) }
    }
}

pub static STORE: Lazy<Mutex<Store>> =
Lazy::new(|| Mutex::new(Store::default()));

#[derive(Default)]
pub struct Store {
    pub items:     VecDeque<Notification>,
    next_id:       u32,
    pub on_change: Vec<Box<dyn Fn() + Send + 'static>>,
}

impl Store {
    pub fn add(&mut self, mut n: Notification) -> u32 {
        if n.replaces_id > 0 {
            self.items.retain(|x| x.id != n.replaces_id);
            n.id = n.replaces_id;
        } else {
            self.next_id += 1;
            n.id = self.next_id;
        }
        let id = n.id;
        if self.items.len() >= 50 { self.items.pop_back(); }
        self.items.push_front(n);
        self.fire();
        id
    }

    pub fn close(&mut self, id: u32) {
        self.items.retain(|n| n.id != id);
        self.fire();
    }

    pub fn close_expired(&mut self) {
        let before = self.items.len();
        self.items.retain(|n| !n.is_expired());
        if self.items.len() != before { self.fire(); }
    }

    pub fn clear_all(&mut self) {
        self.items.clear();
        self.fire();
    }

    pub fn active_popups(&self) -> Vec<&Notification> {
        self.items.iter().filter(|n| !n.is_expired()).take(3).collect()
    }

    fn fire(&self) {
        for f in &self.on_change { f(); }
    }
}

pub fn strip_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _   => if !in_tag { out.push(c); }
        }
    }
    out .replace("&amp;",  "&")
    .replace("&lt;",   "<")
    .replace("&gt;",   ">")
    .replace("&quot;", "\"")
    .replace("&apos;", "'")
    .replace("&#10;",  "\n")
}
