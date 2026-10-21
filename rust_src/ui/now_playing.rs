use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::ambient;
use crate::app::App;
use crate::cover::CoverRenderer;
use crate::lyrics::LyricsProvider;
use crate::models::Focus;
use crate::settings::ArtSize;

/// Below this many columns the Now Playing panes stack responsively: the
/// lyrics pane auto-hides so art/info and the tracklist get the full width
/// (PWA-like reflow — no clipped info on narrow windows).
const NARROW_STACK_WIDTH: u16 = 100;

pub fn render_now_playing(f: &mut Frame, app: &App, area: Rect) {
    // Ambient backdrop first: blurred album art / tint ramp underneath
    // every widget below (widgets draw over it; ratatui's patch semantics
    // keep the backdrop's cell backgrounds wherever a widget does not paint
    // its own background).
    ambient::render_ambient(f, app, area);

    // Split area into:
    // 1. Panes main area (Min 14)
    // 2. Spectrum lane (Length 6 while the cava visualizer is on, else 0)
    // 3. Bottom transport & progress bar (Length 4)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(14),
            Constraint::Length(spectrum_lane_height(app)),
            Constraint::Length(4),
        ])
        .split(area);

    let panes_area = main_chunks[0];
    let spectrum_area = main_chunks[1];
    let bottom_bar_area = main_chunks[2];

    // Horizontal: Left column and Right column (50/50). The lyrics pane
    // auto-hides on narrow windows (responsive stacking: art/info and the
    // tracklist take the full width, nothing cramped), and stays hidden when
    // disabled in Settings.
    let show_lyrics = app.settings.show_lyrics_pane && panes_area.width >= NARROW_STACK_WIDTH;
    let cols = if show_lyrics {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(panes_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(panes_area)
    };

    let left_col = cols[0];

    // Left column:
    // - Top: Album art + song info (everything the playlist pane doesn't need)
    // - Bottom: Album / Playlist tracklist, sized to fit its track count so
    //   the album art grows instead of the pane swallowing a fixed 40%.
    //   (the old bottom-left Categories browser was removed in v0.3;
    //    category browsing lives in the Categories tab)
    let playlist_height = playlist_pane_height(
        app.active_playlist_tracks.len(),
        left_col.height,
        app.settings.art_size,
    );
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(playlist_height),
        ])
        .split(left_col);

    let left_top_area = left_rows[0];
    let left_bottom_area = left_rows[1];

    // Right column: synchronized lyrics, full height
    render_left_top_cover_and_info(f, app, left_top_area);
    render_left_bottom_tracklist(f, app, left_bottom_area);
    if show_lyrics {
        render_right_lyrics(f, app, cols[1]);
    }

    // Spectrum lane (between the panes and the transport bar)
    render_spectrum(f, app, spectrum_area);

    // Render Bottom Transport Bar
    render_bottom_controls_bar(f, app, bottom_bar_area);
}

/// Height of the cava spectrum lane: 6 rows while the visualizer is enabled
/// in Settings (collapses to 0 when disabled or when `cava` is missing, so
/// the panes regain the space).
pub fn spectrum_lane_height(app: &App) -> u16 {
    if app.settings.visualizer_enabled && !app.viz_missing() {
        6
    } else {
        0
    }
}

/// cava spectrum bars: one column per cell, frame values sampled across the
/// terminal width, drawn with eighth-block sub-cell resolution. Bar bodies
/// set only the foreground color, so the ambient backdrop shows through
/// behind and between the bars.
fn render_spectrum(f: &mut Frame, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let Some(frame) = &app.viz_frame else {
        return;
    };
    if frame.is_empty() || area.width < 16 {
        return;
    }

    let accent = app.accent();
    let bars = frame.len();
    let width = area.width as usize;
    let height = area.height as usize;
    let buf = f.buffer_mut();

    for col in 0..width {
        // Sample the frame across the width (nearest bar per column).
        let idx = (col * bars) / width;
        let v = frame[idx.min(bars - 1)];
        if v <= 0.0 {
            continue;
        }
        let lit = (v * height as f32).clamp(0.0, height as f32);
        let full_cells = lit.floor() as usize;
        let frac = lit - full_cells as f32;

        for row in 0..height {
            let from_bottom = height - 1 - row; // draw bottom-up
            let (symbol, strength) = if from_bottom < full_cells {
                ("█", body_brightness(from_bottom, height))
            } else if from_bottom == full_cells && frac > 0.08 {
                (eighth_glyph(frac), 1.0)
            } else {
                continue;
            };
            if let Some(cell) = buf.cell_mut((area.x + col as u16, area.y + row as u16)) {
                cell.set_symbol(symbol);
                cell.set_fg(scale_rgb(accent, strength));
            }
        }
    }
}

