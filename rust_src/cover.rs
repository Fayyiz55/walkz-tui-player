use ratatui::buffer::Buffer;
use ratatui::layout::{Rect, Size};
use ratatui::style::Color;
use ratatui_image::{protocol::Protocol, Resize};

pub struct CoverRenderer;

/// DJB2 title hash → pseudo-random RGB, shared by the procedural vinyl
/// placeholder and the ambient backdrop's hue fallback (deterministic per
/// title, so a track's vinyl art and its ambient hue always agree).
pub(crate) fn title_hash_rgb(title: &str) -> (u8, u8, u8) {
    let mut hash: u32 = 5381;
    for b in title.bytes() {
        hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as u32);
    }
    ((hash & 0xFF) as u8, ((hash >> 8) & 0xFF) as u8, ((hash >> 16) & 0xFF) as u8)
}

/// Cached prepared album-art protocol: rebuilt only when the track or the
/// art-area dimensions change.
pub struct CoverCache {
    pub track_id: String,
    pub art_dims: (u16, u16),
    pub proto: Protocol,
}

impl CoverRenderer {
    /// Render the current track's cover through ratatui-image (physical
    /// resolution via the terminal's graphics protocol — Kitty, Sixel,
    /// iTerm2, or a half-blocks fallback), caching the prepared protocol
    /// per (track, area dims). Falls back to the procedural vinyl artwork
    /// when there is no art, no picker, or the payload cannot be decoded.
    pub fn render_cover(f: &mut ratatui::Frame, app: &crate::app::App, area: Rect, track: &crate::models::Track) {
        let track_id = track.id.clone();
        let art_dims = (area.width, area.height);

        // Cache hit: render the prepared protocol.
        {
            let cache = app.cover_cache.borrow();
            if let Some(entry) = cache.as_ref() {
                if entry.track_id == track_id && entry.art_dims == art_dims {
                    render_protocol_centered(f, &entry.proto, area);
                    return;
                }
            }
        }

        // Cache miss: prepare a new protocol from the cover bytes (loaded
        // lazily — the library no longer keeps every track's artwork in RAM).
        if let Some(bytes) = app.cover_art_bytes(track) {
            if let Some(picker) = app.picker.as_ref() {
                if let Ok(dyn_img) = image::load_from_memory(&bytes) {
                    if let Ok(proto) = picker.new_protocol(
                        dyn_img,
                        Size::new(area.width, area.height),
                        // CatmullRom: antialiased resample — the default
                        // Nearest filter drops pixel rows on every downscale
                        // and makes covers look crunchy next to the original.
                        Resize::Fit(Some(image::imageops::FilterType::CatmullRom)),
                    ) {
                        *app.cover_cache.borrow_mut() = Some(CoverCache {
                            track_id,
                            art_dims,
                            proto,
                        });
                        let entry = app.cover_cache.borrow();
                        if let Some(entry) = entry.as_ref() {
                            render_protocol_centered(f, &entry.proto, area);
                            return;
                        }
                    }
                }
            }
        }

        // Fallback: procedural vinyl artwork.
        CoverRenderer::render_vinyl_placeholder(f.buffer_mut(), area, &track.title);
    }

    /// Render a stylish procedural vinyl record artwork when no image file exists
    pub fn render_vinyl_placeholder(buf: &mut Buffer, area: Rect, title: &str) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        let w = area.width as f32;
        let h = (area.height as f32) * 2.0;
        let cx = w / 2.0;
        let cy = h / 2.0;
        let radius = (cx.min(cy) - 1.0).max(1.0);

        // Unique aesthetic album color derived from the title hash.
        let (hue_r, hue_g, hue_b) = title_hash_rgb(title);

        for row in 0..area.height {
            let y_pos = area.y + row;
            let top_py = (row as f32) * 2.0;
            let bot_py = top_py + 1.0;

            for col in 0..area.width {
                let x_pos = area.x + col;
                let px = col as f32;

                let top_color = Self::pixel_vinyl_color(px, top_py, cx, cy, radius, hue_r, hue_g, hue_b);
                let bot_color = Self::pixel_vinyl_color(px, bot_py, cx, cy, radius, hue_r, hue_g, hue_b);

                if let Some(cell) = buf.cell_mut((x_pos, y_pos)) {
                    cell.set_symbol("▀");
                    cell.set_fg(top_color);
                    cell.set_bg(bot_color);
                }
            }
        }
    }

    fn pixel_vinyl_color(
        x: f32,
        y: f32,
        cx: f32,
        cy: f32,
        r: f32,
        hue_r: u8,
        hue_g: u8,
        hue_b: u8,
    ) -> Color {
        let dx = x - cx;
        let dy = y - cy;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > r {
            // Background outer space
            return Color::Rgb(12, 12, 18);
        }

        // Center spindle hole
        if dist < r * 0.12 {
            return Color::Rgb(20, 20, 25);
        }

        // Center label (album sticker)
        if dist < r * 0.38 {
            let factor = (dist / (r * 0.38)).clamp(0.0, 1.0);
            let lr = (hue_r as f32 * 0.7 + 50.0 * factor) as u8;
            let lg = (hue_g as f32 * 0.7 + 50.0 * factor) as u8;
            let lb = (hue_b as f32 * 0.7 + 50.0 * factor) as u8;
            return Color::Rgb(lr, lg, lb);
        }

        // Vinyl outer ring edge
        if dist > r * 0.95 {
            return Color::Rgb(30, 32, 38);
        }

        // Vinyl body with grooves and reflection sheen
        let angle = dy.atan2(dx);
        let sheen = ((angle * 2.0).sin().abs() * 35.0) as u8;
        let groove = if ((dist * 2.5) as i32) % 2 == 0 { 8 } else { 0 };

        let base_v = 22 + sheen + groove;
        Color::Rgb(base_v, base_v + 2, base_v + 6)
    }
}

/// Render a prepared protocol centered inside `area` (the protocol is
/// already sized to fit; center it within the pane for a balanced look).
fn render_protocol_centered(f: &mut ratatui::Frame, proto: &Protocol, area: Rect) {
    let size = proto.size();
    let width = size.width.min(area.width);
    let height = size.height.min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect { x, y, width, height };
    f.render_widget(ratatui_image::Image::new(proto).allow_clipping(true), rect);
}
