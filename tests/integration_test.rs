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

    // The Playlist category only lists user-created playlists; a fresh scan
    // with none must show an empty list (no folder or .m3u playlists).
    let playlists = lib.get_category_items(CategoryType::Playlist);
    assert!(playlists.is_empty(), "no synthesized playlists are listed: {playlists:?}");
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
    assert!(save_res.is_ok(), "Saving tags should succeed");

    // Re-read with lofty to confirm the tag reached disk in the file's
    // native format.
    use lofty::file::TaggedFileExt;
    use lofty::prelude::*;
    let tagged = lofty::read_from_path(&track.path).expect("Read tag from disk");
    let title = tagged
        .first_tag()
        .and_then(|t| t.title().map(|c| c.to_string()));
    assert_eq!(title.as_deref(), Some("Demo Tone (Edited)"));
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

/// The scan parser must read tags from ALAC .m4a files (the user's main
/// library format — invisible to the old id3-only reader), including the
/// album artist, year, track number and an embedded cover.
#[test]
fn test_m4a_metadata_and_cover_via_lofty() {
    use std::path::PathBuf;
    use std::process::Command;

    let Ok(_) = Command::new("ffmpeg").arg("-version").output() else {
        eprintln!("skipping: ffmpeg not available");
        return;
    };

    let root = std::env::temp_dir().join(format!("walkz-m4a-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scratch = root.join("scratch"); // encoding workspace, never scanned
    let scan_dir = root.join("scan");
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::create_dir_all(&scan_dir).unwrap();

    let raw = scratch.join("raw.m4a");

    // 1. A short ALAC-in-MP4 audio stream with MP4 tags baked in.
    let gen = Command::new("ffmpeg")
        .args([
            "-y", "-v", "error",
            "-f", "lavfi", "-i", "sine=frequency=440:duration=1",
            "-c:a", "alac",
            "-metadata", "title=Neon Skyline",
            "-metadata", "artist=The Wanderer",
            "-metadata", "album=Night Drive",
            "-metadata", "album_artist=The Wanderer",
            "-metadata", "track=3/12",
            "-metadata", "date=2024",
            raw.to_str().unwrap(),
        ])
        .output()
        .expect("run ffmpeg encode");
    assert!(gen.status.success(), "ffmpeg encode failed: {:?}", gen.stderr);

    // 2. Move the finished file into the scanned folder.
    let m4a = scan_dir.join("Neon Skyline.m4a");
    std::fs::rename(&raw, &m4a).unwrap();
    let m4a = PathBuf::from(m4a);

    // 3. Embed a cover the same way the app would write one (tests the
    //    lofty `covr` write/read pair end-to-end).
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
        0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00,
        0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
        0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    {
        use lofty::config::WriteOptions;
        use lofty::file::TaggedFileExt;
        use lofty::picture::{MimeType, Picture, PictureType};
        use lofty::prelude::*;
        let mut tagged = lofty::read_from_path(&m4a).expect("read back the m4a");
        let tag_type = tagged.primary_tag_type();
        if tagged.tag_mut(tag_type).is_none() {
            tagged.insert_tag(lofty::tag::Tag::new(tag_type));
        }
        let tag = tagged.tag_mut(tag_type).unwrap();
        tag.push_picture(
            Picture::unchecked(png.clone())
                .pic_type(PictureType::CoverFront)
                .mime_type(MimeType::Png)
                .build(),
        );
        tagged
            .save_to_path(&m4a, WriteOptions::default())
            .expect("save cover into m4a");
    }

    // 4. Scan and verify the parser sees the real metadata.
    let mut lib = Library::new();
    lib.scan_folder(&scan_dir);

    assert_eq!(lib.tracks.len(), 1, "one m4a track indexed");
    let track = &lib.tracks[0];
    assert_eq!(track.title, "Neon Skyline", "title from ©nam tag");
    assert_eq!(track.artist, "The Wanderer");
    assert_eq!(track.album, "Night Drive");
    assert_eq!(track.album_artist, "The Wanderer");
    assert_eq!(track.track_number, Some(3));
    assert_eq!(track.year, Some(2024), "year from ©day tag");
    assert_eq!(track.codec.to_uppercase(), "ALAC", "codec from the stream");

    // 5. The embedded `covr` picture is found for cover art.
    let art = Library::load_cover_art(&track.path);
    assert!(art.is_some(), "embedded m4a cover art should be found");
    assert_eq!(art.unwrap().len(), png.len(), "cover bytes intact");

    // 6. The track lands in the metadata-driven categories.
    let album_artists = lib.get_category_items(CategoryType::AlbumArtist);
    assert!(album_artists
        .iter()
        .any(|(name, _)| name == "The Wanderer"), "{album_artists:?}");
    let albums_of_aa = lib.albums_of_album_artist("The Wanderer");
    assert_eq!(albums_of_aa, vec![("Night Drive".to_string(), 1)]);

    let _ = std::fs::remove_dir_all(&root);
}