/// Body brightness grows toward the bar tip (row 0 = the tip row).
fn body_brightness(from_bottom: usize, height: usize) -> f32 {
    0.45 + 0.50 * ((from_bottom + 1) as f32 / height.max(1) as f32)
}

/// Eighth-block glyph for the partially-lit tip cell.
fn eighth_glyph(frac: f32) -> &'static str {
    const GLYPHS: [&str; 7] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇"];
    let idx = ((frac * 7.0) as usize).min(GLYPHS.len() - 1);
    GLYPHS[idx]
}

/// Scale an RGB color by `k` (named colors pass through untouched).
fn scale_rgb(color: Color, k: f32) -> Color {
    match color {
        Color::Rgb(r, g, b) => Color::Rgb(
            ((r as f32) * k).round() as u8,
            ((g as f32) * k).round() as u8,
            ((b as f32) * k).round() as u8,
        ),
        other => other,
    }
}

/// Height of the bottom-left playlist pane: exactly `tracks + borders` when
/// that fits, clamped to [3, `available * art_size cap`]. A smaller cap
/// reserves more room (resolution) for the album art.
pub fn playlist_pane_height(track_count: usize, available: u16, art_size: ArtSize) -> u16 {
    let wanted = (track_count as u16).saturating_add(2); // + top/bottom border
    let cap = (available * art_size.playlist_cap_percent() / 100).max(3);
    wanted.min(cap).max(3)
}

/// First visible list row for a selection kept within a scrolling window of
/// `visible` rows (standard bottom-edge clamping).
pub fn scroll_start(selected: usize, visible: usize) -> usize {
    if visible == 0 {
        0
    } else if selected < visible {
        0
    } else {
        selected + 1 - visible
    }
}

/// [Left Bottom]: Album / Playlist tracklist
fn render_left_bottom_tracklist(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::NowTracklist;
    let border_style = if is_focused {
        Style::default().fg(app.accent())
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(
        " 🎵 Playlist: {} ({} tracks) ",
        app.active_category_name,
        app.active_playlist_tracks.len()
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if is_focused { BorderType::Double } else { BorderType::Rounded })
        .border_style(border_style)
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.active_playlist_tracks.is_empty() {
        let msg = Paragraph::new("No tracks in this category.\nPick one in the Categories tab (2).")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let current_track_id = app.audio.current_track.as_ref().map(|t| t.id.as_str());
    let visible = inner.height as usize;
    let start = scroll_start(app.selected_tracklist_idx, visible);

    let mut items = Vec::new();

    for (pos, &track_idx) in app
        .active_playlist_tracks
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
        if let Some(track) = app.library.tracks.get(track_idx) {
            let is_selected = is_focused && pos == app.selected_tracklist_idx;
            let is_now_playing = current_track_id == Some(track.id.as_str());

            let icon = if is_now_playing {
                if app.audio.is_playing { "▶ " } else { "⏸ " }
            } else {
                "  "
            };

            let num = track.track_number.map(|n| format!("{:02}. ", n)).unwrap_or_else(|| format!("{:02}. ", pos + 1));
            let title = &track.title;
            let artist = track.artist_display();
            let dur = track.duration_formatted();

            // Responsive columns: fixed overhead (icon 2 + num 4 + gaps 2 +
            // duration) leaves title/artist to split the rest; the artist
            // column disappears entirely on very narrow panes.
            let inner_w = inner.width as usize;
            let overhead = 2 + 4 + 2 + dur.chars().count().max(5);
            let remaining = inner_w.saturating_sub(overhead);
            let artist_w = if remaining >= 30 { 18.min(remaining / 3) } else { 0 };
            let title_w = remaining.saturating_sub(artist_w).max(8);

            let mut spans = vec![
                Span::styled(icon.to_string(), if is_now_playing { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() }),
                Span::styled(num, Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<width$} ", truncate_str(title, title_w), width = title_w),
                    if is_now_playing { Style::default().fg(app.accent()).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) },
                ),
            ];
            if artist_w > 0 {
                spans.push(Span::styled(
                    format!("{:<width$} ", truncate_str(artist, artist_w), width = artist_w),
                    Style::default().fg(Color::Gray),
                ));
            }
            spans.push(Span::styled(dur, Style::default().fg(Color::DarkGray)));

            let item_style = if is_selected {
                Style::default().bg(Color::Rgb(30, 45, 65)).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            items.push(ListItem::new(Line::from(spans)).style(item_style));
        }
    }

    let list = List::new(items);
    f.render_widget(list, inner);
}

