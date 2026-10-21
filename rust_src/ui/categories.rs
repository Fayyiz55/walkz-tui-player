//! Categories tab: one layout per category type, all driven by song
//! metadata tags.
//!
//! - `1 Album Artist`: three boxes — album artists → their albums → the
//!   selected album's tracks.
//! - `2 Song`: the alphabetically sorted song list, with the highlighted
//!   song's info panel on the right.
//! - `3 Artists`: artists → all of that artist's songs.
//! - `4 Album`: albums → the album's tracks (track-number order).
//! - `5 Playlist`: user-created playlists → the playlist's songs.
//!
//! The digits in the strip are display labels; ←/→ switches the type from
//! anywhere in the tab and Tab cycles through every box.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::app::App;
use crate::models::{CategoryType, Focus};

pub fn render_categories(f: &mut Frame, app: &App, area: Rect) {
    // Full-width sub-tab strip on top (5 labels don't fit in a side column),
    // then the per-type pane layout.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // sub-tabs + hint row
            Constraint::Min(6),    // panes
        ])
        .split(area);

    render_subtab_strip(f, app, rows[0]);

    match app.category_type {
        CategoryType::AlbumArtist => {
            // [Album Artists | Albums | Tracks in album]
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(28),
                    Constraint::Percentage(28),
                    Constraint::Percentage(44),
                ])
                .split(rows[1]);

            render_item_list(
                f,
                app,
                chunks[0],
                " 📂 Album Artists ",
                Focus::CategoryList,
                app.library.get_category_items(CategoryType::AlbumArtist),
                app.selected_category_idx,
            );
            render_item_list(
                f,
                app,
                chunks[1],
                " 💽 Albums ",
                Focus::CategoryMid,
                app.category_mid_items.clone(),
                app.selected_category_mid_idx,
            );
            render_tracks_pane(
                f,
                app,
                chunks[2],
                format!(" 🎶 Tracks in '{}' ", mid_album_name(app)),
            );
        }
        CategoryType::Song => {
            // [Songs | Song info panel]
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(55),
                    Constraint::Percentage(45),
                ])
                .split(rows[1]);

            render_song_list(f, app, chunks[0]);
            render_song_info(f, app, chunks[1]);
        }
        CategoryType::Artist => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(rows[1]);

            render_item_list(
                f,
                app,
                chunks[0],
                " 🎤 Artists ",
                Focus::CategoryList,
                app.library.get_category_items(CategoryType::Artist),
                app.selected_category_idx,
            );
            render_tracks_pane(f, app, chunks[1], format!(" 🎶 Songs by '{}' ", app.active_category_name));
        }
        CategoryType::Album => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(rows[1]);

            render_item_list(
                f,
                app,
                chunks[0],
                " 💽 Albums ",
                Focus::CategoryList,
                app.library.get_category_items(CategoryType::Album),
                app.selected_category_idx,
            );
            render_tracks_pane(f, app, chunks[1], format!(" 🎶 Tracks in '{}' ", app.active_category_name));
        }
        CategoryType::Playlist => {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(40),
                    Constraint::Percentage(60),
                ])
                .split(rows[1]);

            render_item_list(
                f,
                app,
                chunks[0],
                " 📜 Playlists ",
                Focus::CategoryList,
                app.library.get_category_items(CategoryType::Playlist),
                app.selected_category_idx,
            );
            render_tracks_pane(f, app, chunks[1], format!(" 🎶 Songs in '{}' ", app.active_category_name));
        }
    }
}

/// The album selected in the middle box (empty when nothing is drilled).
fn mid_album_name(app: &App) -> &str {
    app.category_mid_items
        .get(app.selected_category_mid_idx)
        .map(|(name, _)| name.as_str())
        .unwrap_or("—")
}

