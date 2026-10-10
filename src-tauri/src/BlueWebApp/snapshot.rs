use base64::{engine::general_purpose::STANDARD, Engine as _};
use image::{imageops::FilterType, RgbImage};

/// Wider than this (in device pixels) is downscaled: a HiDPI webview would
/// otherwise produce a multi-megabyte data URL for what is only a stand-in.
pub const MAX_SNAPSHOT_WIDTH: u32 = 1600;
const JPEG_QUALITY: u8 = 78;
/// Colour a translucent pixel is composited onto (the shell's slate-900).
const BACKDROP: [u32; 3] = [15, 23, 42];

/// Pixels exactly as cairo hands them over: 32-bit native-endian words,
/// `ARGB32` (premultiplied alpha) or `RGB24` (top byte unused), `stride`
/// bytes per row.
pub struct RawSnapshot {
    pub width: u32,
    pub height: u32,
    pub stride: usize,
    pub has_alpha: bool,
    pub bytes: Vec<u8>,
}

/// Converts cairo's packed 32-bit pixels to tightly packed RGB8.
fn to_rgb(raw: &RawSnapshot) -> Result<Vec<u8>, String> {
    let (w, h) = (raw.width as usize, raw.height as usize);
    if w == 0 || h == 0 {
        return Err("empty snapshot".into());
    }
    if raw.stride < w * 4 || raw.bytes.len() < raw.stride * (h - 1) + w * 4 {
        return Err("snapshot buffer is smaller than its dimensions".into());
    }
    let mut out = Vec::with_capacity(w * h * 3);
    for y in 0..h {
        let row = &raw.bytes[y * raw.stride..y * raw.stride + w * 4];
        for px in row.chunks_exact(4) {
            let v = u32::from_ne_bytes([px[0], px[1], px[2], px[3]]);
            let (r, g, b) = ((v >> 16) & 0xff, (v >> 8) & 0xff, v & 0xff);
            if raw.has_alpha {
                // Premultiplied: colour already carries its alpha, only the
                // uncovered part of the pixel needs the backdrop.
                let inv = 255 - ((v >> 24) & 0xff);
                out.push((r + BACKDROP[0] * inv / 255).min(255) as u8);
                out.push((g + BACKDROP[1] * inv / 255).min(255) as u8);
                out.push((b + BACKDROP[2] * inv / 255).min(255) as u8);
            } else {
                out.extend_from_slice(&[r as u8, g as u8, b as u8]);
            }
        }
    }
    Ok(out)
}

/// True when every sampled pixel is identical. A capture that produced one flat
/// colour is almost certainly a failed render (accelerated compositing that
/// WebKit could not read back), and showing it would be worse than showing the
/// neutral placeholder.
fn looks_blank(rgb: &[u8], w: u32, h: u32) -> bool {
    let (w, h) = (w as usize, h as usize);
    let first = [rgb[0], rgb[1], rgb[2]];
    // 32 x 32 grid of samples is plenty to catch any real content.
    for gy in 0..32usize {
        let y = (gy * (h - 1)) / 31;
        for gx in 0..32usize {
            let x = (gx * (w - 1)) / 31;
            let i = (y * w + x) * 3;
            if rgb[i] != first[0] || rgb[i + 1] != first[1] || rgb[i + 2] != first[2] {
                return false;
            }
        }
    }
    true
}

/// Raw pixels → `data:image/jpeg;base64,…`.
pub fn encode_data_url(raw: &RawSnapshot) -> Result<String, String> {
    let rgb = to_rgb(raw)?;
    if looks_blank(&rgb, raw.width, raw.height) {
        return Err("snapshot is blank".into());
    }
    let mut img = RgbImage::from_raw(raw.width, raw.height, rgb)
        .ok_or_else(|| "snapshot buffer does not match its dimensions".to_string())?;
    if img.width() > MAX_SNAPSHOT_WIDTH {
        let nh = ((img.height() as u64 * MAX_SNAPSHOT_WIDTH as u64) / img.width() as u64).max(1) as u32;
        img = image::imageops::resize(&img, MAX_SNAPSHOT_WIDTH, nh, FilterType::Triangle);
    }
    let mut jpeg = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY);
    img.write_with_encoder(enc).map_err(|e| e.to_string())?;
    Ok(format!("data:image/jpeg;base64,{}", STANDARD.encode(jpeg)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a raw ARGB32 buffer from `f(x, y) -> (r, g, b)` with row padding.
    fn raw(w: u32, h: u32, stride_pad: usize, f: impl Fn(u32, u32) -> (u8, u8, u8)) -> RawSnapshot {
        let stride = w as usize * 4 + stride_pad;
        let mut bytes = vec![0u8; stride * h as usize];
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = f(x, y);
                let v = (0xffu32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
                let o = y as usize * stride + x as usize * 4;
                bytes[o..o + 4].copy_from_slice(&v.to_ne_bytes());
            }
        }
        RawSnapshot { width: w, height: h, stride, has_alpha: true, bytes }
    }

    #[test]
    fn encodes_real_content_as_jpeg_data_url() {
        let r = raw(64, 48, 8, |x, y| ((x * 4) as u8, (y * 5) as u8, 90));
        let url = encode_data_url(&r).expect("should encode");
        assert!(url.starts_with("data:image/jpeg;base64,"));
        let b64 = url.trim_start_matches("data:image/jpeg;base64,");
        let bytes = STANDARD.decode(b64).unwrap();
        // JPEG magic number
        assert_eq!(&bytes[..3], &[0xff, 0xd8, 0xff]);
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (64, 48));
    }

    #[test]
    fn respects_row_stride_padding() {
        // Left half red, right half blue — wrong stride handling would smear it.
        let r = raw(40, 10, 16, |x, _| if x < 20 { (255, 0, 0) } else { (0, 0, 255) });
        let rgb = to_rgb(&r).unwrap();
        assert_eq!(&rgb[0..3], &[255, 0, 0]);
        let last_row_right = (9 * 40 + 39) * 3;
        assert_eq!(&rgb[last_row_right..last_row_right + 3], &[0, 0, 255]);
    }

    #[test]
    fn flat_capture_is_rejected_as_blank() {
        let r = raw(50, 50, 0, |_, _| (255, 255, 255));
        assert!(encode_data_url(&r).is_err());
    }

    #[test]
    fn wide_captures_are_downscaled() {
        let r = raw(3200, 100, 0, |x, y| ((x % 251) as u8, (y % 251) as u8, 10));
        let url = encode_data_url(&r).unwrap();
        let bytes = STANDARD.decode(url.trim_start_matches("data:image/jpeg;base64,")).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.width(), MAX_SNAPSHOT_WIDTH);
        assert_eq!(decoded.height(), 50);
    }

    #[test]
    fn rejects_truncated_buffers() {
        let mut r = raw(20, 20, 0, |x, _| (x as u8, 1, 2));
        r.bytes.truncate(100);
        assert!(encode_data_url(&r).is_err());
    }

    #[test]
    fn translucent_pixels_are_composited_on_the_backdrop() {
        // alpha 0 premultiplied → pure backdrop colour.
        let w = 4u32;
        let v = 0u32.to_ne_bytes();
        let r = RawSnapshot { width: w, height: 1, stride: 16, has_alpha: true, bytes: v.repeat(4) };
        let rgb = to_rgb(&r).unwrap();
        assert_eq!(&rgb[0..3], &[15, 23, 42]);
    }
}