/// [Left Top]: Album art (hi-res via terminal graphics protocol) with the
/// quality badge and song info directly beneath.
fn render_left_top_cover_and_info(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" 💽 Album Art ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 10 || inner.height < 6 {
        return;
    }

    // Split inner vertically:
    // - Top: Album Artwork
    // - Bottom: Quality badge + Music Info (Title, Artist•Album, Specs)
    let info_height = 5.min(inner.height.saturating_sub(4));
    let art_height = inner.height.saturating_sub(info_height);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(art_height),
            Constraint::Length(info_height),
        ])
        .split(inner);

    let art_area = rows[0];
    let info_area = rows[1];

    // 1. Render album art (ratatui-image protocol, centered; vinyl fallback)
    if let Some(track) = &app.audio.current_track {
        // Invalidate the cache when the track changed (cheap id check in
        // render_cover; here we just call it — it manages the cache).
        CoverRenderer::render_cover(f, app, art_area, track);
    } else {
        CoverRenderer::render_vinyl_placeholder(f.buffer_mut(), art_area, "Vinyl");
    }

    // 2. Render Quality badge + Music Info DIRECTLY BELOW the art
    if let Some(track) = &app.audio.current_track {
        // Quality badge: ∿∿∿ Hi-Res Lossless (gold) / ∿∿∿ Lossless (cyan) / LOSSY (gray)
        let badge = track.quality_tier().badge();
        let badge_color = match track.quality_tier() {
            crate::models::QualityTier::HiResLossless => Color::Rgb(255, 215, 0),
            crate::models::QualityTier::Lossless => app.accent(),
            crate::models::QualityTier::Lossy => Color::DarkGray,
        };
        let badge_line = Line::from(Span::styled(
            badge,
            Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
        ));

        let title_line = Line::from(vec![
            Span::styled("♪ ", Style::default().fg(app.accent()).add_modifier(Modifier::BOLD)),
            Span::styled(
                truncate_str(&track.title, (info_area.width as usize).saturating_sub(2).max(4)),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let artist_album_line = build_artist_album_line(track, app, info_area.width as usize);

        // Specs: width-tiered — Format │ Depth/Rate │ Bitrate │ Length │
        // Driver, dropping pieces and compacting as the pane narrows so
        // nothing is ever cut off mid-character.
        let specs_line = build_specs_line(info_area.width as usize, track, app);

        let info_paragraph = Paragraph::new(vec![
            badge_line,
            title_line,
            artist_album_line,
            specs_line,
        ])
        .alignment(Alignment::Center);

        f.render_widget(info_paragraph, info_area);
    } else {
        let empty_msg = Paragraph::new("No track loaded. Select a song from the playlist below.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty_msg, info_area);
    }
}

/// Width-tiered specs line, richest first — the first tier that fits the
/// available columns wins (progressive disclosure, PWA-style):
/// 1. `Format: ALAC │ 16-bit/44.1 kHz │ 1040 kbps │ 03:23 │ driver`
/// 2. drops the driver, 3. drops the bitrate, 4. compact `ALAC · 44.1k/16b ·
/// 03:23`, 5. bare `ALAC · 03:23`.
fn build_specs_line(avail: usize, track: &crate::models::Track, app: &App) -> Line<'static> {
    let sep_st = Style::default().fg(Color::Rgb(50, 50, 60));
    let codec_st = Style::default().fg(Color::Yellow);
    let depth_st = Style::default().fg(Color::Green);
    let dur_st = Style::default().fg(app.accent());
    let bitrate_st = Style::default().fg(Color::Green);
    let driver_st = Style::default().fg(Color::Magenta);

    let codec = track.codec.clone();
    let depth_rate = track.depth_rate_str();
    let dur = track.duration_formatted();
    let driver = app.audio.output_driver.clone();

    // (piece, style) pairs per tier; wide tiers use "  │  ", compact " · ".
    let wide_sep = "  │  ";
    let compact_sep = " · ";
    let tiers: [(Vec<(String, Style)>, &str); 5] = [
        (
            vec![
                (format!("Format: {codec}"), codec_st),
                (depth_rate.clone(), depth_st),
                (format!("{}", track.bitrate), bitrate_st),
                (dur.clone(), dur_st),
                (driver.clone(), driver_st),
            ],
            wide_sep,
        ),
        (
            vec![
                (format!("Format: {codec}"), codec_st),
                (depth_rate.clone(), depth_st),
                (format!("{}", track.bitrate), bitrate_st),
                (dur.clone(), dur_st),
            ],
            wide_sep,
        ),
        (
            vec![
                (format!("Format: {codec}"), codec_st),
                (depth_rate.clone(), depth_st),
                (dur.clone(), dur_st),
            ],
            wide_sep,
        ),
        (
            vec![
                (codec.clone(), codec_st),
                (compact_depth_rate(track), depth_st),
                (dur.clone(), dur_st),
            ],
            compact_sep,
        ),
        (vec![(codec.clone(), codec_st), (dur.clone(), dur_st)], compact_sep),
    ];

    for (pieces, sep) in tiers {
        // Empty pieces (unknown depth/rate, no metadata) drop out before the
        // width math, so no dangling separator is left behind.
        let pieces: Vec<(String, Style)> =
            pieces.into_iter().filter(|(t, _)| !t.is_empty()).collect();
        let total: usize = pieces.iter().map(|(s, _)| s.chars().count()).sum::<usize>()
            + sep.chars().count() * pieces.len().saturating_sub(1);
        if total <= avail {
            let mut spans = Vec::with_capacity(pieces.len() * 2);
            for (i, (text, style)) in pieces.into_iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(sep.to_string(), sep_st));
                }
                spans.push(Span::styled(text, style));
            }
            return Line::from(spans);
        }
    }
    // Absurdly narrow pane: bare codec + duration, clipped gracefully.
    Line::from(vec![
        Span::styled(codec, codec_st),
        Span::styled(compact_sep.to_string(), sep_st),
        Span::styled(dur, dur_st),
    ])
}

