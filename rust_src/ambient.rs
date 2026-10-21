//! Ambient Now Playing backdrop.
//!
//! When enabled in the Settings tab, the Now Playing tab draws a backdrop
//! layer underneath every widget. Widgets draw over it; ratatui's patch
//! semantics keep the backdrop's cell background colors wherever a widget
//! does not paint its own background.
//!
//! - **Album art style** (default): the current track's cover, downscaled,
//!   center-cropped to the terminal's pixel aspect, gaussian-blurred and
//!   darkened, rendered full-bleed through the terminal's graphics protocol
//!   (Kitty / Sixel / iTerm2 — see the picker setup in `main.rs`). Terminals
//!   that only support half-blocks have no image channel, so the backdrop
//!   degrades to the color tint below.
//! - **Color tint style**: a per-row gradient derived from the cover's
//!   dominant color (or the title-hash hue when the track has no art) —
//!   plain cell background colors, possible on every terminal.
//!
//! Heavy pixel work runs once per track change; the stateful protocol is
//! rebuilt only when the track, the area dimensions, or a tuning knob
//! changes (mirrors the `CoverCache` pattern in `cover.rs`).

use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Frame;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::Resize;

use crate::settings::{AmbientStyle, AMBIENT_DIMMING_MAX, AMBIENT_DIMMING_MIN};

/// Longest edge of the downscaled backdrop source. The blur hides the lost
/// detail; smaller means cheaper processing and smaller payloads.
const THUMB_EDGE: u32 = 320;

/// Gaussian blur sigma applied to the backdrop (heavy, Walkman-style).
const BLUR_SIGMA: f32 = 8.0;

/// Brightest (top) and darkest (bottom) tint lightness before dimming.
const TINT_TOP: f32 = 0.34;
const TINT_BOTTOM: f32 = 0.12;

/// Which source fed the cached backdrop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackdropSource {
    BlurredArt,
    DominantColor,
    TitleHue,
}

/// Cached ambient backdrop, rebuilt only when its key changes
/// (track id, area dims, dimming, style).
pub struct AmbientCache {
    pub track_id: String,
    pub dims: (u16, u16),
    pub dimming: u8,
    pub style: AmbientStyle,
    #[allow(dead_code)]
    source: BackdropSource,
    /// Stateful protocol built from the processed backdrop image
    /// (graphic-protocol terminals); encodes once per rebuild, subsequent
    /// frames only place the image.
    pub state: Option<StatefulProtocol>,
    /// Per-row tint (RGB) used when no image channel exists.
    pub ramp: Vec<(u8, u8, u8)>,
}

impl AmbientCache {
    fn matches(
        &self,
        track_id: &str,
        dims: (u16, u16),
        dimming: u8,
        style: AmbientStyle,
    ) -> bool {
        self.track_id == track_id
            && self.dims == dims
            && self.dimming == dimming
            && self.style == style
    }
}

/// Draw the ambient backdrop underneath the whole Now Playing area.
///
/// Called first in `render_now_playing`, before any pane, the spectrum lane
/// and the transport bar.
pub fn render_ambient(f: &mut Frame, app: &crate::app::App, area: Rect) {
    if !app.settings.ambient_enabled || area.width == 0 || area.height == 0 {
        return;
    }
    let track_id = app
        .audio
        .current_track
        .as_ref()
        .map(|t| t.id.clone())
        .unwrap_or_default();
    let style = app.settings.ambient_style;
    let dimming = app
        .settings
        .ambient_dimming
        .clamp(AMBIENT_DIMMING_MIN, AMBIENT_DIMMING_MAX);
    let dims = (area.width, area.height);

    // Cache hit: draw the prepared backdrop (the immutable borrow guard
    // must drop before draw_cached takes borrow_mut).
    let cache_hit = {
        let cache = app.ambient_cache.borrow();
        cache
            .as_ref()
            .map(|entry| entry.matches(&track_id, dims, dimming, style))
            .unwrap_or(false)
    };
    if cache_hit {
        draw_cached(f, app, area);
        return;
    }

    // Rebuild: pick a source (art → dominant → hue fallback), process the
    // pixels once, prepare the stateful protocol. Every protocol renders the
    // image — graphic terminals at true pixel resolution, half-block
    // terminals as soft cell mosaic (the blur hides the low resolution, the
    // Walkman look works everywhere).
    let art_bytes = app
        .audio
        .current_track
        .as_ref()
        .and_then(|t| app.cover_art_bytes(t));
    let pixel_aspect = backdrop_pixel_aspect(app, area);
    let (source, image, ramp) = build_backdrop(
        app.audio.current_track.as_ref().map(|t| t.title.as_str()),
        art_bytes.as_deref(),
        style,
        dims,
        pixel_aspect,
        dimming,
    );
    let state = image.as_ref().and_then(|img| {
        app.picker
            .as_ref()
            .map(|p| p.new_resize_protocol(img.clone()))
    });

    *app.ambient_cache.borrow_mut() = Some(AmbientCache {
        track_id,
        dims,
        dimming,
        style,
        source,
        state,
        ramp,
    });

    draw_cached(f, app, area);
}

