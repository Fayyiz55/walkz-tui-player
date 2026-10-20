use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, Paragraph};
use ratatui::Frame;
use crate::app::App;
use crate::models::{CategoryType, Focus};

pub fn render_categories(f: &mut Frame, app: &App, area: Rect) {
    // Full-width sub-tab strip on top (5 labels don't fit in the 40% column),
    // then the category list / tracks panes.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // sub-tabs + hint row
            Constraint::Min(6),    // panes
        ])
        .split(area);

    render_subtab_strip(f, app, rows[0]);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Category list
            Constraint::Percentage(60), // Tracks in selected category
        ])
        .split(rows[1]);

    render_category_column(f, app, chunks[0]);
    render_tracks_column(f, app, chunks[1]);
}

/// Full-width sub-tab row: [1] Album Artist [2] Song [3] Artists [4] Album
/// [5] Playlist. The digits respond only while this strip owns focus; when
/// the main tab bar owns focus they dim (1-6 switch main tabs there).
fn render_subtab_strip(f: &mut Frame, app: &App, area: Rect) {
    let tabs_focused = app.focus == Focus::CategoryTabs;
    let digits_live = tabs_focused;

    let mut spans = Vec::new();
    if tabs_focused {
        spans.push(Span::styled(
            "▸ ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
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
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            let digit_style = if digits_live {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!(" {} ", n), digit_style));
            spans.push(Span::styled(
                format!("{}  ", cat.title()),
                Style::default().fg(if digits_live { Color::White } else { Color::DarkGray }),
            ));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);

    // Second row: focus hint while the strip owns focus.
    let hint = if tabs_focused {
        Line::from(Span::styled(
            "  [1-5] Switch type   [←/→] Cycle   [Tab] List",
            Style::default().fg(Color::Cyan),
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

fn render_category_column(f: &mut Frame, app: &App, area: Rect) {
    let list_focused = app.focus == Focus::CategoryList;

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if list_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if list_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }))
        .title(" 📂 Categories ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Items list (with a footer hint row underneath)
    let items = app.library.get_category_items(app.category_type);
    let list_hints = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(4),
            Constraint::Length(1),
        ])
        .split(inner);

    let visible = list_hints[0].height as usize;
    let start = crate::ui::now_playing::scroll_start(app.selected_category_idx, visible);
    let mut list_items = Vec::new();

    for (idx, (name, count)) in items.iter().enumerate().skip(start).take(visible) {
        let is_sel = idx == app.selected_category_idx;
        let prefix = if is_sel { "▶ " } else { "  " };

        // User playlists and favourites carry ♪ / ♥ markers; tint them.
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
        let line = Line::from(vec![
            Span::styled(prefix, Style::default().fg(if hl { Color::Cyan } else { Color::DarkGray })),
            Span::styled(
                format!("{:<26} ", name),
                if hl {
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(name_color)
                },
            ),
            Span::styled(format!("({} tracks)", count), Style::default().fg(Color::DarkGray)),
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
    let hint = if list_focused {
        " [↑/↓] Browse  [Enter] Tracks  [c] New Playlist"
    } else if app.focus == Focus::CategoryTabs {
        " [c] New Playlist"
    } else {
        " "
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            hint,
            Style::default().fg(Color::DarkGray),
        ))),
        list_hints[1],
    );
}

fn render_tracks_column(f: &mut Frame, app: &App, area: Rect) {
    let tracks_focused = app.focus == Focus::CategoryTracks;
    let is_song = app.category_type == CategoryType::Song;

    // The Song category keeps everything on the left; the right pane stays
    // a blank box (per design).
    let title = if is_song {
        " 🎶 Song ".to_string()
    } else {
        format!(" 🎶 Tracks in '{}' ", app.active_category_name)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if tracks_focused {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(Style::default().fg(if tracks_focused {
            Color::Cyan
        } else {
            Color::DarkGray
        }))
        .title(title);

    let inner = block.inner(area);
    f.render_widget(block, area);

    if is_song {
        let msg = if tracks_focused {
            Paragraph::new(format!(
                "▶ Enter plays '{}'",
                app.active_category_name
            ))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
        } else {
            Paragraph::new("(song list is on the left)")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center)
        };
        f.render_widget(msg, inner);
        return;
    }

    if app.active_playlist_tracks.is_empty() {
        let msg = Paragraph::new("No tracks found.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(msg, inner);
        return;
    }

    let visible = inner.height as usize;
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
                        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
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

    f.render_widget(List::new(list_items), inner);
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
        let text = render_to_text(&app, 150, 40);
        assert!(text.contains("[2] Song"), "{text}");
        assert!(text.contains("1 Album Artist"), "{text}");
    }

    #[test]
    fn song_category_blanks_right_pane() {
        let mut app = app_with_library(vec![
            make_track("First Light", "X", "Album"),
            make_track("Second Wind", "Y", "Album"),
        ]);
        app.category_type = CategoryType::Song;
        app.refresh_active_category();
        let text = render_to_text(&app, 110, 40);
        // Right pane is a blank box with a hint, not a track list.
        assert!(text.contains("Song"), "{text}");
        assert!(!text.contains("Tracks in '"), "{text}");
        assert!(text.contains("song list is on the left"), "{text}");
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
}