/// "44.1k/16b"-style compact depth/rate for the narrow specs tier.
fn compact_depth_rate(track: &crate::models::Track) -> String {
    let rate = track.sample_rate_khz().map(|k| format!("{k}k"));
    let depth = track.bit_depth.map(|d| format!("{d}b"));
    match (depth, rate) {
        (Some(d), Some(r)) => format!("{r}/{d}"),
        (Some(d), None) => d,
        (None, Some(r)) => r,
        (None, None) => String::new(),
    }
}

/// Artist • Album line that never clips: keeps the artist whole, ellipsizes
/// the album first, then falls back to the artist alone.
fn build_artist_album_line<'a>(
    track: &'a crate::models::Track,
    app: &'a App,
    avail: usize,
) -> Line<'a> {
    let artist = track.artist_display();
    let album = track.album_display();
    let sep = "  •  ";

    let artist_len = artist.chars().count();
    let album_len = album.chars().count();
    let sep_len = sep.chars().count();

    if artist_len + sep_len + album_len <= avail {
        return Line::from(vec![
            Span::styled(artist.to_string(), Style::default().fg(app.accent())),
            Span::styled(sep.to_string(), Style::default().fg(Color::DarkGray)),
            Span::styled(album.to_string(), Style::default().fg(Color::Gray)),
        ]);
    }
    if artist_len + sep_len + 2 <= avail {
        let album_w = avail - artist_len - sep_len;
        return Line::from(vec![
            Span::styled(artist.to_string(), Style::default().fg(app.accent())),
            Span::styled(sep.to_string(), Style::default().fg(Color::DarkGray)),
            Span::styled(truncate_str(album, album_w), Style::default().fg(Color::Gray)),
        ]);
    }
    Line::from(vec![Span::styled(
        truncate_str(artist, avail.max(4)),
        Style::default().fg(app.accent()),
    )])
}