/// Pixels-per-cell aspect of `area` at the picker's font size, so the
/// processed image can be center-cropped to fill the area with no bands.
fn backdrop_pixel_aspect(app: &crate::app::App, area: Rect) -> f32 {
    let (fw, fh) = app
        .picker
        .as_ref()
        .map(|p| {
            let fs = p.font_size();
            (fs.width.max(1), fs.height.max(1))
        })
        .unwrap_or((10, 20));
    (area.width as f32 * fw as f32) / (area.height as f32 * fh as f32).max(1.0)
}

/// Draw the cached backdrop: stateful image (any terminal protocol, incl.
/// the half-blocks cell mosaic), else the tint ramp (empty ramp → draw
/// nothing, the historic flat background).
fn draw_cached(f: &mut Frame, app: &crate::app::App, area: Rect) {
    let mut cache = app.ambient_cache.borrow_mut();
    let Some(entry) = cache.as_mut() else {
        return;
    };
    if let Some(state) = entry.state.as_mut() {
        let widget = ratatui_image::StatefulImage::<StatefulProtocol>::default()
            // Scale (not Fit): the processed backdrop is a deliberately small
            // blurred thumb — Fit never upscales and would render a tiny
            // rectangle; Scale blows it up to the full area for the bleed.
            .resize(Resize::Scale(Some(image::imageops::FilterType::CatmullRom)));
        f.render_stateful_widget(widget, area, state);
        return;
    }
    if !entry.ramp.is_empty() {
        paint_ramp(f.buffer_mut(), area, &entry.ramp);
    }
}

/// Paint the per-row tint as cell background colors across `area`.
fn paint_ramp(buf: &mut ratatui::buffer::Buffer, area: Rect, ramp: &[(u8, u8, u8)]) {
    let last = ramp.len().saturating_sub(1);
    for row in 0..area.height as usize {
        let Some(&(r, g, b)) = ramp.get(row.min(last)) else {
            break;
        };
        let color = Color::Rgb(r, g, b);
        for col in 0..area.width {
            if let Some(cell) = buf.cell_mut((area.x + col, area.y + row as u16)) {
                cell.set_bg(color);
            }
        }
    }
}

/// Decide the backdrop source and produce its pixels.
///
/// Cascade: album art (blurred image on every terminal — true pixels on
/// graphic protocols, soft cell mosaic on half-blocks) → dominant-color
/// tint → title-hue tint → flat (no track loaded).
fn build_backdrop(
    title: Option<&str>,
    art_bytes: Option<&[u8]>,
    style: AmbientStyle,
    cells: (u16, u16),
    pixel_aspect: f32,
    dimming: u8,
) -> (BackdropSource, Option<DynamicImage>, Vec<(u8, u8, u8)>) {
    let rows = cells.1.max(1);

    // Tint from the title hue, shared by every fallback path.
    let hue_tint = |title: &str| {
        let dom = hue_to_pleasing(crate::cover::title_hash_rgb(title));
        (BackdropSource::TitleHue, None, tint_ramp(dom, rows, dimming))
    };

    if let Some(bytes) = art_bytes {
        // Album art style: the full blurred image, on every terminal.
        if style == AmbientStyle::AlbumArt {
            if let Some(img) = prepare_backdrop_image(bytes, pixel_aspect, dimming) {
                return (BackdropSource::BlurredArt, Some(img), Vec::new());
            }
        }
        if let Some(dom) = dominant_rgb_from_bytes(bytes) {
            return (
                BackdropSource::DominantColor,
                None,
                tint_ramp(dom, rows, dimming),
            );
        }
    }

    match title {
        Some(title) => hue_tint(title),
        None => (BackdropSource::TitleHue, None, Vec::new()),
    }
}

