//! Song context menu, playlist dialogs, credits view, and scan-progress
//! overlays (v0.3).

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::{App, MenuSection};
use crate::models::QualityTier;

/// Centered modal rect, clamped to the screen.
fn modal_rect(screen: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(screen.width.saturating_sub(4));
    let h = height.min(screen.height.saturating_sub(2));
    let x = (screen.width.saturating_sub(w)) / 2;
    let y = (screen.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

fn menu_frame_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title)
}

// ---------------------------------------------------------------------------
// Song context menu (`a`)
// ---------------------------------------------------------------------------

pub fn render_song_menu(f: &mut Frame, app: &App) {
    let Some(menu) = &app.menu else { return };
    let screen = f.area();

    // The modal grows with the picker's entry count (base rows fit ~6
    // items) so typical libraries show everything; the item window below
    // keeps the selection visible beyond the cap.
    let item_count = match menu.section {
        MenuSection::Main => 3usize,
        MenuSection::PlaylistPicker => app.user_data.playlists.len() + 1,
    };
    let wanted_h = 12u16 + (item_count as u16)
        .saturating_sub(6)
        .min(10);
    let area = modal_rect(screen, 62, wanted_h);
    f.render_widget(Clear, area);

    let block = menu_frame_block(" ♫ About This Song ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(track) = app.library.tracks.get(menu.track_idx) else {
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header: mini cover + title/artist/album
            Constraint::Length(1), // gap
            Constraint::Min(4),    // items
            Constraint::Length(1), // footer hints
        ])
        .split(inner);

    // Header: mini vinyl swatch + Title / Artist · Album
    render_menu_header(f, app, track, chunks[0]);

    // Items depend on the section
    let (items, selected) = match menu.section {
        MenuSection::Main => {
            let fav = app.user_data.is_favourite(&track.path.to_string_lossy());
            let fav_label = if fav { "♥ Unfavourite" } else { "☆ Favourite" };
            (
                vec![
                    "⊞  Add to a Playlist…".to_string(),
                    "ⓘ  View Credits".to_string(),
                    fav_label.to_string(),
                ],
                menu.selected,
            )
        }
        MenuSection::PlaylistPicker => {
            let mut items: Vec<String> = app
                .user_data
                .playlists
                .iter()
                .map(|p| {
                    let desc = if p.description.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", p.description)
                    };
                    format!("♪  {}{}", p.name, desc)
                })
                .collect();
            items.push("＋  New Playlist…".to_string());
            (items, menu.selected)
        }
    };

    let list_items: Vec<ListItem> = {
        // Window so the selected entry stays visible (non-stateful List).
        let visible = chunks[2].height.max(1) as usize;
        let offset = if selected + 1 > visible {
            selected + 1 - visible
        } else {
            0
        };
        items
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .map(|(idx, label)| {
                let is_sel = idx == selected;
                let line = Line::from(Span::styled(
                    format!("  {}  ", label),
                    if is_sel {
                        Style::default().fg(Color::White).bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                ));
                ListItem::new(line)
            })
            .collect()
    };
    f.render_widget(List::new(list_items), chunks[2]);

    // Footer hints
    let footer = match menu.section {
        MenuSection::Main => " [↑/↓] Select   [Enter] Choose   [Esc] Close ",
        MenuSection::PlaylistPicker => " [↑/↓] Select   [Enter] Add   [Esc] Back ",
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            footer,
            Style::default().fg(Color::DarkGray),
        )))
        .alignment(Alignment::Center),
        chunks[3],
    );
}

/// Mini cover swatch + song identification header inside the menu.
fn render_menu_header(f: &mut Frame, app: &App, track: &crate::models::Track, area: Rect) {
    if area.width < 20 {
        return;
    }
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(area);

    // Mini cover: 4x2 colored vinyl swatch (title-hash tint)
    let swatch = Rect { x: cols[0].x, y: cols[0].y, width: 4, height: 2 };
    crate::cover::CoverRenderer::render_vinyl_placeholder(f.buffer_mut(), swatch, &track.title);

    let info = Line::from(vec![
        Span::styled(
            &track.title,
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(track.artist_display(), Style::default().fg(Color::Cyan)),
        Span::styled("  ·  ", Style::default().fg(Color::DarkGray)),
        Span::styled(track.album_display(), Style::default().fg(Color::Gray)),
    ]);
    f.render_widget(Paragraph::new(info), cols[1]);
    let _ = app; // reserved for future mini art protocol
}

// ---------------------------------------------------------------------------
// Create playlist form
// ---------------------------------------------------------------------------

pub fn render_create_playlist(f: &mut Frame, app: &App) {
    let Some(state) = &app.create_playlist else { return };
    let screen = f.area();

    let area = modal_rect(screen, 62, 11);
    f.render_widget(Clear, area);

    let block = menu_frame_block(" ＋ New Playlist ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Name field
            Constraint::Length(3), // Description field
            Constraint::Min(1),    // error line
            Constraint::Length(1), // footer hints
        ])
        .split(inner);

    // Name field
    let name_style = if state.focused_field == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let name_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(name_style)
        .title(" Name ");
    let name_inner = name_block.inner(chunks[0]);
    f.render_widget(name_block, chunks[0]);
    let name_disp = if state.name_input.is_empty() && state.focused_field == 0 {
        Line::from(Span::styled("e.g. Road Trip █", Style::default().fg(Color::DarkGray)))
    } else {
        Line::from(vec![
            Span::styled(&state.name_input, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    };
    f.render_widget(Paragraph::new(name_disp), name_inner);

    // Description field
    let desc_style = if state.focused_field == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(desc_style)
        .title(" Description (optional) ");
    let desc_inner = desc_block.inner(chunks[1]);
    f.render_widget(desc_block, chunks[1]);
    let desc_disp = if state.desc_input.is_empty() && state.focused_field == 1 {
        Line::from(Span::styled("what's this playlist for? █", Style::default().fg(Color::DarkGray)))
    } else {
        Line::from(vec![
            Span::styled(&state.desc_input, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ])
    };
    f.render_widget(Paragraph::new(desc_disp), desc_inner);

    // Error line
    if let Some(err) = &state.error {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("⚠️  {}", err),
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ))),
            chunks[2],
        );
    }

    // Footer
    let footer = Line::from(vec![
        Span::styled(" [Enter] Save ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(" [Tab] Next Field ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
        Span::raw("   "),
        Span::styled(" [Esc] Cancel ", Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50))),
    ]);
    f.render_widget(Paragraph::new(footer).alignment(Alignment::Center), chunks[3]);
}

