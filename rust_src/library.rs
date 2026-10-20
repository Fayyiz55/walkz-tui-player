use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use id3::TagLike;
use crate::models::{CategoryType, Track};
use crate::lyrics::LyricsProvider;

pub struct Library {
    pub folder_path: Option<PathBuf>,
    pub tracks: Vec<Track>,
    pub artists: BTreeMap<String, Vec<usize>>,
    pub playlists: BTreeMap<String, Vec<usize>>,
    pub albums: BTreeMap<String, Vec<usize>>,
    pub album_artists: BTreeMap<String, Vec<usize>>,
}

impl Library {
    pub fn new() -> Self {
        Self {
            folder_path: None,
            tracks: Vec::new(),
            artists: BTreeMap::new(),
            playlists: BTreeMap::new(),
            albums: BTreeMap::new(),
            album_artists: BTreeMap::new(),
        }
    }

    /// Scan directory and index into Artist, Playlist, Album, and Album Artist.
    ///
    /// Synchronous wrapper (joins the worker inline) — used by tests and the
    /// CLI fallback; the app's live lifecycle uses [`crate::scanner::ScanJob`].
    #[allow(dead_code)] // exercised from tests/integration_test.rs (external crate)
    pub fn scan_folder(&mut self, folder: &Path) {
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let (audio_paths, m3u_paths) = crate::scanner::collect_files(folder, &cancel);
        let m3us = crate::scanner::read_m3us(m3u_paths);
        let tracks =
            crate::scanner::parse_tracks(&audio_paths, folder, &cancel, &|_, _, _, _| {});
        self.ingest(tracks, m3us, folder);
    }

    /// Rebuild all category indices from the current tracks
    pub fn rebuild_indexes(&mut self) {
        self.artists.clear();
        self.albums.clear();
        self.album_artists.clear();

        // If no custom playlists exist, group by subfolder as natural playlists
        let mut folder_playlists: BTreeMap<String, Vec<usize>> = BTreeMap::new();

        for (idx, track) in self.tracks.iter().enumerate() {
            // Index by Artist
            let artist = track.artist_display().to_string();
            self.artists.entry(artist).or_default().push(idx);

            // Index by Album
            let album = track.album_display().to_string();
            self.albums.entry(album).or_default().push(idx);

            // Index by Album Artist
            let album_artist = track.album_artist_display().to_string();
            self.album_artists.entry(album_artist).or_default().push(idx);

            // Index by Subfolder as Playlist
            if let Some(parent) = track.path.parent() {
                if let Some(name) = parent.file_name().and_then(|s| s.to_str()) {
                    folder_playlists.entry(format!("📁 {}", name)).or_default().push(idx);
                }
            }
        }

        // Sort album tracks by track number
        for indices in self.albums.values_mut() {
            indices.sort_by_key(|&idx| self.tracks[idx].track_number.unwrap_or(9999));
        }

        // Merge folder playlists if no .m3u exists or add "All Tracks"
        if self.playlists.is_empty() {
            let all_indices: Vec<usize> = (0..self.tracks.len()).collect();
            self.playlists.insert("⭐ All Local Tracks".to_string(), all_indices);
            for (name, indices) in folder_playlists {
                self.playlists.insert(name, indices);
            }
        } else {
            let all_indices: Vec<usize> = (0..self.tracks.len()).collect();
            self.playlists.insert("⭐ All Local Tracks".to_string(), all_indices);
        }
    }

