pub mod header;
pub mod now_playing;
pub mod categories;
pub mod queue;
pub mod metadata_editor;
pub mod audio_output;
pub mod folder_dialog;

use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;
use crate::app::App;
use crate::models::AppTab;

pub mod context_menu;

pub fn render_app(f: &mut Frame, app: &App) {
    let size = f.area();
    if size.width < 20 || size.height < 10 {
        return;
    }

    // Top Header (Height 2)
    // Main Content (Min 8)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(8),
        ])
        .split(size);

    header::render_header(f, app, chunks[0]);

    match app.active_tab {
        AppTab::NowPlaying => now_playing::render_now_playing(f, app, chunks[1]),
        AppTab::Categories => categories::render_categories(f, app, chunks[1]),
        AppTab::Queue => queue::render_queue(f, app, chunks[1]),
        AppTab::MetadataEditor => metadata_editor::render_metadata_editor(f, app, chunks[1]),
        AppTab::AudioOutput => audio_output::render_audio_output(f, app, chunks[1]),
    }

    // Overlays (z-order: scan progress first, dialogs and menus paint over it)
    if app.scan_state.is_some() {
        context_menu::render_scan_overlay(f, app);
    }
    if app.is_folder_dialog_open {
        folder_dialog::render_folder_dialog(f, app);
    }
    if app.menu.is_some() {
        context_menu::render_song_menu(f, app);
    }
    if app.create_playlist.is_some() {
        context_menu::render_create_playlist(f, app);
    }
    if app.credits.is_some() {
        context_menu::render_credits(f, app);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Track;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn make_track() -> Track {
        Track {
            id: "t1".into(),
            path: PathBuf::from("/tmp/x.m4a"),
            title: "Neon Skyline".into(),
            artist: "The Wanderer".into(),
            album: "Night Drive".into(),
            album_artist: "The Wanderer".into(),
            genre: "Synth".into(),
            year: Some(2024),
            track_number: Some(1),
            duration_secs: 200,
            codec: "ALAC".into(),
            bitrate: "1873 kbps".into(),
            sample_rate: Some(96000),
            bit_depth: Some(24),
            channels: Some(2),
            cover_art: None,
            lyrics: Vec::new(),
        }
    }

    fn app_for_overlays() -> (App, PathBuf) {
        let tmp = std::env::temp_dir().join(format!("walkz-ui-overlays-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mut app = App::new_in(tmp.clone(), None);
        let track = make_track();
        app.library.tracks = vec![track.clone()];
        app.active_playlist_tracks = vec![0];
        app.audio.current_track = Some(track);
        app.picker = Some(ratatui_image::picker::Picker::halfblocks());
        (app, tmp)
    }

    fn render_app_to_text(app: &App, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| render_app(f, app)).unwrap();
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
    fn header_shows_walkz_brand() {
        let (app, tmp) = app_for_overlays();
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("Walkz TUI Player"), "brand:\n{text}");
        assert!(text.contains("v0.3"));
        assert!(!text.contains("MYX TUI"));
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn song_menu_renders_header_and_items() {
        let (mut app, tmp) = app_for_overlays();
        app.open_song_menu();
        let text = render_app_to_text(&app, 120, 40);
        // Apple-style header: track + artist + album
        assert!(text.contains("Neon Skyline"), "{text}");
        assert!(text.contains("The Wanderer"), "{text}");
        assert!(text.contains("Night Drive"), "{text}");
        // Items
        assert!(text.contains("Add to a Playlist"), "{text}");
        assert!(text.contains("View Credits"), "{text}");
        assert!(text.contains("Favourite"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn playlist_picker_shows_new_playlist_entry() {
        let (mut app, tmp) = app_for_overlays();
        app.user_data.create_playlist("Road Trip", "").unwrap();
        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 0; // Add to a Playlist…
        app.menu_activate();
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("Road Trip"), "{text}");
        assert!(text.contains("New Playlist"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn playlist_picker_scroll_keeps_selection_visible_with_many_playlists() {
        let (mut app, tmp) = app_for_overlays();
        for name in ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel"] {
            app.user_data.create_playlist(name, "").unwrap();
        }
        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 0; // Add to a Playlist…
        app.menu_activate(); // → picker with 9 entries
        // Jump the highlight to the last entry (＋ New Playlist…) and render
        app.menu.as_mut().unwrap().selected = 8;
        let text = render_app_to_text(&app, 120, 40);
        // Selected entry must be visible even with more items than rows
        assert!(text.contains("New Playlist"), "selection scrolled out:\n{text}");
        assert!(text.contains("Hotel"), "late entries visible:\n{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn create_playlist_form_renders_fields() {
        let (mut app, tmp) = app_for_overlays();
        app.open_create_playlist_standalone();
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("New Playlist"), "{text}");
        assert!(text.contains("Name"), "{text}");
        assert!(text.contains("Description"), "{text}");
        assert!(text.contains("Enter"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn credits_modal_renders_quality_and_specs() {
        let (mut app, tmp) = app_for_overlays();
        app.credits = Some(0);
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("Credits"), "{text}");
        assert!(text.contains("Hi-Res Lossless"), "{text}");
        assert!(text.contains("24-bit/96 kHz"), "{text}");
        assert!(text.contains("ALAC"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn scan_overlay_renders_progress_modal() {
        let (mut app, tmp) = app_for_overlays();
        // Fake scan on an empty tmp dir (worker finishes instantly; receiver
        // holds unread events, which keeps the overlay state alive).
        app.scan_state = Some(crate::app::ScanState::new(tmp.clone()));
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("Scanning"), "{text}");
        assert!(text.contains("Esc"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn queue_shows_real_codec() {
        let (mut app, tmp) = app_for_overlays();
        app.active_tab = AppTab::Queue;
        let text = render_app_to_text(&app, 120, 40);
        assert!(text.contains("Format:"), "{text}");
        assert!(text.contains("ALAC"), "{text}");
        std::fs::remove_dir_all(&tmp).ok();
    }
}