/// Full-width sub-tab row: [1] Album Artist [2] Song [3] Artists [4] Album
/// [5] Playlist. The digits are display labels — the type switches with
/// ←/→ from anywhere in the tab.
fn render_subtab_strip(f: &mut Frame, app: &App, area: Rect) {
    let tabs_focused = app.focus == Focus::CategoryTabs;

    let mut spans = Vec::new();
    if tabs_focused {
        spans.push(Span::styled(
            "▸ ",
            Style::default().fg(app.accent()).add_modifier(Modifier::BOLD),
        ));
    }
    for (i, cat) in CategoryType::all().iter().enumerate() {
        let n = i + 1;
        let is_active = app.category_type == *cat;
        if is_active {
            spans.push(Span::styled(
                format!("[{}] {} ", n, cat.title()),
                Style::default()
                    .fg(Color::Black)
                    .bg(app.accent())
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            let digit_style = if tabs_focused {
                Style::default().fg(app.accent()).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {} ", n), digit_style));
            spans.push(Span::styled(
                format!("{}  ", cat.title()),
                Style::default().fg(if tabs_focused { Color::White } else { Color::DarkGray }),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    // Second row: focus hint while the strip owns focus.
    let hint = if tabs_focused {
        Line::from(Span::styled(
            "  [←/→] Switch type   [Tab] Panes   [c] New playlist",
            Style::default().fg(app.accent()),
        ))
    } else {
        Line::from(" ")
    };
    let hint_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: 1,
    };
    f.render_widget(Paragraph::new(hint), hint_area);
}

/// A bordered list of (name, track-count) items with a footer hint.
fn render_item_list(
    f: &mut Frame,
    app: &App,
    area: Rect,
    title: &str,
    focus: Focus,
    items: Vec<(String, usize)>,
    selected: usize,
) {
    let list_focused = app.focus == focus;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if list_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if list_focused {
            app.accent()
        } else {
            Color::DarkGray
        }))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Items list (with a footer hint row underneath)
    let list_hints = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let visible = list_hints[0].height as usize;
    let list_w = list_hints[0].width as usize;
    let start = crate::ui::now_playing::scroll_start(selected, visible);
    let mut list_items = Vec::new();

    for (idx, (name, count)) in items.iter().enumerate().skip(start).take(visible) {
        let is_sel = idx == selected;
        let prefix = if is_sel { "▶ " } else { "  " };

        // Playlists carry ♪ / ♥ markers; tint them.
        let name_color = if name.starts_with('♥') {
            Color::Rgb(255, 120, 160)
        } else if name.starts_with('♪') {
            Color::Green
        } else if is_sel {
            Color::Yellow
        } else {
            Color::White
        };

        // The selection highlight only shows while the list owns focus.
        let hl = is_sel && list_focused;
        // Keep the "(N tracks)" suffix visible in narrow columns (the
        // Album Artist pane is only ~28% wide): truncate the name to the
        // space left after the prefix and the count instead of padding a
        // fixed 26-char field that pushes the count off the edge.
        let count_text = format!("({} tracks)", count);
        let name_field = (list_w
            .saturating_sub(3 + count_text.chars().count()))
        .max(12);
        let shown = if name.chars().count() > name_field {
            let mut s: String = name.chars().take(name_field.saturating_sub(1)).collect();
            s.push('…');
            s
        } else {
            name.clone()
        };
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(if hl { app.accent() } else { Color::DarkGray })),
            Span::styled(
                format!("{:<width$} ", shown, width = name_field),
                if hl {
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(name_color)
                },
            ),
            Span::styled(count_text, Style::default().fg(Color::DarkGray)),
        ]);

        let style = if hl {
            Style::default().bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        list_items.push(ListItem::new(line).style(style));
    }

    f.render_widget(List::new(list_items), list_hints[0]);

    // Footer hint: what the keys do on the focused area.
    let hint = match focus {
        Focus::CategoryList => {
            if list_focused {
                " [↑/↓] Browse  [Enter] Open  [c] New Playlist"
            } else {
                " "
            }
        }
        Focus::CategoryMid => {
            if list_focused {
                " [↑/↓] Albums  [Enter] Tracks"
            } else {
                " "
            }
        }
        _ => " ",
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))),
        list_hints[1],
    );
}