/// [Right]: Synchronized lyrics, full height
fn render_right_lyrics(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == Focus::NowLyrics;
    let border_style = if is_focused {
        Style::default().fg(app.accent())
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if is_focused { BorderType::Double } else { BorderType::Rounded })
        .border_style(border_style)
        .title(" 📜 Synchronized Lyrics ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(track) = &app.audio.current_track else {
        let msg = Paragraph::new("Play a track to view synced lyrics.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    };

    if track.lyrics.is_empty() {
        let msg = Paragraph::new("No lyrics available for this track.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let active_idx = LyricsProvider::current_line_index(&track.lyrics, app.audio.position_secs);
    let view_height = inner.height as usize;
    let half_view = view_height / 2;

    let start_idx = active_idx.saturating_sub(half_view);
    let end_idx = (start_idx + view_height).min(track.lyrics.len());

    let mut lines = Vec::new();
    for i in start_idx..end_idx {
        if let Some(lyric) = track.lyrics.get(i) {
            let is_active = i == active_idx;
            if is_active {
                lines.push(Line::from(vec![
                    Span::styled("▶ ", Style::default().fg(app.accent()).add_modifier(Modifier::BOLD)),
                    Span::styled(
                        &lyric.text,
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(25, 40, 60))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                let dist = (i as isize - active_idx as isize).abs();
                let color = if dist <= 2 {
                    Color::Gray
                } else {
                    Color::DarkGray
                };
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(&lyric.text, Style::default().fg(color)),
                ]));
            }
        }
    }

    let paragraph = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}

/// Persistent bottom transport bar
fn render_bottom_controls_bar(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Rgb(40, 40, 50)));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Progress bar
            Constraint::Length(1), // Controls & Shortcuts
        ])
        .split(inner);

    // 1. Progress Bar
    let cur_pos = app.audio.position_secs;
    let dur = app.audio.duration_secs as f64;
    let ratio = if dur > 0.0 { (cur_pos / dur).clamp(0.0, 1.0) } else { 0.0 };

    let cur_mins = (cur_pos as u64) / 60;
    let cur_secs = (cur_pos as u64) % 60;
    let total_mins = (app.audio.duration_secs) / 60;
    let total_secs = (app.audio.duration_secs) % 60;

    let time_str_left = format!("{:02}:{:02} ", cur_mins, cur_secs);
    let time_str_right = format!(" {:02}:{:02}", total_mins, total_secs);

    let bar_width = (rows[0].width as usize)
        .saturating_sub(time_str_left.len() + time_str_right.len() + 2);

    let filled_len = ((bar_width as f64) * ratio).round() as usize;
    let unfilled_len = bar_width.saturating_sub(filled_len);

    let filled_bar = "━".repeat(filled_len);
    let unfilled_bar = "─".repeat(unfilled_len);

    let progress_line = Line::from(vec![
        Span::styled(time_str_left, Style::default().fg(app.accent())),
        Span::styled(filled_bar, Style::default().fg(app.accent())),
        Span::styled("●", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(unfilled_bar, Style::default().fg(Color::DarkGray)),
        Span::styled(time_str_right, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(progress_line), rows[0]);

    // 2. Status & Shortcuts Line
    let status_str = if app.audio.is_playing { "▶ PLAYING" } else { "⏸ PAUSED" };
    let status_color = if app.audio.is_playing { Color::Green } else { Color::Yellow };

    let repeat_badge = format!("🔁 {}", app.audio.repeat_mode.label());
    let shuffle_badge = if app.audio.shuffle { "🔀 On" } else { "🔀 Off" };

    // Shortcuts hint reflecting the user's (possibly rebound) hotkeys.
    let hk = |action: crate::settings::HotkeyAction| -> String {
        let c = app.settings.hotkey_char(action);
        if c == ' ' {
            "Space".to_string()
        } else {
            c.to_string()
        }
    };
    let default_hint = format!(
        "[{}] Open Folder  [{}] Scan  [{}] Song Menu  [{}] Play/Pause  [{}/{}] Next/Prev  [←/→] Seek  [↑/↓] Volume  [Tab] Focus  [q] Quit",
        hk(crate::settings::HotkeyAction::OpenFolder),
        hk(crate::settings::HotkeyAction::Rescan),
        hk(crate::settings::HotkeyAction::SongMenu),
        hk(crate::settings::HotkeyAction::PlayPause),
        hk(crate::settings::HotkeyAction::PrevTrack),
        hk(crate::settings::HotkeyAction::NextTrack),
    );
    let notif = app
        .get_active_notification()
        .unwrap_or(default_hint.as_str());

    let controls_line = Line::from(vec![
        Span::styled(format!(" {} ", status_str), Style::default().fg(Color::Black).bg(status_color).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(format!(" {} ", repeat_badge), Style::default().fg(Color::Gray).bg(Color::Rgb(25, 25, 35))),
        Span::raw(" "),
        Span::styled(format!(" {} ", shuffle_badge), Style::default().fg(Color::Gray).bg(Color::Rgb(25, 25, 35))),
        Span::raw("  │  "),
        Span::styled(notif, Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(Paragraph::new(controls_line), rows[1]);
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let mut out: String = s.chars().take(max_len.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Track;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn make_track(codec: &str, sample_rate: Option<u32>, bit_depth: Option<u8>) -> Track {
        Track {
            id: format!("t-{codec}"),
            path: PathBuf::from("/tmp/x.m4a"),
            title: "Neon Skyline".into(),
            artist: "The Wanderer".into(),
            album: "Night Drive".into(),
            album_artist: "The Wanderer".into(),
            genre: "Synth".into(),
            year: Some(2024),
            track_number: Some(1),
            duration_secs: 200,
            codec: codec.into(),
            bitrate: "1873 kbps".into(),
            sample_rate,
            bit_depth,
            channels: Some(2),
            cover_art: None,
            lyrics: Vec::new(),
        }
    }

    fn app_with_track(track: Track) -> App {
        let tmp = std::env::temp_dir().join(format!("walkz-ui-np-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mut app = App::new_in(tmp, None);
        app.library.tracks = vec![track.clone()];
        app.active_playlist_tracks = vec![0];
        app.audio.current_track = Some(track);
        app
    }

    fn render_to_text(app: &App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_now_playing(f, app, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let area = buf.area;
        let mut out = String::new();
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn hires_track_shows_gold_badge_and_specs() {
        let app = app_with_track(make_track("ALAC", Some(96000), Some(24)));
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("Hi-Res Lossless"), "badge line:\n{text}");
        assert!(text.contains("24-bit/96 kHz"), "spec line:\n{text}");
        assert!(!text.contains("LOSSY"));
        // Gold color on the wave glyph
        let buf_cell_gold = {
            let backend = TestBackend::new(110, 40);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| render_now_playing(f, &app, f.area())).unwrap();
            let buf = terminal.backend().buffer();
            buf.content
                .iter()
                .find(|c| c.symbol() == "∿")
                .map(|c| c.fg)
        };
        assert_eq!(buf_cell_gold, Some(ratatui::style::Color::Rgb(255, 215, 0)));
    }

    #[test]
    fn lossless_16bit_shows_cyan_badge_without_hires() {
        let app = app_with_track(make_track("FLAC", Some(44100), Some(16)));
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("∿∿∿ Lossless"), "badge:\n{text}");
        assert!(!text.contains("Hi-Res"), "no hi-res for 16/44.1:\n{text}");
        assert!(text.contains("16-bit/44.1 kHz"));
    }

    #[test]
    fn lossy_track_shows_gray_badge() {
        let app = app_with_track(make_track("MP3", Some(44100), None));
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("LOSSY"), "badge:\n{text}");
        assert!(!text.contains("Lossless"));
    }

    #[test]
    fn now_playing_has_no_categories_quadrant() {
        let app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        let text = render_to_text(&app, 110, 40);
        assert!(!text.contains("📂"), "old quadrant gone:\n{text}");
        // New layout pieces present: art pane, tracklist, lyrics
        assert!(text.contains("Album Art"), "{text}");
        assert!(text.contains("Playlist:"), "{text}");
        assert!(text.contains("Synchronized Lyrics"), "{text}");
    }

    #[test]
    fn playlist_pane_height_fits_and_caps() {
        use crate::settings::ArtSize;
        // Small album: exact fit (tracks + 2 border rows).
        assert_eq!(playlist_pane_height(8, 40, ArtSize::Medium), 10);
        // Big playlist: capped by the art-size share of the column.
        assert_eq!(playlist_pane_height(500, 40, ArtSize::Medium), 18); // 45%
        // Art size drives the cap: Large art = smaller playlist cap.
        assert_eq!(playlist_pane_height(500, 40, ArtSize::Large), 12); // 30%
        assert_eq!(playlist_pane_height(500, 40, ArtSize::Small), 24); // 60%
        // Never smaller than one content row.
        assert_eq!(playlist_pane_height(0, 40, ArtSize::Medium), 3);
    }

    #[test]
    fn scroll_start_keeps_selection_visible() {
        assert_eq!(scroll_start(0, 10), 0);
        assert_eq!(scroll_start(9, 10), 0);
        assert_eq!(scroll_start(10, 10), 1);
        assert_eq!(scroll_start(25, 10), 16);
        // Moving back above the window snaps to the top.
        assert_eq!(scroll_start(3, 10), 0);
        // Degenerate area.
        assert_eq!(scroll_start(7, 0), 0);
    }

    #[test]
    fn small_album_playlist_pane_fits_track_count() {
        // 3-track album: the playlist pane shrinks to fit, leaving the art
        // more room than the old fixed 40% split.
        let mut app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        let mut t1 = make_track("ALAC", Some(44100), Some(16));
        let mut t2 = make_track("MP3", Some(44100), None);
        let mut t3 = make_track("FLAC", Some(48000), Some(24));
        t1.track_number = Some(1);
        t2.track_number = Some(2);
        t3.track_number = Some(3);
        app.library.tracks = vec![t1, t2, t3];
        app.active_playlist_tracks = vec![0, 1, 2];
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("Playlist:"), "{text}");
        // All three track titles visible (they are the same title here, so
        // check the numbered entries exist).
        assert!(text.contains("01."), "{text}");
        assert!(text.contains("03."), "{text}");
    }

    #[test]
    fn hidden_lyrics_pane_gives_left_column_full_width() {
        let mut app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        app.settings.show_lyrics_pane = false;
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("Album Art"), "{text}");
        assert!(text.contains("Playlist:"), "{text}");
        assert!(!text.contains("Synchronized Lyrics"), "{text}");
    }

    #[test]
    fn cover_bytes_render_via_ratatui_image_protocol_and_cache() {
        let mut track = make_track("ALAC", Some(44100), Some(16));
        // Tiny 8x8 opaque red PNG
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_fn(8, 8, |_, _| {
            image::Rgba([200, 30, 30, 255])
        }));
        let mut png = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut png),
            image::ImageFormat::Png,
        )
        .unwrap();
        track.cover_art = Some(png);

        let mut app = app_with_track(track);
        app.picker = Some(ratatui_image::picker::Picker::halfblocks());

        let text = render_to_text(&app, 110, 40);
        // Halfblocks protocol draws with upper-half-block glyphs
        assert!(text.contains('▀') || text.contains('▄') || text.contains('█'),
            "art rendered:\n{text}");
        // Cached per-track
        assert!(app.cover_cache.borrow().is_some(), "cover cache populated");

        // Second render hits the cache (no panic, still rendered)
        let text2 = render_to_text(&app, 110, 40);
        assert!(text2.contains('▀') || text2.contains('▄') || text2.contains('█'));
    }

    #[test]
    fn spectrum_lane_height_follows_settings_and_availability() {
        let app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        assert_eq!(spectrum_lane_height(&app), 6);
        let mut off = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        off.settings.visualizer_enabled = false;
        assert_eq!(spectrum_lane_height(&off), 0);
        let mut missing = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        missing.viz_unavailable = true;
        assert_eq!(spectrum_lane_height(&missing), 0);
    }

    #[test]
    fn spectrum_draws_bars_from_the_latest_frame() {
        let mut app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        app.viz_frame = Some(vec![1.0; 48]);
        let (buf, w, h) = {
            let backend = TestBackend::new(110, 40);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| render_now_playing(f, &app, f.area())).unwrap();
            let buf = terminal.backend().buffer().clone();
            (buf, 110usize, 40usize)
        };
        // The bottom-most spectrum row (above the 4-row transport bar) is
        // fully lit '█' columns in the accent color for a full-scale frame.
        let lane_bottom = h - 1 - 4; // last row of the 6-row lane
        let lit: Vec<_> = buf
            .content
            .iter()
            .enumerate()
            .filter(|(i, c)| {
                let (x, y) = (i % w, i / w);
                y == lane_bottom && x >= 2 && x < w - 2 && c.symbol() == "█"
            })
            .collect();
        assert!(lit.len() > 20, "expected a filled spectrum row, got {} bars", lit.len());
    }

    #[test]
    fn ambient_tint_sets_cell_backgrounds_when_enabled() {
        // Halfblocks picker + no cover art: ambient falls back to the
        // title-hue tint ramp (pure cell background colors).
        let mut app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        app.picker = Some(ratatui_image::picker::Picker::halfblocks());
        let backend = TestBackend::new(110, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_now_playing(f, &app, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let tinted = buf
            .content
            .iter()
            .filter(|c| c.symbol() == " " && c.bg != Color::Reset)
            .count();
        assert!(tinted > 100, "tint ramp should color empty cells: {}", tinted);
    }

    /// Cells painted with a background on spaces — the ambient tint's
    /// footprint (minus the legitimate widget backgrounds like selection
    /// highlights, which appear in both baselines).
    fn tinted_space_count(app: &App) -> usize {
        let backend = TestBackend::new(110, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_now_playing(f, app, f.area())).unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .filter(|c| c.symbol() == " " && c.bg != Color::Reset)
            .count()
    }

    #[test]
    fn ambient_off_leaves_cell_backgrounds_untouched() {
        // Halfblocks picker + no cover art: the ambient tint must add a
        // large number of colored spaces when enabled and return exactly to
        // the widget-background baseline when disabled.
        let mut off = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        off.picker = Some(ratatui_image::picker::Picker::halfblocks());
        off.settings.ambient_enabled = false;
        let baseline = tinted_space_count(&off);

        let mut on = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        on.picker = Some(ratatui_image::picker::Picker::halfblocks());
        let tinted = tinted_space_count(&on);
        assert!(tinted > baseline + 300, "on={tinted} baseline={baseline}");

        // Disabled again → back to the baseline.
        on.settings.ambient_enabled = false;
        on.invalidate_ambient();
        assert_eq!(tinted_space_count(&on), baseline);
    }

    #[test]
    fn ambient_blurred_art_backdrop_renders_on_halfblocks() {
        // Cover art present + halfblocks picker: AlbumArt style now builds
        // the blurred image on EVERY protocol — the half-blocks cell mosaic
        // paints (fg+bg) nearly the whole Now Playing area, the Walkman
        // look without a graphics protocol.
        let mut png = Vec::new();
        let img = image::RgbaImage::from_fn(24, 24, |x, _| {
            image::Rgba([(x * 8) as u8, 40, 200, 255])
        });
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let mut track = make_track("ALAC", Some(44100), Some(16));
        track.cover_art = Some(png);
        let mut app = app_with_track(track);
        app.picker = Some(ratatui_image::picker::Picker::halfblocks());
        assert!(app.settings.ambient_enabled);

        let backend = ratatui::backend::TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_now_playing(f, &app, f.area())).unwrap();
        let painted = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .filter(|c| c.bg != Color::Reset)
            .count();
        assert!(painted > 2000, "backdrop-painted cells: {painted}/2400");
    }

    #[test]
    fn specs_line_tiers_progressive_disclosure() {
        let app = app_with_track(make_track("ALAC", Some(96000), Some(24)));
        let track = app.audio.current_track.as_ref().unwrap();

        let joined = |avail: usize| {
            build_specs_line(avail, track, &app)
                .spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        };

        // Huge budget: full tier — Format, depth/rate, bitrate, length, driver.
        let full = joined(400);
        assert!(full.contains("Format: ALAC"), "{full}");
        assert!(full.contains("24-bit/96 kHz"), "{full}");
        assert!(full.contains("1873 kbps"), "{full}");
        assert!(full.contains("03:20"), "{full}");

        // Narrow budget: compact tier — no "Format:", slash-compact rate/depth.
        let compact = joined(30);
        assert!(compact.contains("ALAC · "), "{compact}");
        assert!(compact.contains("96k/24b"), "{compact}");
        assert!(compact.contains("03:20"), "{compact}");
        assert!(!compact.contains("Format:"), "{compact}");
        assert!(!compact.contains("1873 kbps"), "{compact}");

        // Ultra-narrow budget: bare codec + length.
        let bare = joined(12);
        assert_eq!(bare, "ALAC · 03:20", "{bare}");

        // Track with no depth/rate metadata: no dangling separator.
        let lossy = app_with_track(make_track("MP3", None, None));
        let track = lossy.audio.current_track.as_ref().unwrap();
        let joined = build_specs_line(400, track, &lossy)
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>();
        assert!(joined.contains("Format: MP3"), "{joined}");
        assert!(!joined.contains(" ·  "), "{joined}");
        assert!(!joined.contains("│  │"), "{joined}");
    }

    #[test]
    fn lyrics_pane_auto_hides_below_narrow_threshold() {
        let app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        assert!(app.settings.show_lyrics_pane);

        // Wide window: lyrics pane present.
        let wide = render_to_text(&app, 120, 40);
        assert!(wide.contains("Synchronized Lyrics"), "{wide}");

        // Narrow window: auto-hidden even though the setting is still on.
        let narrow = render_to_text(&app, 90, 40);
        assert!(!narrow.contains("Synchronized Lyrics"), "{narrow}");
    }

    #[test]
    fn tracklist_duration_survives_narrow_panes() {
        let app = app_with_track(make_track("ALAC", Some(44100), Some(16)));
        // 46-col window: the panes take the full width, inner ≈ 44 — the old
        // fixed 26/18-col title/artist pushed the duration off the edge.
        let backend = ratatui::backend::TestBackend::new(46, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_now_playing(f, &app, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        let row_strings: Vec<String> = (0..buf.area.height)
            .map(|y| {
                // Columns strictly inside the pane borders: the trailing
                // border glyph after the duration must not fool ends_with.
                (1..buf.area.width.saturating_sub(1))
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect();
        let row = row_strings
            .iter()
            .find(|r| r.contains("01.") && r.contains("Neon"))
            .expect("track row visible");
        // The row ends exactly at the duration: nothing clipped.
        assert!(
            row.trim_end().ends_with("03:20"),
            "duration clipped: {row:?}"
        );
    }
}
