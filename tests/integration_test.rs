use std::path::Path;
use walkz_tui::library::Library;
use walkz_tui::models::CategoryType;
use walkz_tui::tag_editor::TagEditorState;
use walkz_tui::lyrics::LyricsProvider;

#[test]
fn test_category_scanning() {
    let mut lib = Library::new();
    let folder = Path::new("test_music");
    lib.scan_folder(folder);

    // Verify tracks were found
    assert!(!lib.tracks.is_empty(), "Library should find tracks in test_music");

    // Verify artists category
    let artists = lib.get_category_items(CategoryType::Artist);
    assert!(artists.iter().any(|(name, _)| name.contains("Test Artist")), "Should contain Test Artist artist");
    assert!(artists.iter().any(|(name, _)| name.contains("Score Composer")), "Should contain Score Composer artist");

    // Verify albums category
    let albums = lib.get_category_items(CategoryType::Album);
    assert!(albums.iter().any(|(name, _)| name.contains("Test Album")), "Should contain Test Album album");
    assert!(albums.iter().any(|(name, _)| name.contains("Score Album")), "Should contain Score Album album");

    // Verify album artist category
    let album_artists = lib.get_category_items(CategoryType::AlbumArtist);
    assert!(album_artists.iter().any(|(name, _)| name.contains("Test Artist")), "Should contain Test Artist album artist");

    // Verify playlist category (folder playlists + all tracks)
    let playlists = lib.get_category_items(CategoryType::Playlist);
    assert!(playlists.iter().any(|(name, _)| name.contains("All Local Tracks")), "Should contain All Local Tracks playlist");
}

#[test]
fn test_cover_image_detection() {
    let mut lib = Library::new();
    let folder = Path::new("test_music");
    lib.scan_folder(folder);

    // Cover art is no longer kept resident per track; it is loaded lazily
    // from the track's companion image / embedded tag picture.
    let test_album_track = lib.tracks.iter().find(|t| t.album.contains("Test Album"));
    assert!(test_album_track.is_some(), "Should find Test Album track");
    let test_album_track = test_album_track.unwrap();
    assert!(test_album_track.cover_art.is_none(), "scan does not keep art bytes resident");

    let art = Library::load_cover_art(&test_album_track.path);
    assert!(art.is_some(), "Should lazily detect companion cover.jpg");
    assert!(!art.unwrap().is_empty(), "cover bytes should be non-empty");
}

#[test]
fn test_tag_editor_save() {
    let mut lib = Library::new();
    let folder = Path::new("test_music");
    lib.scan_folder(folder);

    let mut editor = TagEditorState::new();
    let track = &lib.tracks[0];
    editor.load_track(0, track);

    // Modify title
    editor.fields[0].value = "Demo Tone (Edited)".to_string();
    let save_res = editor.save_to_disk();
    assert!(save_res.is_ok(), "Saving ID3 tag should succeed");

    // Re-read with ID3 directly to confirm written to disk
    use id3::TagLike;
    let read_tag = id3::Tag::read_from_path(&track.path).expect("Read tag from disk");
    assert_eq!(read_tag.title(), Some("Demo Tone (Edited)"));
}

#[test]
fn test_lyrics_parsing() {
    let sample_lrc = "[00:10.50] Hello world\n[00:25.00] Second line of lyrics\n[01:05.12] Final line";
    let lines = LyricsProvider::parse_lrc(sample_lrc);
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0].text, "Hello world");
    assert!((lines[0].time_secs - 10.5).abs() < 0.001);

    let active_at_20 = LyricsProvider::current_line_index(&lines, 20.0);
    assert_eq!(active_at_20, 0);

    let active_at_30 = LyricsProvider::current_line_index(&lines, 30.0);
    assert_eq!(active_at_30, 1);
}

#[test]
fn test_folder_dialog_submission() {
    use walkz_tui::app::App;

    let mut app = App::new(None);
    app.open_folder_dialog();
    assert!(app.is_folder_dialog_open);

    // Paste path with quotes and whitespace
    app.folder_dialog_input = "  \"./test_music\"  ".to_string();
    let res = app.submit_folder_dialog();
    assert!(res, "Folder dialog submission should succeed for ./test_music");
    assert!(!app.is_folder_dialog_open, "Dialog should close upon success");
    // v0.3: scanning is non-blocking; wait for the worker to apply results
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while app.scan_state.is_some() && std::time::Instant::now() < deadline {
        app.poll_scan();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app.scan_state.is_none(), "scan finished");
    assert!(!app.library.tracks.is_empty(), "Library should be loaded with tracks");
    assert!(app.folder_dialog_error.is_none());

    // Test non-existent folder
    app.open_folder_dialog();
    app.folder_dialog_input = "/non_existent_folder_xyz_123".to_string();
    let fail_res = app.submit_folder_dialog();
    assert!(!fail_res, "Should fail for non-existent directory");
    assert!(app.folder_dialog_error.is_some());
    assert!(app.is_folder_dialog_open, "Dialog should stay open when error occurs");
}