/// Decode, downscale to a small thumb, center-crop to the terminal's pixel
/// aspect ratio, blur and darken. `None` when the payload cannot be decoded.
pub fn prepare_backdrop_image(bytes: &[u8], pixel_aspect: f32, dimming: u8) -> Option<DynamicImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (iw, ih) = (img.width(), img.height());
    if iw == 0 || ih == 0 {
        return None;
    }

    // Downscale: longest edge → THUMB_EDGE (the blur hides the lost detail).
    let scale = THUMB_EDGE as f32 / iw.max(ih) as f32;
    let (nw, nh) = if scale < 1.0 {
        (
            ((iw as f32 * scale).round() as u32).max(1),
            ((ih as f32 * scale).round() as u32).max(1),
        )
    } else {
        (iw, ih)
    };
    let thumb = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::CatmullRom);

    // Center-crop to the terminal pixel aspect for a full-bleed fit.
    let cropped = crop_to_aspect(thumb, pixel_aspect);

    // Heavy blur + dim: the ambient wash.
    let blurred = image::imageops::blur(&cropped, BLUR_SIGMA);
    Some(DynamicImage::ImageRgba8(darken(&blurred, dimming)))
}

/// Center-crop `img` to `aspect` (width / height).
fn crop_to_aspect(img: image::RgbaImage, aspect: f32) -> image::RgbaImage {
    let (iw, ih) = (img.width(), img.height());
    let target = aspect.max(0.01);
    let cur = iw as f32 / ih.max(1) as f32;
    let (cw, ch) = if cur > target {
        ((((ih as f32) * target).round() as u32).clamp(1, iw), ih)
    } else {
        (iw, ((((iw as f32) / target).round() as u32).clamp(1, ih)))
    };
    let x = iw.saturating_sub(cw) / 2;
    let y = ih.saturating_sub(ch) / 2;
    image::imageops::crop_imm(&img, x, y, cw, ch).to_image()
}

/// Multiply RGB (not alpha) by the dimming factor.
fn darken(img: &image::RgbaImage, dimming: u8) -> image::RgbaImage {
    let k = dim_factor(dimming);
    let mut out = img.clone();
    for p in out.pixels_mut() {
        p.0[0] = ((p.0[0] as f32) * k).round() as u8;
        p.0[1] = ((p.0[1] as f32) * k).round() as u8;
        p.0[2] = ((p.0[2] as f32) * k).round() as u8;
    }
    out
}

/// Multiplicative brightness from the dimming knob (45% default → 0.55).
fn dim_factor(dimming: u8) -> f32 {
    (1.0 - dimming as f32 / 100.0).clamp(0.10, 0.90)
}

fn dominant_rgb_from_bytes(bytes: &[u8]) -> Option<(u8, u8, u8)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    dominant_rgb(&img)
}

/// Average color of a (small) image, saturation-boosted for vibrancy.
pub fn dominant_rgb(img: &image::RgbaImage) -> Option<(u8, u8, u8)> {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return None;
    }
    let total = (w as u64) * (h as u64);
    let stride = (total / 4096).max(1) as usize; // sample ~≥4096 px
    let mut acc = [0u64; 3];
    let mut n = 0u64;
    for (i, p) in img.pixels().enumerate() {
        if i % stride != 0 {
            continue;
        }
        acc[0] += p.0[0] as u64;
        acc[1] += p.0[1] as u64;
        acc[2] += p.0[2] as u64;
        n += 1;
    }
    if n == 0 {
        return None;
    }
    let r = (acc[0] / n) as f32 / 255.0;
    let g = (acc[1] / n) as f32 / 255.0;
    let b = (acc[2] / n) as f32 / 255.0;
    let (hue, sat, light) = rgb_to_hsl(r, g, b);
    let (r, g, b) = hsl_to_rgb(
        hue,
        (sat * 1.35).clamp(0.0, 1.0),
        light.clamp(0.30, 0.60),
    );
    Some(((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8))
}