// ---------------------------------------------------------------------------
// Credits view
// ---------------------------------------------------------------------------

pub fn render_credits(f: &mut Frame, app: &App) {
    let Some(track_idx) = app.credits else { return };
    let Some(track) = app.library.tracks.get(track_idx) else { return };
    let screen = f.area();

    let area = modal_rect(screen, 70, 14);
    f.render_widget(Clear, area);

    let block = menu_frame_block(" ⓘ Credits ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let (badge, badge_color) = match track.quality_tier() {
        QualityTier::HiResLossless => (track.quality_tier().badge(), Color::Rgb(255, 215, 0)),
        QualityTier::Lossless => (track.quality_tier().badge(), Color::Cyan),
        QualityTier::Lossy => (track.quality_tier().badge(), Color::DarkGray),
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("♪ ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(
                &track.title,
                Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled(track.artist_display(), Style::default().fg(Color::Cyan)),
            Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
            Span::styled(track.album_display(), Style::default().fg(Color::Gray)),
            if let Some(year) = track.year {
                Span::styled(format!("  ·  {}", year), Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            },
        ]),
        Line::from(""),
        Line::from(Span::styled(
            badge,
            Style::default().fg(badge_color).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("Codec: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&track.codec, Style::default().fg(Color::Yellow)),
            Span::styled("   Rate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if track.depth_rate_str().is_empty() {
                    "—".to_string()
                } else {
                    track.depth_rate_str()
                },
                Style::default().fg(Color::Green),
            ),
            Span::styled("   Bitrate: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&track.bitrate, Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Genre: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&track.genre, Style::default().fg(Color::Gray)),
            Span::styled("   Length: ", Style::default().fg(Color::DarkGray)),
            Span::styled(track.duration_formatted(), Style::default().fg(Color::Cyan)),
            Span::styled("   Output: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.audio.output_driver, Style::default().fg(Color::Magenta)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("File: ", Style::default().fg(Color::DarkGray)),
            Span::styled(track.path.display().to_string(), Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " [Esc] Close ",
            Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50)),
        )),
    ];

    f.render_widget(Paragraph::new(lines), inner);
}

// ---------------------------------------------------------------------------
// Scan progress overlay
// ---------------------------------------------------------------------------

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn render_scan_overlay(f: &mut Frame, app: &App) {
    let Some(scan) = app.scan_state.as_ref() else { return };
    let screen = f.area();

    let area = modal_rect(screen, 64, 9);
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" 🔎 Scanning Library ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // spinner + phase/status
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // counts
            Constraint::Length(1), // current file
            Constraint::Length(1), // footer hint
        ])
        .split(inner);

    let spinner_idx = ((scan.started.elapsed().as_millis() / 80) as usize) % SPINNER_FRAMES.len();
    let spinner = SPINNER_FRAMES[spinner_idx];

    let status_text = if scan.listed {
        format!("Parsing audio files…")
    } else {
        format!("Walking directories…")
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(status_text, Style::default().fg(Color::White)),
        ])),
        chunks[0],
    );

    // Progress bar
    let ratio = scan.ratio();
    let bar_width = inner.width.saturating_sub(2) as usize;
    let filled = ((bar_width as f64) * ratio).round() as usize;
    let line = Line::from(vec![
        Span::styled("█".repeat(filled), Style::default().fg(Color::Yellow)),
        Span::styled("░".repeat(bar_width.saturating_sub(filled)), Style::default().fg(Color::Rgb(50, 50, 60))),
    ]);
    f.render_widget(Paragraph::new(line), chunks[1]);

    // Counts
    let pct = (ratio * 100.0).round() as u64;
    let counts = if scan.listed {
        format!(
            "{}% · {}/{} files · {} tracks found · {}s",
            pct, scan.files_done, scan.files_total, scan.tracks_found, scan.elapsed_secs()
        )
    } else {
        format!(
            "Walking… {} dirs · {} files · {}s",
            scan.walk_dirs, scan.walk_files, scan.elapsed_secs()
        )
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(counts, Style::default().fg(Color::Gray)))),
        chunks[2],
    );

    // Current file
    if !scan.current.is_empty() {
        let max_chars = chunks[3].width.saturating_sub(2) as usize;
        let disp = truncate_path(&scan.current, max_chars);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(disp, Style::default().fg(Color::DarkGray)))),
            chunks[3],
        );
    }

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            " [Esc] Cancel ",
            Style::default().fg(Color::White).bg(Color::Rgb(40, 40, 50)),
        )))
        .alignment(Alignment::Center),
        chunks[4],
    );
}

fn truncate_path(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let tail: String = s
        .chars()
        .skip(s.chars().count() - max_len.saturating_sub(1))
        .collect();
    format!("…{}", tail)
}
