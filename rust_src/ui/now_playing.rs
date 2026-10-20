use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::cover::CoverRenderer;
use crate::lyrics::LyricsProvider;
use crate::models::QuadrantFocus;

pub fn render_now_playing(f: &mut Frame, app: &App, area: Rect) {
    // Split area into:
    // 1. Panes main area (Min 14)
    // 2. Bottom transport & progress bar (Length 4)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(14),
            Constraint::Length(4),
        ])
        .split(area);

    let panes_area = main_chunks[0];
    let bottom_bar_area = main_chunks[1];

    // Horizontal: Left column (50%) and Right column (50%)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(panes_area);

    let left_col = cols[0];
    let right_col = cols[1];

    // Left column:
    // - Top: Album art + song info (60%)
    // - Bottom: Album / Playlist tracklist (40%)
    //   (the old bottom-left Categories browser was removed in v0.3;
    //    category browsing lives in the Categories tab)
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(40),
        ])
        .split(left_col);

    let left_top_area = left_rows[0];
    let left_bottom_area = left_rows[1];

    // Right column: synchronized lyrics, full height
    render_left_top_cover_and_info(f, app, left_top_area);
    render_left_bottom_tracklist(f, app, left_bottom_area);
    render_right_lyrics(f, app, right_col);

    // Render Bottom Transport Bar
    render_bottom_controls_bar(f, app, bottom_bar_area);
}

/// [Left Bottom]: Album / Playlist tracklist
fn render_left_bottom_tracklist(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.quadrant_focus == QuadrantFocus::Tracklist;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
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
    let mut items = Vec::new();

    for (pos, &track_idx) in app.active_playlist_tracks.iter().enumerate() {
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

            let line = Line::from(vec![
                Span::styled(icon, if is_now_playing { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default() }),
                Span::styled(num, Style::default().fg(Color::DarkGray)),
                Span::styled(format!("{:<26} ", truncate_str(title, 26)), if is_now_playing { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::White) }),
                Span::styled(format!("{:<18} ", truncate_str(artist, 18)), Style::default().fg(Color::Gray)),
                Span::styled(dur, Style::default().fg(Color::DarkGray)),
            ]);

            let item_style = if is_selected {
                Style::default().bg(Color::Rgb(30, 45, 65)).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            items.push(ListItem::new(line).style(item_style));
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
            crate::models::QualityTier::Lossless => Color::Cyan,
            crate::models::QualityTier::Lossy => Color::DarkGray,
        };
        let badge_line = Line::from(Span::styled(
            badge,
            Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
        ));

        let title_line = Line::from(vec![
            Span::styled("♪ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                &track.title,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let artist_album_line = Line::from(vec![
            Span::styled(track.artist_display(), Style::default().fg(Color::Cyan)),
            Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
            Span::styled(track.album_display(), Style::default().fg(Color::Gray)),
        ]);

        // Specs: Format │ Depth/Rate │ Bitrate │ Length │ Driver
        let mut specs = vec![Span::styled(
            format!("Format: {}", track.codec),
            Style::default().fg(Color::Yellow),
        )];
        let depth_rate = track.depth_rate_str();
        if !depth_rate.is_empty() {
            specs.push(Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 60))));
            specs.push(Span::styled(depth_rate, Style::default().fg(Color::Green)));
        }
        specs.push(Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 60))));
        specs.push(Span::styled(
            format!("{}", track.bitrate),
            Style::default().fg(Color::Green),
        ));
        specs.push(Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 60))));
        specs.push(Span::styled(
            track.duration_formatted(),
            Style::default().fg(Color::Cyan),
        ));
        specs.push(Span::styled("  │  ", Style::default().fg(Color::Rgb(50, 50, 60))));
        specs.push(Span::styled(
            &app.audio.output_driver,
            Style::default().fg(Color::Magenta),
        ));
        let specs_line = Line::from(specs);

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

/// [Right]: Synchronized lyrics, full height
fn render_right_lyrics(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.quadrant_focus == QuadrantFocus::Lyrics;
    let border_style = if is_focused {
        Style::default().fg(Color::Cyan)
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
                    Span::styled("▶ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
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
        Span::styled(time_str_left, Style::default().fg(Color::Cyan)),
        Span::styled(filled_bar, Style::default().fg(Color::Cyan)),
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

    let notif = app.get_active_notification().unwrap_or("[o] Open Folder  [S] Scan  [a] Song Menu  [Space] Play/Pause  [n/p] Next/Prev  [←/→] Seek  [↑/↓] Volume  [Tab] Focus  [q] Quit");

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
}