/// Per-row tint from a dominant color: brighter at the top, dimmer at the
/// bottom, scaled by the dimming knob.
pub fn tint_ramp(dom: (u8, u8, u8), rows: u16, dimming: u8) -> Vec<(u8, u8, u8)> {
    let rows = rows.max(1) as usize;
    let df = dim_factor(dimming);
    let (hue, _, _) = rgb_to_hsl(
        dom.0 as f32 / 255.0,
        dom.1 as f32 / 255.0,
        dom.2 as f32 / 255.0,
    );
    (0..rows)
        .map(|i| {
            let t = if rows == 1 {
                0.0
            } else {
                i as f32 / (rows - 1) as f32
            };
            let light = ((TINT_TOP - (TINT_TOP - TINT_BOTTOM) * t) * df).clamp(0.02, 0.75);
            let (r, g, b) = hsl_to_rgb(hue, 0.55, light);
            ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
        })
        .collect()
}

/// Map a raw title-hash RGB to a pleasing, saturated mid-tone.
fn hue_to_pleasing(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    let (hue, _, _) = rgb_to_hsl(
        rgb.0 as f32 / 255.0,
        rgb.1 as f32 / 255.0,
        rgb.2 as f32 / 255.0,
    );
    let (r, g, b) = hsl_to_rgb(hue, 0.55, 0.50);
    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let hue = if (max - r).abs() < f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (hue / 6.0, s.clamp(0.0, 1.0), l.clamp(0.0, 1.0))
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s.abs() < f32::EPSILON {
        return (l.clamp(0.0, 1.0), l.clamp(0.0, 1.0), l.clamp(0.0, 1.0));
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue2rgb = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (
        hue2rgb(h + 1.0 / 3.0).clamp(0.0, 1.0),
        hue2rgb(h).clamp(0.0, 1.0),
        hue2rgb(h - 1.0 / 3.0).clamp(0.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(w, h, |_, _| {
            image::Rgba(rgba)
        }));
        let mut png = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    fn mean_rgb(img: &DynamicImage) -> (f32, f32, f32) {
        let img = img.to_rgba8();
        let mut acc = [0u64; 3];
        for p in img.pixels() {
            acc[0] += p.0[0] as u64;
            acc[1] += p.0[1] as u64;
            acc[2] += p.0[2] as u64;
        }
        let n = (img.width() * img.height()) as f32;
        (acc[0] as f32 / n, acc[1] as f32 / n, acc[2] as f32 / n)
    }

    #[test]
    fn backdrop_image_dims_and_darkening() {
        let png = solid_png(64, 64, [220, 60, 60, 255]);
        let bright = prepare_backdrop_image(&png, 1.5, 25).expect("decodes");
        let dark = prepare_backdrop_image(&png, 1.5, 75).expect("decodes");
        assert!(bright.width() > 0 && bright.height() > 0);

        // Heavier dimming means a strictly darker backdrop.
        let (br, bg, bb) = mean_rgb(&bright);
        let (dr, dg, db) = mean_rgb(&dark);
        assert!(dr < br && dg < bg && db < bb, "bright={br}/{bg}/{bb} dark={dr}/{dg}/{db}");

        // Center-crop honors the requested pixel aspect (± tolerance).
        let wide = prepare_backdrop_image(&png, 3.0, 45).expect("decodes");
        let ratio = wide.width() as f32 / wide.height() as f32;
        assert!((ratio - 3.0).abs() < 0.2, "ratio {ratio}");
    }

    #[test]
    fn tint_ramp_bounds_and_monotonic_dimming() {
        let dom = (180, 90, 40);
        let rows = 40;
        let ramp = tint_ramp(dom, rows, 45);
        assert_eq!(ramp.len(), rows as usize);
        // Brighter at the top, dimmer at the bottom.
        assert!(ramp[0].0 >= ramp[rows as usize - 1].0);
        // All channels within u8 range.
        for (r, g, b) in &ramp {
            assert!(r <= &255 && g <= &255 && b <= &255);
        }
        // Heavier dimming darkens the whole ramp.
        let light_mean: f32 = tint_ramp(dom, rows, 25)
            .iter()
            .map(|c| (c.0 as f32 + c.1 as f32 + c.2 as f32) / 3.0)
            .sum::<f32>()
            / rows as f32;
        let dark_mean: f32 = tint_ramp(dom, rows, 75)
            .iter()
            .map(|c| (c.0 as f32 + c.1 as f32 + c.2 as f32) / 3.0)
            .sum::<f32>()
            / rows as f32;
        assert!(dark_mean < light_mean);
        // Ramp length adapts (taller area → more rows).
        assert_eq!(tint_ramp(dom, 80, 45).len(), 80);
    }

    #[test]
    fn dominant_rgb_of_solid_color_is_that_color() {
        let img = image::RgbaImage::from_fn(32, 32, |_, _| image::Rgba([200, 40, 30, 255]));
        let dom = dominant_rgb(&img).expect("non-empty");
        assert!(dom.0 > dom.1 && dom.0 > dom.2, "red dominates: {dom:?}");
        assert!(dom.0 >= 180 && dom.1 <= 120 && dom.2 <= 120, "vivid: {dom:?}");
    }

    #[test]
    fn build_backdrop_cascades() {
        let art = solid_png(32, 32, [40, 120, 220, 255]);

        // Album art style → blurred image (every terminal now renders it).
        let (src, img, ramp) =
            build_backdrop(Some("T"), Some(&art), AmbientStyle::AlbumArt, (100, 40), 1.5, 45);
        assert_eq!(src, BackdropSource::BlurredArt);
        assert!(img.is_some());
        assert!(ramp.is_empty());

        // ColorTint style → dominant tint, never an image.
        let (src, img, ramp) =
            build_backdrop(Some("T"), Some(&art), AmbientStyle::ColorTint, (100, 40), 1.5, 45);
        assert_eq!(src, BackdropSource::DominantColor);
        assert!(img.is_none());
        assert_eq!(ramp.len(), 40);

        // No art + title → hue tint.
        let (src, img, ramp) =
            build_backdrop(Some("Neon Skyline"), None, AmbientStyle::AlbumArt, (100, 40), 1.5, 45);
        assert_eq!(src, BackdropSource::TitleHue);
        assert!(img.is_none());
        assert_eq!(ramp.len(), 40);

        // Nothing playing → flat (empty ramp).
        let (src, img, ramp) =
            build_backdrop(None, None, AmbientStyle::AlbumArt, (100, 40), 1.5, 45);
        assert_eq!(src, BackdropSource::TitleHue);
        assert!(img.is_none());
        assert!(ramp.is_empty());

        // Undecodable art bytes in AlbumArt style → dominant tint is
        // skipped (decode fails twice) → hue fallback.
        let (src, img, ramp) = build_backdrop(
            Some("Fallback"),
            Some(b"not-an-image"),
            AmbientStyle::AlbumArt,
            (100, 40),
            1.5,
            45,
        );
        assert_eq!(src, BackdropSource::TitleHue);
        assert!(img.is_none());
        assert_eq!(ramp.len(), 40);
    }

    #[test]
    fn hsl_roundtrips_primaries() {
        for (r, g, b) in [(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.2, 0.4, 0.8), (0.9, 0.9, 0.1)] {
            let (h, s, l) = rgb_to_hsl(r, g, b);
            let (r2, g2, b2) = hsl_to_rgb(h, s, l);
            assert!((r - r2).abs() < 0.01 && (g - g2).abs() < 0.01 && (b - b2).abs() < 0.01);
        }
    }

    #[test]
    fn title_hue_is_deterministic_per_title() {
        let a = hue_to_pleasing(crate::cover::title_hash_rgb("Daft Punk"));
        let b = hue_to_pleasing(crate::cover::title_hash_rgb("Daft Punk"));
        assert_eq!(a, b);
        // Distinct hues across distinct titles (probabilistically stable).
        let mut distinct = std::collections::HashSet::new();
        for t in ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Fox", "Golf", "Hotel"] {
            distinct.insert(hue_to_pleasing(crate::cover::title_hash_rgb(t)));
        }
        assert!(distinct.len() > 1, "hash should vary across titles");
    }
}