    /// Extract metadata and cover image from an audio file
    pub(crate) fn parse_audio_file(path: &Path, base_folder: &Path) -> Option<Track> {
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Track");
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mp3")
            .to_uppercase();

        let mut title = file_stem.to_string();
        let mut artist = "Unknown Artist".to_string();
        let mut album = "Single".to_string();
        let mut album_artist = String::new();
        let mut genre = "Music".to_string();
        let mut year = None;
        let mut track_number = None;
        let mut duration_secs = 180;
        let mut sample_rate = None;
        let mut bit_depth = None;
        let mut channels = None;

        // Try ID3 tag first (kept around: the lyrics loader reuses it instead
        // of paying a second full file read, which matters on network/WSL
        // mounts where every read is a round trip).
        let tag = id3::Tag::read_from_path(path).ok();
        if let Some(tag) = tag.as_ref() {
            if let Some(t) = tag.title() {
                if !t.trim().is_empty() {
                    title = t.trim().to_string();
                }
            }
            if let Some(a) = tag.artist() {
                if !a.trim().is_empty() {
                    artist = a.trim().to_string();
                }
            }
            if let Some(al) = tag.album() {
                if !al.trim().is_empty() {
                    album = al.trim().to_string();
                }
            }
            if let Some(aa) = tag.album_artist() {
                if !aa.trim().is_empty() {
                    album_artist = aa.trim().to_string();
                }
            }
            if let Some(g) = tag.genre() {
                if !g.trim().is_empty() {
                    genre = g.trim().to_string();
                }
            }
            if let Some(y) = tag.year() {
                year = Some(y);
            }
            if let Some(tr) = tag.track() {
                track_number = Some(tr);
            }
        }

        // If artist or album still default, derive from subfolder structure: e.g. Artist/Album/01 Title.mp3
        if let Ok(rel) = path.strip_prefix(base_folder) {
            let components: Vec<&str> = rel
                .iter()
                .filter_map(|c| c.to_str())
                .collect();
            if components.len() >= 3 {
                // e.g. ["Artist", "Album", "Song.mp3"]
                if artist == "Unknown Artist" {
                    artist = components[0].to_string();
                }
                if album == "Single" {
                    album = components[1].to_string();
                }
            } else if components.len() == 2 {
                // e.g. ["Artist or Album", "Song.mp3"]
                if album == "Single" {
                    album = components[0].to_string();
                }
            }
        }

        // If title has artist - title pattern (e.g. "Artist - Song")
        if title.contains(" - ") && artist == "Unknown Artist" {
            let parts: Vec<&str> = title.splitn(2, " - ").collect();
            artist = parts[0].trim().to_string();
            title = parts[1].trim().to_string();
        }

        // Cover art is loaded lazily (see [`Library::load_cover_art`]):
        // keeping every track's artwork bytes resident ballooned RSS to
        // ~1 MB per track (~800 MB for a 1000-track library).

        // Probe media info once: duration, real codec, bit depth, sample rate, bitrate.
        // (Supersedes the old duration-only ffprobe call.)
        let probe_info = crate::media_probe::probe(path);
        if let Some(pi) = &probe_info {
            if let Some(dur) = pi.duration_secs {
                if dur > 0.5 {
                    duration_secs = dur.round() as u64;
                }
            }
            sample_rate = pi.sample_rate;
            bit_depth = pi.bit_depth;
            channels = pi.channels;
        }

        if album_artist.is_empty() {
            album_artist = artist.clone();
        }

        let lyrics = LyricsProvider::load_for_track_with_tag(
            path,
            &title,
            &artist,
            duration_secs,
            tag.as_ref(),
        );

        // Real codec from the stream (ALAC inside M4A etc.), falling back to
        // a friendly extension mapping when ffprobe is unavailable.
        let codec = probe_info
            .as_ref()
            .and_then(|pi| pi.codec.clone())
            .unwrap_or_else(|| match ext.as_str() {
                "ALAC" => "ALAC".to_string(),
                "FLAC" => "FLAC".to_string(),
                "WAV" => "WAV".to_string(),
                "M4A" => "M4A".to_string(),
                other => other.to_string(),
            });

        let bitrate = probe_info
            .as_ref()
            .and_then(crate::media_probe::display_bit_rate_kbps)
            .map(|kbps| format!("{} kbps", kbps))
            .unwrap_or_else(|| "—".to_string());

        Some(Track {
            id: path.to_string_lossy().to_string(),
            path: path.to_path_buf(),
            title,
            artist,
            album,
            album_artist,
            genre,
            year,
            track_number,
            duration_secs,
            codec,
            bitrate,
            sample_rate,
            bit_depth,
            channels,
            cover_art: None,
            lyrics,
        })
    }

    /// Load the cover art for one track on demand: embedded tag picture
    /// first, then a companion image file next to the track (`cover.jpg`,
    /// `folder.png`, …). Called lazily from the UI (only the currently
    /// displayed track needs its bytes in memory).
    pub fn load_cover_art(path: &Path) -> Option<Vec<u8>> {
        if let Ok(tag) = id3::Tag::read_from_path(path) {
            if let Some(pic) = tag.pictures().next() {
                return Some(pic.data.clone());
            }
        }
        path.parent().and_then(Self::find_companion_cover)
    }