/// The flat song list (Song tab, left box): every track alphabetically by
/// title, each row one track.
fn render_song_list(f: &mut Frame, app: &App, area: Rect) {
    let list_focused = app.focus == Focus::CategoryList;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if list_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if list_focused {
            app.accent()
        } else {
            Color::DarkGray
        }))
        .title(" 🎶 Songs ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.library.tracks.is_empty() {
        let msg = Paragraph::new("No songs in the library.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let list_hints = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let list_area = list_hints[0];
    let hint_area = list_hints[1];

    let visible = list_area.height as usize;
    let start = crate::ui::now_playing::scroll_start(app.selected_category_idx, visible);
    let order = app.library.song_order();
    let mut list_items = Vec::new();

    for (pos, &track_idx) in order.iter().enumerate().skip(start).take(visible) {
        let Some(track) = app.library.tracks.get(track_idx) else {
            continue;
        };
        let is_sel = pos == app.selected_category_idx;
        let hl = is_sel && list_focused;
        let fav = app.user_data.is_favourite(&track.path.to_string_lossy());
        let marker = if fav { "♥" } else { " " };

        let line = Line::from(vec![
            Span::styled(
                format!("{} ", marker),
                Style::default().fg(Color::Rgb(255, 120, 160)),
            ),
            Span::styled(
                format!("{:<34} ", track.title),
                if hl {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ),
            Span::styled(
                format!("{:<22} ", track.artist_display()),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(track.duration_formatted(), Style::default().fg(Color::DarkGray)),
        ]);

        let style = if hl {
            Style::default().bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        list_items.push(ListItem::new(line).style(style));
    }

    f.render_widget(List::new(list_items), list_area);

    let hint = if list_focused {
        " [↑/↓] Browse  [Enter] Play"
    } else {
        " "
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}

/// The highlighted song's metadata (Song tab, right box).
fn render_song_info(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" ℹ Song Info ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let Some(&track_idx) = app.active_playlist_tracks.first() else {
        let msg = Paragraph::new("No song selected.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    };
    let Some(track) = app.library.tracks.get(track_idx) else {
        return;
    };

    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);
    let accent_style = Style::default().fg(app.accent()).add_modifier(Modifier::BOLD);

    let field = |label: &str, value: String| -> Line<'static> {
        Line::from(vec![
            Span::styled(format!(" {:<13}", label), label_style),
            Span::styled(value, value_style),
        ])
    };

    let depth_rate = track.depth_rate_str();
    let mut format_value = track.codec.clone();
    if !depth_rate.is_empty() {
        format_value.push_str(" · ");
        format_value.push_str(&depth_rate);
    }

    let mut lines = vec![
        Line::from(Span::styled(
            format!(" ♪ {}", track.title),
            accent_style,
        )),
        Line::from(" "),
        field("Artist:", track.artist_display().to_string()),
        field("Album:", track.album_display().to_string()),
        field("Album artist:", track.album_artist_display().to_string()),
        field("Genre:", track.genre.clone()),
        field(
            "Year:",
            track.year.map(|y| y.to_string()).unwrap_or_else(|| "—".into()),
        ),
        field(
            "Track #:",
            track
                .track_number
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".into()),
        ),
        field("Length:", track.duration_formatted()),
        field("Format:", format_value),
        field("Bitrate:", track.bitrate.clone()),
        field(
            "File:",
            track
                .path
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("—")
                .to_string(),
        ),
        Line::from(" "),
        Line::from(Span::styled(
            " [Enter] Play",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    // Trim to the visible height so short panes don't overflow.
    lines.truncate(inner.height as usize);
    f.render_widget(Paragraph::new(lines), inner);
}

/// A bordered list of the tracks in the currently selected category item.
fn render_tracks_pane(f: &mut Frame, app: &App, area: Rect, title: String) {
    let tracks_focused = app.focus == Focus::CategoryTracks;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if tracks_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if tracks_focused {
            app.accent()
        } else {
            Color::DarkGray
        }))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.active_playlist_tracks.is_empty() {
        let msg = Paragraph::new("No tracks found.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let list_hints = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let list_area = list_hints[0];
    let hint_area = list_hints[1];

    let visible = list_area.height as usize;
    let start = crate::ui::now_playing::scroll_start(app.selected_tracklist_idx, visible);
    let mut list_items = Vec::new();
    let current_id = app.audio.current_track.as_ref().map(|t| t.id.as_str());

    for (pos, &idx) in app
        .active_playlist_tracks
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
        if let Some(track) = app.library.tracks.get(idx) {
            let is_now_playing = current_id == Some(track.id.as_str());
            let is_sel = tracks_focused && pos == app.selected_tracklist_idx;
            let icon = if is_now_playing { "▶ " } else { "  " };

            let line = Line::from(vec![
                Span::styled(icon, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:02}. ", pos + 1), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<30} ", track.title),
                    if is_now_playing {
                        Style::default().fg(app.accent()).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                Span::styled(format!("{:<20} ", track.artist_display()), Style::default().fg(Color::Gray)),
                Span::styled(track.duration_formatted(), Style::default().fg(Color::DarkGray)),
            ]);

            let style = if is_sel {
                Style::default().bg(Color::Rgb(30, 45, 65)).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            list_items.push(ListItem::new(line).style(style));
        }
    }

    f.render_widget(List::new(list_items), list_area);

    let hint = if tracks_focused {
        " [↑/↓] Browse  [Enter] Play"
    } else {
        " "
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))),
        hint_area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppTab, Track};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn make_track(title: &str, artist: &str, album: &str) -> Track {
        Track {
            id: format!("t-{title}"),
            path: PathBuf::from("/tmp/x.m4a"),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            album_artist: artist.into(),
            genre: "Synth".into(),
            year: Some(2024),
            track_number: Some(1),
            duration_secs: 200,
            codec: "ALAC".into(),
            bitrate: "1873 kbps".into(),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            channels: Some(2),
            cover_art: None,
            lyrics: Vec::new(),
        }
    }

    fn app_with_library(tracks: Vec<Track>) -> App {
        let tmp = std::env::temp_dir().join(format!("walkz-ui-cat-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mut app = App::new_in(tmp, None);
        app.scan_state = None;
        app.library.tracks = tracks.clone();
        app.library.rebuild_indexes();
        app.active_tab = AppTab::Categories;
        app
    }

    fn render_to_text(app: &App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_categories(f, app, f.area())).unwrap();
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
    fn subtab_labels_show_new_numbering() {
        let app = app_with_library(vec![make_track("A", "X", "Album")]);
        // Wide enough for the five sub-tab labels (full-width strip).
        let text = render_to_text(&app, 150, 40);
        // The active type gets the bracketed chip, the rest show plain digits.
        assert!(text.contains("[1] Album Artist"), "{text}");
        assert!(text.contains("2 Song"), "{text}");
        assert!(text.contains("3 Artists"), "{text}");
        assert!(text.contains("4 Album"), "{text}");
        assert!(text.contains("5 Playlist"), "{text}");

        // Selecting another type moves the bracketed chip with the digit.
        let mut app = app_with_library(vec![make_track("A", "X", "Album")]);
        app.category_type = CategoryType::Song;
        app.refresh_active_category();
        let text = render_to_text(&app, 150, 40);
        assert!(text.contains("[2] Song"), "{text}");
        assert!(text.contains("1 Album Artist"), "{text}");
    }

    #[test]
    fn song_tab_shows_info_panel_on_right() {
        let mut app = app_with_library(vec![
            make_track("First Light", "X", "Album"),
            make_track("Second Wind", "Y", "Album"),
        ]);
        app.category_type = CategoryType::Song;
        app.refresh_active_category();
        let text = render_to_text(&app, 110, 40);
        // Right pane is the highlighted song's info panel.
        assert!(text.contains("Song Info"), "{text}");
        assert!(text.contains("First Light"), "{text}");
        assert!(text.contains("Artist:"), "{text}");
        assert!(text.contains("ALAC"), "{text}");
        assert!(!text.contains("Tracks in '"), "{text}");
    }

    #[test]
    fn song_category_lists_track_titles_on_left() {
        let mut app = app_with_library(vec![
            make_track("First Light", "X", "Album"),
            make_track("Second Wind", "Y", "Album"),
        ]);
        app.category_type = CategoryType::Song;
        app.refresh_active_category();
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("First Light"), "{text}");
        assert!(text.contains("Second Wind"), "{text}");
    }

    #[test]
    fn album_artist_tab_renders_three_boxes() {
        let mut app = app_with_library(vec![
            make_track("Song One", "Aurora", "Night Drive"),
            make_track("Song Two", "Aurora", "Night Drive"),
            make_track("Song Three", "Aurora", "Daybreak"),
        ]);
        app.category_type = CategoryType::AlbumArtist;
        app.refresh_active_category();
        app.focus = Focus::CategoryMid;
        let text = render_to_text(&app, 150, 40);
        // Three boxes: album artists, albums of the selection, its tracks.
        assert!(text.contains("Album Artists"), "{text}");
        assert!(text.contains("Albums"), "{text}");
        // Albums are alphabetical: the first one is drilled by default.
        assert!(text.contains("Tracks in 'Daybreak'"), "{text}");
        assert!(text.contains("Aurora"), "{text}");
        assert!(text.contains("Song Three"), "{text}");

        // Moving the mid selection to Night Drive swaps the tracklist.
        app.selected_category_mid_idx = 1;
        app.refresh_active_category();
        let text = render_to_text(&app, 150, 40);
        assert!(text.contains("Tracks in 'Night Drive'"), "{text}");
        assert!(text.contains("Song One"), "{text}");
    }

    #[test]
    fn tracklist_highlights_selection_when_focused() {
        let mut app = app_with_library(vec![
            make_track("First Light", "X", "Night Drive"),
            make_track("Second Wind", "X", "Night Drive"),
        ]);
        app.category_type = CategoryType::Album;
        app.refresh_active_category();
        app.focus = Focus::CategoryTracks;
        app.selected_tracklist_idx = 1;
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("Tracks in 'Night Drive'"), "{text}");
        // The selected row marker is visible.
        assert!(text.contains("▶"), "{text}");
    }

    #[test]
    fn playlist_tab_title_mentions_playlists() {
        let mut app = app_with_library(vec![make_track("A", "X", "Album")]);
        assert!(app.user_data.create_playlist("Road Trip", "").is_ok());
        assert!(app.user_data.add_track("Road Trip", "/tmp/x.m4a"));
        app.after_library_rebuild();
        app.category_type = CategoryType::Playlist;
        app.refresh_active_category();
        let text = render_to_text(&app, 110, 40);
        assert!(text.contains("Playlists"), "{text}");
        assert!(text.contains("Road Trip"), "{text}");
        assert!(text.contains("Songs in '♪ Road Trip'"), "{text}");
    }
}
