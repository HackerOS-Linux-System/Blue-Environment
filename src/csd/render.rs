#![allow(deprecated)]
// src/csd/render.rs
// Renderowanie dekoracji okien (titlebar + przyciski + obramowanie)
// Używane przez backend renderer (glow/pixman) podczas rysowania klatek

use super::{
    DecoButton, DecoMode, WindowDecoration,
    BORDER_W, TITLEBAR_H, BTN_SIZE, BTN_MARGIN, BTN_Y_OFFSET,
};
use smithay::utils::{Logical, Physical, Point, Rectangle, Size, Transform};

/// Dane do narysowania jednej dekoracji.
/// Backend renderer iteruje przez tę listę i rysuje prostokąty.
#[derive(Debug, Clone)]
pub struct DecoRect {
    /// Pozycja i rozmiar w pikselach (względem lewego-górnego rogu OUTER geometry)
    pub rect: Rectangle<i32, Logical>,
    pub color: [f32; 4],  // RGBA
    pub corner_radius: f32,
}

/// Dane tekstu do narysowania (tytuł okna)
#[derive(Debug, Clone)]
pub struct DecoText {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub color: [f32; 4],
    pub font_size: f32,
}

/// Pełny zestaw danych do narysowania dekoracji jednego okna
#[derive(Debug, Clone)]
pub struct DecoDrawData {
    pub rects: Vec<DecoRect>,
    pub texts: Vec<DecoText>,
}

// Kolory
const COLOR_BAR_ACTIVE:   [f32; 4] = [0.051, 0.071, 0.098, 0.97]; // #0d1219
const COLOR_BAR_INACTIVE: [f32; 4] = [0.039, 0.051, 0.071, 0.95]; // #0a0d12
const COLOR_BORDER_ACTIVE:   [f32; 4] = [0.239, 0.557, 0.941, 0.6]; // #3d8ef0
const COLOR_BORDER_INACTIVE: [f32; 4] = [0.118, 0.176, 0.290, 1.0]; // #1e2d4a
const COLOR_TITLE_ACTIVE:    [f32; 4] = [0.910, 0.941, 0.996, 1.0]; // #e8f0fe
const COLOR_TITLE_INACTIVE:  [f32; 4] = [0.533, 0.600, 0.733, 1.0]; // #8899bb
const COLOR_BTN_CLOSE:       [f32; 4] = [1.000, 0.278, 0.341, 1.0]; // #ff4757
const COLOR_BTN_MAX:         [f32; 4] = [0.239, 0.557, 0.941, 1.0]; // #3d8ef0
const COLOR_BTN_MIN:         [f32; 4] = [1.000, 0.753, 0.196, 1.0]; // #ffc032
const COLOR_TRANSPARENT:     [f32; 4] = [0.0, 0.0, 0.0, 0.0];

impl DecoDrawData {
    pub fn build(
        deco: &WindowDecoration,
        client_size: Size<i32, Logical>,
    ) -> Self {
        if deco.mode == DecoMode::ClientSide {
            return Self { rects: vec![], texts: vec![] };
        }

        let mut rects = Vec::new();
        let mut texts = Vec::new();

        let outer_w = client_size.w + BORDER_W * 2;
        let outer_h = client_size.h + TITLEBAR_H + BORDER_W * 2;

        let bar_color    = if deco.active { COLOR_BAR_ACTIVE }    else { COLOR_BAR_INACTIVE };
        let border_color = if deco.active { COLOR_BORDER_ACTIVE }  else { COLOR_BORDER_INACTIVE };
        let title_color  = if deco.active { COLOR_TITLE_ACTIVE }   else { COLOR_TITLE_INACTIVE };

        // ── Pasek tytułu ──────────────────────────────────
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (0, 0),
                                               (outer_w, TITLEBAR_H + BORDER_W),
            ),
            color: bar_color,
            corner_radius: 8.0,
        });

        // ── Obramowanie lewe ──────────────────────────────
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (0, TITLEBAR_H + BORDER_W),
                                               (BORDER_W, client_size.h + BORDER_W),
            ),
            color: border_color,
            corner_radius: 0.0,
        });
        // ── Obramowanie prawe ─────────────────────────────
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (outer_w - BORDER_W, TITLEBAR_H + BORDER_W),
                                               (BORDER_W, client_size.h + BORDER_W),
            ),
            color: border_color,
            corner_radius: 0.0,
        });
        // ── Obramowanie dolne ─────────────────────────────
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (0, outer_h - BORDER_W),
                                               (outer_w, BORDER_W),
            ),
            color: border_color,
            corner_radius: 0.0,
        });

        // ── Linia akcentu pod tytułem ─────────────────────
        if deco.active {
            rects.push(DecoRect {
                rect: Rectangle::from_loc_and_size(
                    (BORDER_W + 8, TITLEBAR_H + BORDER_W - 1),
                                                   (outer_w - BORDER_W * 2 - 16, 1),
                ),
                color: [0.239, 0.557, 0.941, 0.3],
                corner_radius: 0.0,
            });
        }

        // ── Ikona aplikacji (placeholder — kółko z inicjałem) ──
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (BORDER_W + 6, BORDER_W + 7),
                                               (18, 18),
            ),
            color: [0.239, 0.557, 0.941, 0.2],
            corner_radius: 4.0,
        });

        // ── Tytuł okna ────────────────────────────────────
        texts.push(DecoText {
            text: deco.title.clone(),
                   x: BORDER_W + 30,  // po ikonie
                   y: BORDER_W + 8,
                   color: title_color,
                   font_size: 12.0,
        });

        // ── Przyciski (prawy bok paska) ───────────────────
        let btn_y = BORDER_W + BTN_Y_OFFSET;
        let btn_close_x = outer_w - BTN_MARGIN - BTN_SIZE;
        let btn_max_x   = btn_close_x - BTN_MARGIN - BTN_SIZE;
        let btn_min_x   = btn_max_x - BTN_MARGIN - BTN_SIZE;

        // Przycisk zamknij
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (btn_close_x, btn_y), (BTN_SIZE, BTN_SIZE),
            ),
            color: COLOR_BTN_CLOSE,
            corner_radius: (BTN_SIZE / 2) as f32,
        });
        texts.push(DecoText {
            text: "✕".to_string(),
                   x: btn_close_x + 3, y: btn_y + 2,
                   color: [1.0, 1.0, 1.0, 0.9],
                   font_size: 9.0,
        });

        // Przycisk maksymalizuj
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (btn_max_x, btn_y), (BTN_SIZE, BTN_SIZE),
            ),
            color: COLOR_BTN_MAX,
            corner_radius: (BTN_SIZE / 2) as f32,
        });
        texts.push(DecoText {
            text: if deco.maximized { "❐" } else { "□" }.to_string(),
                   x: btn_max_x + 3, y: btn_y + 2,
                   color: [1.0, 1.0, 1.0, 0.9],
                   font_size: 9.0,
        });

        // Przycisk minimalizuj
        rects.push(DecoRect {
            rect: Rectangle::from_loc_and_size(
                (btn_min_x, btn_y), (BTN_SIZE, BTN_SIZE),
            ),
            color: COLOR_BTN_MIN,
            corner_radius: (BTN_SIZE / 2) as f32,
        });
        texts.push(DecoText {
            text: "−".to_string(),
                   x: btn_min_x + 4, y: btn_y + 2,
                   color: [1.0, 1.0, 1.0, 0.9],
                   font_size: 9.0,
        });

        Self { rects, texts }
    }
}