    /// Find cover image in folder (cover.jpg, folder.png, etc.)
    fn find_companion_cover(dir: &Path) -> Option<Vec<u8>> {
        let candidates = [
            "cover.jpg", "cover.png", "cover.webp",
            "folder.jpg", "folder.png",
            "album.jpg", "album.png",
            "front.jpg", "front.png", "art.jpg",
        ];

        for name in &candidates {
            let p = dir.join(name);
            if p.exists() {
                if let Ok(bytes) = fs::read(&p) {
                    return Some(bytes);
                }
            }
        }
        None
    }

    /// Provide built-in demo tracks when no folder or files exist yet
    fn load_built_in_tracks(&mut self) {
        let demos = [
            (
                "Midnight City Lights",
                "The Synthwave Project",
                "Neon Horizon",
                "The Synthwave Project",
                "Retrowave",
                2024,
                1,
                142u64,
                "FLAC",
                "Lossless 24-bit",
                Some(48000u32),
                Some(24u8),
            ),
            (
                "Coffee & Rainy Days",
                "Chillhop Collective",
                "Lo-Fi Study Beats Vol. 3",
                "Chillhop Collective",
                "Lo-Fi / Chill",
                2023,
                2,
                128,
                "Opus",
                "256 kbps VBR",
                Some(48000),
                None,
            ),
            (
                "Quantum Funk Odyssey",
                "The Groove Matrix",
                "Electrosoul Dimensions",
                "The Groove Matrix",
                "Nu-Disco / Funk",
                2024,
                3,
                156,
                "AAC",
                "320 kbps",
                Some(44100),
                None,
            ),
            (
                "Acoustic Solitude",
                "Willow & Pine",
                "Forest Echoes",
                "Willow & Pine",
                "Acoustic Folk",
                2022,
                4,
                135,
                "ALAC",
                "Lossless",
                Some(44100),
                Some(16),
            ),
            (
                "Cyberpunk Overdrive",
                "Kavinsky Tribute",
                "Outrun Velocity",
                "Various Artists",
                "Darksynth",
                2025,
                5,
                175,
                "WAV",
                "1411 kbps PCM",
                Some(44100),
                Some(16),
            ),
            (
                "Tokyo Midnight Train",
                "City Pop Express",
                "Pacific Breeze",
                "City Pop Express",
                "City Pop",
                2021,
                6,
                164,
                "MP3",
                "320 kbps CBR",
                Some(44100),
                None,
            ),
        ];

        for (title, artist, album, album_artist, genre, year, track_num, dur, codec, bitrate, demo_sample_rate, demo_bit_depth) in demos {
            let path = PathBuf::from(format!("demo://{}", title.to_lowercase().replace(' ', "_")));
            let lyrics = LyricsProvider::load_for_track(&path, title, artist, dur);

            self.tracks.push(Track {
                id: path.to_string_lossy().to_string(),
                path,
                title: title.to_string(),
                artist: artist.to_string(),
                album: album.to_string(),
                album_artist: album_artist.to_string(),
                genre: genre.to_string(),
                year: Some(year),
                track_number: Some(track_num),
                duration_secs: dur,
                codec: codec.to_string(),
                bitrate: bitrate.to_string(),
                sample_rate: demo_sample_rate,
                bit_depth: demo_bit_depth,
                channels: Some(2),
                cover_art: None,
                lyrics,
            });
        }
    }

