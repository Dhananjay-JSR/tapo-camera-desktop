//! A simple generated app icon (a camera lens on a rounded blue tile) — no asset file needed.

pub fn icon() -> egui::IconData {
    let s = 256usize;
    let c = s as f32 / 2.0;
    let mut rgba = vec![0u8; s * s * 4];
    let (ro, ri) = (s as f32 * 0.30, s as f32 * 0.19); // lens outer/inner radius
    let (hx, hy) = (c - ro * 0.45, c - ro * 0.45); // highlight position

    for y in 0..s {
        for x in 0..s {
            let (fx, fy) = (x as f32, y as f32);
            if !rounded_rect(fx, fy, 22.0, s as f32 - 22.0, 46.0) {
                continue; // outside the tile stays transparent
            }
            let d = ((fx - c).powi(2) + (fy - c).powi(2)).sqrt();
            let (mut r, mut g, mut b) = (0x1Eu8, 0x6Fu8, 0xB8u8); // brand blue tile
            if d < ro && d >= ri {
                (r, g, b) = (0xF0, 0xF4, 0xF8); // lens ring
            } else if d < ri {
                (r, g, b) = (0x10, 0x1A, 0x28); // lens glass
            }
            if ((fx - hx).powi(2) + (fy - hy).powi(2)).sqrt() < s as f32 * 0.04 {
                (r, g, b) = (0xFF, 0xFF, 0xFF); // highlight dot
            }
            let i = (y * s + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[r, g, b, 0xFF]);
        }
    }
    egui::IconData {
        rgba,
        width: s as u32,
        height: s as u32,
    }
}

/// Inside the square [lo,hi]² with rounded corners of radius `r`.
fn rounded_rect(x: f32, y: f32, lo: f32, hi: f32, r: f32) -> bool {
    if x < lo || x > hi || y < lo || y > hi {
        return false;
    }
    let nx = x.clamp(lo + r, hi - r);
    let ny = y.clamp(lo + r, hi - r);
    ((x - nx).powi(2) + (y - ny).powi(2)).sqrt() <= r
}