    /// Ingest externally-scanned tracks + raw m3u playlist files (reader side).
    ///
    /// Shared tail of every scan path (startup, folder connect, manual
    /// rescan, and the sync `scan_folder` wrapper): assigns tracks, falls
    /// back to built-ins when empty, resolves m3u playlists against the
    /// final track list, and rebuilds category indexes.
    pub fn ingest(&mut self, tracks: Vec<Track>, m3us: Vec<(PathBuf, String)>, folder: &Path) {
        self.folder_path = Some(folder.to_path_buf());
        self.tracks.clear();
        self.artists.clear();
        self.playlists.clear();
        self.albums.clear();
        self.album_artists.clear();

        self.tracks = tracks;

        // If no tracks found in folder, populate aesthetic built-in tracks
        if self.tracks.is_empty() {
            self.load_built_in_tracks();
        }

        // Process any .m3u/.m3u8 playlists against the final track list
        for (pl_path, content) in &m3us {
            let pl_name = pl_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Playlist")
                .to_string();

            let mut track_indices = Vec::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                let candidate = pl_path.parent().unwrap_or(folder).join(line);
                if let Some(idx) = self.tracks.iter().position(|t| t.path == candidate) {
                    track_indices.push(idx);
                }
            }
            if !track_indices.is_empty() {
                self.playlists.insert(pl_name, track_indices);
            }
        }

        // Build index maps
        self.rebuild_indexes();
    }

    /// Get list of category names and track count for a given CategoryType
    pub fn get_category_items(&self, cat: CategoryType) -> Vec<(String, usize)> {        match cat {
            CategoryType::Artist => self
                .artists
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
            CategoryType::Playlist => self
                .playlists
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
            CategoryType::Album => self
                .albums
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
            CategoryType::AlbumArtist => self
                .album_artists
                .iter()
                .map(|(k, v)| (k.clone(), v.len()))
                .collect(),
        }
    }

    /// Get tracks for a given CategoryType and item name
    pub fn get_tracks_for_category(&self, cat: CategoryType, name: &str) -> Vec<usize> {
        match cat {
            CategoryType::Artist => self.artists.get(name).cloned().unwrap_or_default(),
            CategoryType::Playlist => self.playlists.get(name).cloned().unwrap_or_default(),
            CategoryType::Album => self.albums.get(name).cloned().unwrap_or_default(),
            CategoryType::AlbumArtist => self.album_artists.get(name).cloned().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(id: &str, path: PathBuf, artist: &str, album: &str, track_num: u32) -> Track {
        Track {
            id: id.into(),
            path,
            title: format!("Title {id}"),
            artist: artist.into(),
            album: album.into(),
            album_artist: artist.into(),
            genre: "Test".into(),
            year: None,
            track_number: Some(track_num),
            duration_secs: 100,
            codec: "FLAC".into(),
            bitrate: "900 kbps".into(),
            sample_rate: Some(44100),
            bit_depth: Some(16),
            channels: Some(2),
            cover_art: None,
            lyrics: Vec::new(),
        }
    }

    #[test]
    fn ingest_builds_all_indexes_and_resolves_m3u() {
        let root = std::env::temp_dir().join(format!("walkz-ingest-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let album_dir = root.join("My Artist").join("My Album");
        fs::create_dir_all(&album_dir).unwrap();

        let p1 = album_dir.join("01 - Song One.mp3");
        let p2 = album_dir.join("02 - Song Two.mp3");
        let tracks = vec![
            make_track("t1", p1.clone(), "My Artist", "My Album", 2),
            make_track("t2", p2.clone(), "My Artist", "My Album", 1),
        ];
        let m3u_path = root.join("my mix.m3u");
        fs::write(&m3u_path, "My Artist/My Album/01 - Song One.mp3\n").unwrap();
        let m3us = vec![(m3u_path, "My Artist/My Album/01 - Song One.mp3\n".to_string())];

        let mut lib = Library::new();
        lib.ingest(tracks, m3us, &root);

        assert_eq!(lib.tracks.len(), 2);
        assert!(lib.artists.contains_key("My Artist"));
        assert!(lib.albums.contains_key("My Album"));
        // album tracks sorted by track number
        let album_tracks = &lib.albums["My Album"];
        assert_eq!(lib.tracks[album_tracks[0]].track_number, Some(1));
        assert_eq!(lib.tracks[album_tracks[1]].track_number, Some(2));
        // m3u playlist resolved by path
        let mix = lib.playlists.get("my mix").expect("m3u playlist ingested");
        assert_eq!(mix.len(), 1);
        assert_eq!(lib.tracks[mix[0]].id, "t1");
        // all-tracks playlist exists
        assert!(lib.playlists.keys().any(|k| k.contains("All Local")));

        let _ = fs::remove_dir_all(&root);
    }
}
