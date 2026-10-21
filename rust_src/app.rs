use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use crate::audio::AudioPlayer;
use crate::library::Library;
use crate::models::{AppTab, CategoryType, Focus};
use crate::playlist::UserData;
use crate::scanner::{ScanJob, ScanPayload};
use crate::tag_editor::TagEditorState;

/// Sections inside the song context menu overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSection {
    /// Add to a Playlist… / View Credits / Favourite
    Main,
    /// Pick a user playlist (plus "New Playlist…" entry)
    PlaylistPicker,
}

/// The song context menu overlay state (opened with `a`).
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub track_idx: usize,
    pub section: MenuSection,
    pub selected: usize,
}

/// The create-playlist form state (Name + Description).
#[derive(Debug, Clone)]
pub struct CreatePlaylistState {
    pub name_input: String,
    pub desc_input: String,
    pub focused_field: usize, // 0 = name, 1 = description
    /// Track index to add to the playlist right after creation
    /// (menu-triggered creation), None for standalone creation.
    pub pending_track: Option<usize>,
    pub error: Option<String>,
}

/// Live background scan view state (progress overlay + lifecycle).
pub struct ScanState {
    pub job: ScanJob,
    pub folder: PathBuf,
    pub started: Instant,
    pub listed: bool,
    /// Directory-walk heartbeats (before `listed`).
    pub walk_dirs: usize,
    pub walk_files: usize,
    pub files_done: usize,
    pub files_total: usize,
    pub tracks_found: usize,
    pub current: String,
}

impl ScanState {
    pub fn new(folder: PathBuf) -> Self {
        Self {
            job: ScanJob::start(folder.clone()),
            folder,
            started: Instant::now(),
            listed: false,
            walk_dirs: 0,
            walk_files: 0,
            files_done: 0,
            files_total: 0,
            tracks_found: 0,
            current: String::new(),
        }
    }

    /// Wrap an already-started job in a fresh, zeroed view state (tests).
    #[allow(dead_code)]
    pub fn new_empty(job: ScanJob) -> Self {
        Self {
            job,
            folder: PathBuf::new(),
            started: Instant::now(),
            listed: false,
            walk_dirs: 0,
            walk_files: 0,
            files_done: 0,
            files_total: 0,
            tracks_found: 0,
            current: String::new(),
        }
    }

    /// Elapsed seconds since the scan started.
    pub fn elapsed_secs(&self) -> u64 {
        self.started.elapsed().as_secs()
    }

    /// Overall progress ratio (0.0..1.0).
    pub fn ratio(&self) -> f64 {
        if !self.listed || self.files_total == 0 {
            return 0.0;
        }
        (self.files_done as f64 / self.files_total as f64).clamp(0.0, 1.0)
    }
}

// v2: tags are parsed with lofty (m4a/ALAC metadata was invisible to the
// old id3-only reader), so cached libraries must be re-scanned once.
const LIBRARY_CACHE_VERSION: u32 = 2;

/// Persisted snapshot of the last finished scan, so the next launch can
/// restore the library instantly instead of re-walking the folder. Cover
/// art and lyrics are `#[serde(skip)]` on `Track` (loaded on demand), so
/// this stays small even for large libraries.
#[derive(serde::Serialize, serde::Deserialize)]
struct LibraryCache {
    version: u32,
    folder: PathBuf,
    /// Unix epoch seconds of the scan that produced this cache.
    scanned_at: u64,
    tracks: Vec<crate::models::Track>,
    /// Raw .m3u/.m3u8 playlists, re-resolved against `tracks` on load.
    m3us: Vec<(PathBuf, String)>,
}

#[allow(dead_code)]
pub struct App {
    pub library: Library,
    pub audio: AudioPlayer,
    pub active_tab: AppTab,
    pub focus: Focus,
    pub category_type: CategoryType,
    pub selected_category_idx: usize,
    /// Drill-down list for the Album Artist tab: albums of the selected
    /// album artist as (album name, track count).
    pub category_mid_items: Vec<(String, usize)>,
    pub selected_category_mid_idx: usize,
    pub selected_tracklist_idx: usize,
    pub active_category_name: String,
    pub active_playlist_tracks: Vec<usize>, // Track indices currently shown in the tracklist
    pub tag_editor: TagEditorState,
    pub notification: Option<(String, Instant)>,
    pub should_quit: bool,
    pub is_folder_dialog_open: bool,
    pub folder_dialog_input: String,
    pub folder_dialog_error: Option<String>,
    // v0.3
    pub config_dir: PathBuf,
    pub user_data: UserData,
    pub scan_state: Option<ScanState>,
    pub menu: Option<ContextMenuState>,
    pub create_playlist: Option<CreatePlaylistState>,
    pub credits: Option<usize>,
    /// Terminal graphics-protocol picker (hi-res album art). Built once at
    /// startup; None until assigned (tests use `Picker::halfblocks()`).
    pub picker: Option<ratatui_image::picker::Picker>,
    /// Prepared album-art protocol, cached per (track id, art-area dims).
    pub cover_cache: std::cell::RefCell<Option<crate::cover::CoverCache>>,
    /// Lazily loaded cover-art bytes for the currently displayed track:
    /// `None` until first needed, then keyed by (path, mtime) so edits to
    /// the tags invalidate it. Keeps resident memory at one artwork instead
    /// of one artwork per library track.
    art_cache: std::cell::RefCell<Option<ArtCacheEntry>>,
    /// Persisted user settings (Settings tab).
    pub settings: crate::settings::Settings,
    /// Selected row within the Settings list.
    pub settings_selected: usize,
    /// When set, the next plain keystroke rebinds this hotkey (Settings tab).
    pub rebinding: Option<crate::settings::HotkeyAction>,
    /// Prepared ambient backdrop (blurred album art / tint ramp), cached per
    /// (track, area dims, dimming, style).
    pub ambient_cache: std::cell::RefCell<Option<crate::ambient::AmbientCache>>,
    /// cava spectrum worker (None = not running).
    pub viz: Option<crate::viz::VizWorker>,
    /// Latest decoded spectrum frame (values 0.0..1.0).
    pub viz_frame: Option<Vec<f32>>,
    /// Input-method attempt ladder (0 pipewire, 1 pulse, ≥2 unavailable).
    viz_attempt: usize,
    /// `cava` missing from PATH / unusable (settled until the visualizer is
    /// toggled again in Settings).
    pub viz_unavailable: bool,
}

/// One lazily loaded artwork payload.
struct ArtCacheEntry {
    key: (PathBuf, Option<std::time::SystemTime>),
    art: Option<Vec<u8>>,
}

impl App {
    /// Production constructor: default config dir, async initial scan.
    pub fn new(initial_folder: Option<PathBuf>) -> Self {
        Self::new_in(Self::default_config_dir(), initial_folder)
    }

    /// Injectable constructor (used by tests): explicit config dir; starts
    /// an async initial scan when `initial_folder` is Some.
    pub fn new_in(config_dir: PathBuf, initial_folder: Option<PathBuf>) -> Self {
        let library = Library::new();
        let target_dir = initial_folder
            .or_else(Self::load_last_folder)
            .unwrap_or_else(Self::fallback_music_dir);

        // User settings decide whether the library cache is used at all.
        let settings = crate::settings::Settings::load_from(&config_dir);

        // Launch from the persisted library cache when the target folder is
        // unchanged and the user allows it: the library is ready immediately,
        // no rescan on startup. A changed/missing folder (or missing/corrupt
        // cache, or cache loading disabled in Settings) falls back to the
        // non-blocking startup scan.
        let cached = if settings.load_library_cache_on_launch {
            Self::load_library_cache(&config_dir, &target_dir)
        } else {
            None
        };
        let scan_state = if cached.is_some() {
            None
        } else {
            Some(ScanState::new(target_dir.clone()))
        };

        let audio = AudioPlayer::new();
        let user_data = Self::load_user_data(&config_dir);
        let rescan_key = settings.hotkey_char(crate::settings::HotkeyAction::Rescan);

        let mut app = Self {
            library,
            audio,
            active_tab: AppTab::NowPlaying,
            focus: Focus::default_for_tab(AppTab::NowPlaying),
            category_type: CategoryType::AlbumArtist,
            selected_category_idx: 0,
            category_mid_items: Vec::new(),
            selected_category_mid_idx: 0,
            selected_tracklist_idx: 0,
            active_category_name: String::from("Loading…"),
            active_playlist_tracks: Vec::new(),
            tag_editor: TagEditorState::new(),
            notification: None,
            should_quit: false,
            is_folder_dialog_open: false,
            folder_dialog_input: String::new(),
            folder_dialog_error: None,
            config_dir,
            user_data,
            scan_state,
            menu: None,
            create_playlist: None,
            credits: None,
            picker: None,
            cover_cache: std::cell::RefCell::new(None),
            art_cache: std::cell::RefCell::new(None),
            settings,
            settings_selected: 0,
            rebinding: None,
            ambient_cache: std::cell::RefCell::new(None),
            viz: None,
            viz_frame: None,
            viz_attempt: 0,
            viz_unavailable: false,
        };

        // Apply user defaults: launch volume and remembered playback state.
        app.audio.volume = app.settings.default_volume.min(100) as u32;
        if app.settings.remember_repeat_shuffle {
            if let Some(repeat) = app.settings.saved_repeat {
                app.audio.repeat_mode = repeat;
            }
            if let Some(shuffle) = app.settings.saved_shuffle {
                app.audio.shuffle = shuffle;
            }
        }

        if let Some((payload, track_count)) = cached {
            app.apply_scan_results_at(target_dir, payload);
            app.set_notification(format!(
                "📚 Library ready: {} tracks (from last scan) — [{}] rescans",
                track_count, rescan_key
            ));
        }

        app
    }

    fn default_config_dir() -> PathBuf {
        match std::env::var("HOME") {
            Ok(home) => PathBuf::from(home).join(".config").join("walkz-tui"),
            Err(_) => PathBuf::from(".config").join("walkz-tui"),
        }
    }

    pub fn fallback_music_dir() -> PathBuf {
        // Default to ~/Music if it exists, otherwise current working directory
        if let Ok(home) = std::env::var("HOME") {
            let music_dir = PathBuf::from(home).join("Music");
            if music_dir.exists() {
                return music_dir;
            }
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    fn load_user_data(config_dir: &Path) -> UserData {
        // Legacy myx-tui configs carry only the remembered folder, which is
        // migrated by the last-folder lookup in `new_in`; playlists and
        // favourites are new in v0.3, so there is nothing else to carry.
        UserData::load(config_dir)
    }

    pub fn set_notification(&mut self, msg: impl Into<String>) {
        self.notification = Some((msg.into(), Instant::now()));
    }

    /// The user's accent color (Settings tab) used for highlights, focus
    /// borders and the active tab chip.
    pub fn accent(&self) -> ratatui::style::Color {
        self.settings.accent.color()
    }

    /// Cover-art bytes for one track, loaded on demand and cached until the
    /// track's file changes. Prefers bytes embedded on the track itself
    /// (tests / already-loaded payloads), falls back to reading the file's
    /// tag or companion image.
    pub fn cover_art_bytes(&self, track: &crate::models::Track) -> Option<Vec<u8>> {
        if let Some(bytes) = &track.cover_art {
            return Some(bytes.clone());
        }

        let mtime = std::fs::metadata(&track.path).ok().and_then(|m| m.modified().ok());
        let key = (track.path.clone(), mtime);

        if let Some(entry) = self.art_cache.borrow().as_ref() {
            if entry.key == key {
                return entry.art.clone();
            }
        }

        let art = crate::library::Library::load_cover_art(&track.path);
        *self.art_cache.borrow_mut() = Some(ArtCacheEntry { key, art: art.clone() });
        art
    }

    pub fn get_active_notification(&self) -> Option<&str> {
        if let Some((msg, created)) = &self.notification {
            if created.elapsed() < Duration::from_secs(3) {
                return Some(msg.as_str());
            }
        }
        None
    }

    // ------------------------------------------------------------------
    // Library / category navigation
    // ------------------------------------------------------------------

    /// Switch active category type (Album Artist, Song, Artists, Album,
    /// Playlist) and reset the drill-down selection.
    pub fn set_category_type(&mut self, cat: CategoryType) {
        self.category_type = cat;
        self.selected_category_idx = 0;
        self.selected_category_mid_idx = 0;
        self.refresh_active_category();
    }

    pub fn next_category_type(&mut self) {
        self.set_category_type(self.category_type.next());
    }

    pub fn prev_category_type(&mut self) {
        self.set_category_type(self.category_type.prev());
    }

    /// Jump to a main tab via its number key (1-6) and reset focus to that
    /// tab's default area.
    pub fn switch_main_tab(&mut self, c: char) {
        let tab = match c {
            '1' => AppTab::NowPlaying,
            '2' => AppTab::Categories,
            '3' => AppTab::Queue,
            '4' => AppTab::MetadataEditor,
            '5' => AppTab::AudioOutput,
            _ => AppTab::Settings,
        };
        self.active_tab = tab;
        self.focus = Focus::default_for_tab(tab);
    }

    /// Execute a global action through its (possibly rebound) hotkey.
    pub fn perform_hotkey_action(&mut self, action: crate::settings::HotkeyAction) {
        use crate::settings::HotkeyAction as H;
        match action {
            H::NextTrack => self.play_next(),
            H::PrevTrack => self.play_prev(),
            H::PlayPause => self.audio.toggle_pause(),
            H::VolumeUp => self.change_volume_setting(self.settings.volume_step as i32),
            H::VolumeDown => self.change_volume_setting(-(self.settings.volume_step as i32)),
            H::ToggleShuffle => {
                self.audio.shuffle = !self.audio.shuffle;
                self.set_notification(format!(
                    "🔀 Shuffle: {}",
                    if self.audio.shuffle { "On" } else { "Off" }
                ));
                self.remember_playback_state();
            }
            H::CycleRepeat => {
                self.audio.repeat_mode = self.audio.repeat_mode.next();
                self.set_notification(format!("🔁 Repeat: {}", self.audio.repeat_mode.label()));
                self.remember_playback_state();
            }
            H::SongMenu => self.open_song_menu(),
            H::OpenFolder => self.open_folder_dialog(),
            H::Rescan => self.manual_rescan(),
            H::ToggleAmbient => {
                self.settings.ambient_enabled = !self.settings.ambient_enabled;
                let state = if self.settings.ambient_enabled { "On" } else { "Off" };
                self.save_settings();
                self.invalidate_ambient();
                self.set_notification(format!("🖼 Ambient background: {}", state));
            }
        }
    }

    fn change_volume_setting(&mut self, delta: i32) {
        self.audio.change_volume(delta);
        self.set_notification(format!("🔊 Volume: {}%", self.audio.volume));
    }

    /// Snapshot repeat/shuffle into the settings file while remembering is
    /// enabled in Settings.
    fn remember_playback_state(&mut self) {
        if self.settings.remember_repeat_shuffle {
            self.settings.saved_repeat = Some(self.audio.repeat_mode);
            self.settings.saved_shuffle = Some(self.audio.shuffle);
            self.save_settings();
        }
    }

    /// Update tracks in the tracklist according to current category selection.
    ///
    /// Layouts per category type:
    /// - Album Artist: three boxes — album artists → their albums → the
    ///   selected album's tracks (track-number order).
    /// - Song: one flat, alphabetically sorted song list; the right side is
    ///   the highlighted song's info panel.
    /// - Artists / Album / Playlist: item list → its tracks.
    pub fn refresh_active_category(&mut self) {
        if self.category_type == CategoryType::Song {
            // The highlighted row IS one track; the info panel reads it.
            let order = self.library.song_order();
            if order.is_empty() {
                self.active_category_name = "Empty Category".to_string();
                self.active_playlist_tracks.clear();
                self.category_mid_items.clear();
                self.selected_tracklist_idx = 0;
                return;
            }
            if self.selected_category_idx >= order.len() {
                self.selected_category_idx = order.len() - 1;
            }
            let track_idx = order[self.selected_category_idx];
            self.active_category_name = self.library.tracks[track_idx].title.clone();
            self.active_playlist_tracks = vec![track_idx];
            self.selected_tracklist_idx = 0;
            self.category_mid_items.clear();
            return;
        }

        let items = self.library.get_category_items(self.category_type);
        if items.is_empty() {
            self.active_category_name = "Empty Category".to_string();
            self.active_playlist_tracks.clear();
            self.category_mid_items.clear();
            self.selected_tracklist_idx = 0;
            return;
        }

        if self.selected_category_idx >= items.len() {
            self.selected_category_idx = items.len().saturating_sub(1);
        }

        let (name, _) = &items[self.selected_category_idx];
        self.active_category_name = name.clone();

        if self.category_type == CategoryType::AlbumArtist {
            // Drill level 2: the albums of this album artist.
            let albums = self.library.albums_of_album_artist(name);
            if albums.is_empty() {
                self.category_mid_items.clear();
                self.active_playlist_tracks.clear();
                self.selected_tracklist_idx = 0;
                return;
            }
            if self.selected_category_mid_idx >= albums.len() {
                self.selected_category_mid_idx = albums.len() - 1;
            }
            let album_name = albums[self.selected_category_mid_idx].0.clone();
            self.category_mid_items = albums;
            self.active_playlist_tracks = self
                .library
                .tracks_of_album_under_album_artist(name, &album_name);
            self.selected_tracklist_idx = 0;
            return;
        }

        self.category_mid_items.clear();
        self.active_playlist_tracks = self.library.get_tracks_for_category(self.category_type, name);
        self.selected_tracklist_idx = 0;
    }

    /// Navigate items in the Categories tab list
    pub fn select_next_category_item(&mut self) {
        let items = self.library.get_category_items(self.category_type);
        if !items.is_empty() && self.selected_category_idx + 1 < items.len() {
            self.selected_category_idx += 1;
            self.selected_category_mid_idx = 0;
            self.refresh_active_category();
        }
    }

    pub fn select_prev_category_item(&mut self) {
        if self.selected_category_idx > 0 {
            self.selected_category_idx -= 1;
            self.selected_category_mid_idx = 0;
            self.refresh_active_category();
        }
    }

    /// Navigate the drill-down (middle) list: albums of an album artist.
    pub fn select_next_category_mid(&mut self) {
        if !self.category_mid_items.is_empty()
            && self.selected_category_mid_idx + 1 < self.category_mid_items.len()
        {
            self.selected_category_mid_idx += 1;
            self.refresh_active_category();
        }
    }

    pub fn select_prev_category_mid(&mut self) {
        if self.selected_category_mid_idx > 0 {
            self.selected_category_mid_idx -= 1;
            self.refresh_active_category();
        }
    }

    /// Navigate tracks in tracklist
    pub fn select_next_track(&mut self) {
        if !self.active_playlist_tracks.is_empty()
            && self.selected_tracklist_idx + 1 < self.active_playlist_tracks.len()
        {
            self.selected_tracklist_idx += 1;
        }
    }

    pub fn select_prev_track(&mut self) {
        if self.selected_tracklist_idx > 0 {
            self.selected_tracklist_idx -= 1;
        }
    }

    /// Idempotently apply user playlists + favourites into the library map.
    /// Entries use distinctive prefixes (`♪ `, `♥ `) so they survive
    /// re-application after any `rebuild_indexes`.
    pub fn apply_user_playlists(&mut self) {
        self.library.playlists.retain(|k, _| !(k.starts_with("♪ ") || k.starts_with("♥ ")));

        for pl in &self.user_data.playlists {
            let indices: Vec<usize> = pl
                .track_paths
                .iter()
                .filter_map(|p| {
                    self.library
                        .tracks
                        .iter()
                        .position(|t| t.path == PathBuf::from(p))
                })
                .collect();
            self.library.playlists.insert(format!("♪ {}", pl.name), indices);
        }

        if !self.user_data.favourites.is_empty() {
            let indices: Vec<usize> = self
                .user_data
                .favourites
                .iter()
                .filter_map(|p| {
                    self.library
                        .tracks
                        .iter()
                        .position(|t| t.path == PathBuf::from(p))
                })
                .collect();
            self.library.playlists.insert(String::from("♥ Favourites"), indices);
        }
    }

    /// Called after anything rebuilds library indexes (metadata save, scan
    /// ingest): re-applies user playlists and refreshes the view.
    pub fn after_library_rebuild(&mut self) {
        self.apply_user_playlists();
        self.refresh_active_category();
    }

    // ------------------------------------------------------------------
    // Playback
    // ------------------------------------------------------------------

    /// Play the currently highlighted track in the tracklist
    pub fn play_selected_track(&mut self) {
        if let Some(&track_idx) = self.active_playlist_tracks.get(self.selected_tracklist_idx) {
            if let Some(track) = self.library.tracks.get(track_idx) {
                let track_clone = track.clone();
                let title = track_clone.title.clone();
                self.audio.play(track_clone, 0.0);
                self.set_notification(format!("▶ Now Playing: {}", title));
            }
        }
    }

    /// Next track in current playlist
    pub fn play_next(&mut self) {
        if self.active_playlist_tracks.is_empty() {
            return;
        }

        let current_pos = self
            .audio
            .current_track
            .as_ref()
            .and_then(|ct| self.active_playlist_tracks.iter().position(|&idx| {
                self.library.tracks.get(idx).map(|t| &t.id == &ct.id).unwrap_or(false)
            }))
            .unwrap_or(self.selected_tracklist_idx);

        let next_pos = if self.audio.shuffle {
            use std::collections::hash_map::RandomState;
            use std::hash::{BuildHasher, Hasher};
            let h = RandomState::new().build_hasher().finish() as usize;
            h % self.active_playlist_tracks.len()
        } else {
            (current_pos + 1) % self.active_playlist_tracks.len()
        };

        self.selected_tracklist_idx = next_pos;
        self.play_selected_track();
    }

    /// Previous track in current playlist
    pub fn play_prev(&mut self) {
        if self.active_playlist_tracks.is_empty() {
            return;
        }

        // If played more than 3 seconds, restart current track
        if self.audio.position_secs > 3.0 {
            self.audio.seek_relative(-self.audio.position_secs);
            return;
        }

        let current_pos = self
            .audio
            .current_track
            .as_ref()
            .and_then(|ct| self.active_playlist_tracks.iter().position(|&idx| {
                self.library.tracks.get(idx).map(|t| &t.id == &ct.id).unwrap_or(false)
            }))
            .unwrap_or(self.selected_tracklist_idx);

        let prev_pos = if current_pos > 0 {
            current_pos - 1
        } else {
            self.active_playlist_tracks.len().saturating_sub(1)
        };

        self.selected_tracklist_idx = prev_pos;
        self.play_selected_track();
    }

    /// Open current track in Metadata Editor
    pub fn open_metadata_editor_for_current(&mut self) {
        if let Some(&track_idx) = self.active_playlist_tracks.get(self.selected_tracklist_idx) {
            if let Some(track) = self.library.tracks.get(track_idx) {
                self.tag_editor.load_track(track_idx, track);
                self.active_tab = AppTab::MetadataEditor;
            }
        } else if let Some(track) = &self.audio.current_track {
            if let Some(idx) = self.library.tracks.iter().position(|t| t.id == track.id) {
                self.tag_editor.load_track(idx, track);
                self.active_tab = AppTab::MetadataEditor;
            }
        }
    }

    // ------------------------------------------------------------------
    // Song context menu (`a`)
    // ------------------------------------------------------------------

    /// Open the song context menu for the selected track (tracklist/queue).
    pub fn open_song_menu(&mut self) {
        if let Some(&track_idx) = self.active_playlist_tracks.get(self.selected_tracklist_idx) {
            self.menu = Some(ContextMenuState {
                track_idx,
                section: crate::app::MenuSection::Main,
                selected: 0,
            });
        }
    }

    pub fn menu_move(&mut self, delta: isize) {
        let Some(menu) = &mut self.menu else { return };
        let count = match menu.section {
            crate::app::MenuSection::Main => 3,
            crate::app::MenuSection::PlaylistPicker => self.user_data.playlists.len() + 1,
        } as isize;
        if count == 0 {
            return;
        }
        let next = (menu.selected as isize + delta).rem_euclid(count);
        menu.selected = next as usize;
    }

    pub fn menu_back(&mut self) {
        let Some(menu) = &mut self.menu else { return };
        match menu.section {
            crate::app::MenuSection::Main => self.menu = None,
            crate::app::MenuSection::PlaylistPicker => {
                menu.section = crate::app::MenuSection::Main;
                menu.selected = 0;
            }
        }
    }

    /// Activate the highlighted menu entry.
    pub fn menu_activate(&mut self) {
        let Some(menu) = self.menu.take() else { return };

        match menu.section {
            crate::app::MenuSection::Main => match menu.selected {
                // Add to a Playlist…
                0 => {
                    self.menu = Some(ContextMenuState {
                        track_idx: menu.track_idx,
                        section: crate::app::MenuSection::PlaylistPicker,
                        selected: 0,
                    });
                }
                // View Credits
                1 => {
                    self.credits = Some(menu.track_idx);
                }
                // Favourite / Unfavourite
                2 => {
                    let fav_track = self.library.tracks.get(menu.track_idx).map(|t| {
                        (t.path.to_string_lossy().to_string(), t.title.clone())
                    });
                    if let Some((path, title)) = fav_track {
                        let fav = self.user_data.toggle_favourite(&path);
                        self.after_library_rebuild();
                        match self.user_data.save(&self.config_dir) {
                            Ok(()) => {
                                self.set_notification(if fav {
                                    format!("♥ Favourited: {}", title)
                                } else {
                                    format!("Removed from favourites: {}", title)
                                });
                            }
                            Err(e) => {
                                self.set_notification(format!("⚠️ Saved in session, but couldn't persist: {}", e));
                            }
                        }
                    }
                }
                _ => {}
            },
            crate::app::MenuSection::PlaylistPicker => {
                let playlist_names: Vec<String> = self
                    .user_data
                    .playlists
                    .iter()
                    .map(|p| p.name.clone())
                    .collect();

                if menu.selected == playlist_names.len() {
                    // "＋ New Playlist…" entry
                    self.create_playlist = Some(crate::app::CreatePlaylistState {
                        name_input: String::new(),
                        desc_input: String::new(),
                        focused_field: 0,
                        pending_track: Some(menu.track_idx),
                        error: None,
                    });
                } else if let Some(name) = playlist_names.get(menu.selected) {
                    let name = name.clone();
                    let added_track = self.library.tracks.get(menu.track_idx).map(|t| {
                        (t.path.to_string_lossy().to_string(), t.title.clone())
                    });
                    if let Some((path, title)) = added_track {
                        let added = self.user_data.add_track(&name, &path);
                        self.after_library_rebuild();
                        match self.user_data.save(&self.config_dir) {
                            Ok(()) => {
                                self.set_notification(if added {
                                    format!("♪ Added '{}' to {}", title, name)
                                } else {
                                    format!("'{}' already in {}", title, name)
                                });
                            }
                            Err(e) => {
                                self.set_notification(format!("⚠️ Added in session, but couldn't persist: {}", e));
                            }
                        }
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Create playlist form (`c` in Categories tab, or via song menu)
    // ------------------------------------------------------------------

    pub fn open_create_playlist_standalone(&mut self) {
        self.create_playlist = Some(crate::app::CreatePlaylistState {
            name_input: String::new(),
            desc_input: String::new(),
            focused_field: 0,
            pending_track: None,
            error: None,
        });
    }

    pub fn create_playlist_next_field(&mut self) {
        let Some(cp) = &mut self.create_playlist else { return };
        cp.focused_field = (cp.focused_field + 1) % 2;
    }

    pub fn create_playlist_prev_field(&mut self) {
        let Some(cp) = &mut self.create_playlist else { return };
        cp.focused_field = (cp.focused_field + 1) % 2; // 2 fields: same cycle
    }

    pub fn create_playlist_handle_char(&mut self, c: char) {
        let Some(cp) = &mut self.create_playlist else { return };
        match cp.focused_field {
            0 => cp.name_input.push(c),
            _ => cp.desc_input.push(c),
        }
    }

    pub fn create_playlist_handle_backspace(&mut self) {
        let Some(cp) = &mut self.create_playlist else { return };
        match cp.focused_field {
            0 => {
                cp.name_input.pop();
            }
            _ => {
                cp.desc_input.pop();
            }
        }
    }

    pub fn create_playlist_clear_field(&mut self) {
        let Some(cp) = &mut self.create_playlist else { return };
        match cp.focused_field {
            0 => cp.name_input.clear(),
            _ => cp.desc_input.clear(),
        }
    }

    pub fn close_create_playlist(&mut self) {
        self.create_playlist = None;
    }

    /// Submit the create-playlist form. On success (optionally adding the
    /// pending track) the form closes; on failure it stays open with the
    /// error, inputs preserved.
    pub fn submit_create_playlist(&mut self) {
        let Some(state) = self.create_playlist.take() else { return };

        match self.user_data.create_playlist(&state.name_input, &state.desc_input) {
            Ok(()) => {
                let name = state.name_input.trim().to_string();
                if let Some(track_idx) = state.pending_track {
                    if let Some(track) = self.library.tracks.get(track_idx) {
                        let path = track.path.to_string_lossy().to_string();
                        self.user_data.add_track(&name, &path);
                    }
                }
                self.after_library_rebuild();
                match self.user_data.save(&self.config_dir) {
                    Ok(()) => self.set_notification(format!("♪ Created playlist '{}'", name)),
                    Err(e) => self.set_notification(format!("⚠️ Created in session, but couldn't persist: {}", e)),
                }
            }
            Err(e) => {
                self.create_playlist = Some(crate::app::CreatePlaylistState {
                    name_input: state.name_input,
                    desc_input: state.desc_input,
                    focused_field: state.focused_field,
                    pending_track: state.pending_track,
                    error: Some(e),
                });
            }
        }
    }

    // ------------------------------------------------------------------
    // Scanning lifecycle
    // ------------------------------------------------------------------

    /// Manually re-scan the current library folder (button = `Shift+S`).
    pub fn manual_rescan(&mut self) {
        if let Some(existing) = &self.scan_state {
            existing.job.cancel();
        }
        let folder = self
            .library
            .folder_path
            .clone()
            .unwrap_or_else(Self::fallback_music_dir);
        self.scan_state = Some(ScanState::new(folder));
        self.set_notification("🔎 Scanning library…");
    }

    /// Non-blocking poll of the scan worker; swaps in results when done.
    pub fn poll_scan(&mut self) {
        let Some(state) = self.scan_state.as_ref() else { return };

        let mut listed = None;
        let mut walking: Option<(usize, usize)> = None;
        let mut progress: Option<(usize, usize, usize, String)> = None;
        let mut finished: Option<ScanPayload> = None;
        let mut disconnected = false;

        loop {
            match state.job.receiver.try_recv() {
                Ok(ev) => match ev {
                    crate::scanner::ScanEvent::Walking { dirs, files } => {
                        walking = Some((dirs, files));
                    }
                    crate::scanner::ScanEvent::Listed(total) => listed = Some(total),
                    crate::scanner::ScanEvent::Progress {
                        files_done,
                        files_total,
                        tracks_found,
                        current,
                    } => progress = Some((files_done, files_total, tracks_found, current)),
                    crate::scanner::ScanEvent::Finished(payload) => finished = Some(*payload),
                },
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // The worker thread exited without delivering `Finished`
                    // (panic, cancelled mid-walk, …). Without this the overlay
                    // would stay up forever showing a stale phase.
                    disconnected = true;
                    break;
                }
            }
        }

        if let (Some(st), Some(total)) = (self.scan_state.as_mut(), listed) {
            st.files_total = total;
            st.listed = true;
        }
        if let (Some(st), Some((dirs, files))) = (self.scan_state.as_mut(), walking) {
            st.walk_dirs = dirs;
            st.walk_files = files;
        }
        if let (Some(st), Some((done, total, found, current))) =
            (self.scan_state.as_mut(), progress)
        {
            st.files_done = done;
            st.files_total = total;
            st.tracks_found = found;
            st.current = current;
            st.listed = true;
        }

        if let Some(payload) = finished {
            let folder = self
                .scan_state
                .take()
                .map(|s| s.folder)
                .unwrap_or_else(|| self.library.folder_path.clone().unwrap_or_else(PathBuf::new));
            self.save_library_cache(&folder, &payload.tracks, &payload.m3us);
            self.apply_scan_results_at(folder, payload);
        } else if disconnected {
            self.scan_state = None;
            self.set_notification("⚠️ Scan aborted");
        }
    }

    fn apply_scan_results_at(&mut self, folder: PathBuf, payload: ScanPayload) {
        let track_count = payload.tracks.len();
        self.library.ingest(payload.tracks, payload.m3us, &folder);
        self.after_library_rebuild();

        // Preload first track metadata when nothing is playing (startup /
        // fresh connect). Rescans keep active playback when the track
        // still exists in the library.
        let keep_current = self
            .audio
            .current_track
            .as_ref()
            .map(|ct| self.library.tracks.iter().any(|t| t.id == ct.id))
            .unwrap_or(false);
        if !keep_current {
            if let Some(&first_idx) = self.active_playlist_tracks.first() {
                if let Some(track) = self.library.tracks.get(first_idx) {
                    let track_clone = track.clone();
                    self.audio.current_track = Some(track_clone.clone());
                    self.audio.duration_secs = track_clone.duration_secs;
                    self.audio.position_secs = 0.0;
                    self.tag_editor.load_track(first_idx, &track_clone);
                }
            }
        }

        let folder_name = folder.file_name().and_then(|n| n.to_str()).unwrap_or("folder");
        self.set_notification(format!(
            "📁 Connected to {}: {} tracks indexed across 4 categories",
            folder_name, track_count
        ));
    }

    // ------------------------------------------------------------------
    // Folder dialog
    // ------------------------------------------------------------------

    /// Open folder input dialog
    pub fn open_folder_dialog(&mut self) {
        self.is_folder_dialog_open = true;
        self.folder_dialog_error = None;
        // Pre-fill with current folder if available
        if let Some(p) = &self.library.folder_path {
            self.folder_dialog_input = p.display().to_string();
        } else {
            self.folder_dialog_input.clear();
        }
    }

    /// What the dialog pre-fills when opened: the connected library folder,
    /// or an empty string when none is set. Used to detect "the input is
    /// still the untouched prefill" so the first typed/pasted path replaces
    /// it instead of appending to it.
    fn folder_dialog_prefill(&self) -> String {
        self.library
            .folder_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    }

    /// Enter one typed character into the folder dialog. Typing over the
    /// untouched prefill replaces it; once the input differs (or was
    /// cleared with Ctrl+U) typing appends as usual.
    fn folder_dialog_type_char(&mut self, c: char) {
        let prefill = self.folder_dialog_prefill();
        if !prefill.is_empty() && self.folder_dialog_input == prefill {
            self.folder_dialog_input.clear();
        }
        self.folder_dialog_input.push(c);
    }

    /// Close folder input dialog
    pub fn close_folder_dialog(&mut self) {
        self.is_folder_dialog_open = false;
        self.folder_dialog_error = None;
    }

    /// Submit folder dialog input and start an async scan of the folder.
    pub fn submit_folder_dialog(&mut self) -> bool {
        let raw = self.folder_dialog_input.trim();
        if raw.is_empty() {
            self.folder_dialog_error = Some("Please enter or paste a folder path".to_string());
            return false;
        }

        // Strip quotes if pasted with quotes
        let unquoted = raw.trim_matches('"').trim_matches('\'').trim();

        // Expand tilde if path starts with ~
        let resolved = if unquoted.starts_with('~') {
            if let Ok(home) = std::env::var("HOME") {
                let rest = unquoted.trim_start_matches('~').trim_start_matches('/');
                PathBuf::from(home).join(rest)
            } else {
                PathBuf::from(unquoted)
            }
        } else {
            let p = PathBuf::from(unquoted);
            if p.is_relative() {
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(p)
            } else {
                p
            }
        };

        if !resolved.exists() {
            self.folder_dialog_error = Some(format!("Directory does not exist: {}", resolved.display()));
            return false;
        }

        if !resolved.is_dir() {
            self.folder_dialog_error = Some(format!("Path is not a directory: {}", resolved.display()));
            return false;
        }

        // v0.3: scan is non-blocking — overlay shows progress; results land
        // via apply_scan_results when the worker finishes.
        if let Some(existing) = &self.scan_state {
            existing.job.cancel();
        }
        self.scan_state = Some(ScanState::new(resolved.clone()));
        Self::save_last_folder(&resolved);
        self.is_folder_dialog_open = false;
        self.folder_dialog_error = None;

        let folder_name = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("folder");
        self.set_notification(format!("🔎 Scanning {}…", folder_name));
        true
    }

    // ------------------------------------------------------------------
    // Config persistence
    // ------------------------------------------------------------------

    fn legacy_config_dir() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".config").join("myx-tui"))
    }

    /// Remembered folder: walkz-tui config wins, legacy myx-tui is read
    /// as a one-time migration fallback.
    fn load_last_folder() -> Option<PathBuf> {
        load_last_folder_from(&Self::default_config_dir(), &Self::legacy_config_dir()?)
    }

    fn save_last_folder(folder: &Path) {
        let path = Self::default_config_dir().join("config.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::json!({
            "last_folder": folder.to_string_lossy(),
        });
        let _ = std::fs::write(path, json.to_string());
    }

    // ------------------------------------------------------------------
    // Library launch cache
    // ------------------------------------------------------------------

    /// Persist the results of a finished scan so the next launch can skip
    /// it (best-effort: an atomic write to `<config_dir>/library_cache.json`,
    /// silently ignored on failure). Empty scans are not cached — a folder
    /// that yielded nothing should be walked again next time.
    fn save_library_cache(
        &self,
        folder: &Path,
        tracks: &[crate::models::Track],
        m3us: &[(PathBuf, String)],
    ) {
        if tracks.is_empty() {
            return;
        }
        let cache = LibraryCache {
            version: LIBRARY_CACHE_VERSION,
            folder: folder.to_path_buf(),
            scanned_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            tracks: tracks.to_vec(),
            m3us: m3us.to_vec(),
        };
        let path = self.config_dir.join("library_cache.json");
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension("tmp");
        if let Ok(json) = serde_json::to_string(&cache) {
            if std::fs::write(&tmp, json).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// Load the launch cache for `target`: returns the payload to apply and
    /// the track count when the cache is valid for that folder. A changed,
    /// missing, or unresolvable folder is never a hit (music lives there).
    fn load_library_cache(config_dir: &Path, target: &Path) -> Option<(ScanPayload, usize)> {
        let raw = std::fs::read_to_string(config_dir.join("library_cache.json")).ok()?;
        let cache: LibraryCache = serde_json::from_str(&raw).ok()?;
        if cache.version != LIBRARY_CACHE_VERSION || cache.tracks.is_empty() {
            return None;
        }
        if !Self::same_directory(&cache.folder, target) {
            return None;
        }
        let track_count = cache.tracks.len();
        Some((
            ScanPayload {
                tracks: cache.tracks,
                m3us: cache.m3us,
            },
            track_count,
        ))
    }

    /// True when both paths resolve to the same existing directory.
    fn same_directory(a: &Path, b: &Path) -> bool {
        match (a.canonicalize(), b.canonicalize()) {
            (Ok(ca), Ok(cb)) => ca == cb,
            _ => false,
        }
    }

    // ------------------------------------------------------------------
    // Input routing
    // ------------------------------------------------------------------

    /// Route pasted text (bracketed paste) into the focused input.
    pub fn paste_text(&mut self, text: &str) {
        let clean = text.replace(['\r', '\n'], "");
        if self.is_folder_dialog_open {
            // Pasting a full path over the untouched prefill replaces it
            // (same intent as typing a fresh path); after Ctrl+U or edits
            // the paste appends.
            let prefill = self.folder_dialog_prefill();
            if !prefill.is_empty() && self.folder_dialog_input == prefill {
                self.folder_dialog_input.clear();
            }
            self.folder_dialog_input.push_str(&clean);
        } else if self.create_playlist.is_some() {
            for c in clean.chars() {
                self.create_playlist_handle_char(c);
            }
        } else if self.active_tab == AppTab::MetadataEditor {
            for c in clean.chars() {
                self.tag_editor.handle_char(c);
            }
        }
    }

    /// Central key routing (moved from main.rs in v0.3 for testability).
    pub fn handle_key(&mut self, code: crossterm::event::KeyCode, modifiers: crossterm::event::KeyModifiers) {
        use crossterm::event::KeyCode;

        // 0. Hotkey rebind capture (Settings tab): the next plain key becomes
        // the binding. Esc (or any non-plain key) cancels the capture.
        if let Some(action) = self.rebinding {
            self.rebinding = None;
            match (code, modifiers) {
                (KeyCode::Esc, _) => {
                    self.set_notification("⌨ Rebind cancelled".to_string());
                }
                (KeyCode::Char(c), m) if m.is_empty() => {
                    self.settings.set_hotkey(action, c);
                    self.save_settings();
                    self.set_notification(format!("⌨ {} → [{}]", action.label(), c));
                }
                _ => {
                    self.set_notification("⌨ Rebind cancelled".to_string());
                }
            }
            return;
        }

        // 1. Folder dialog captures all keys
        if self.is_folder_dialog_open {
            match code {
                KeyCode::Enter => {
                    self.submit_folder_dialog();
                    return;
                }
                KeyCode::Esc => {
                    self.close_folder_dialog();
                    return;
                }
                KeyCode::Backspace => {
                    self.folder_dialog_input.pop();
                    return;
                }
                KeyCode::Char('u') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.folder_dialog_input.clear();
                    return;
                }
                KeyCode::Char(c) => {
                    self.folder_dialog_type_char(c);
                    return;
                }
                _ => {}
            }
            return;
        }

        // 2. Create-playlist form captures all keys
        if self.create_playlist.is_some() {
            match code {
                KeyCode::Enter => {
                    self.submit_create_playlist();
                    return;
                }
                KeyCode::Esc => {
                    self.close_create_playlist();
                    return;
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.create_playlist_next_field();
                    return;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.create_playlist_prev_field();
                    return;
                }
                KeyCode::Backspace => {
                    self.create_playlist_handle_backspace();
                    return;
                }
                KeyCode::Char('s') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.submit_create_playlist();
                    return;
                }
                KeyCode::Char('u') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.create_playlist_clear_field();
                    return;
                }
                KeyCode::Char(c) => {
                    self.create_playlist_handle_char(c);
                    return;
                }
                _ => {}
            }
            return;
        }

        // 3. Song context menu captures navigation keys
        if self.menu.is_some() {
            match code {
                KeyCode::Esc => {
                    self.menu_back();
                    return;
                }
                KeyCode::Enter => {
                    self.menu_activate();
                    return;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.menu_move(-1);
                    return;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.menu_move(1);
                    return;
                }
                _ => {}
            }
            return;
        }

        // 4. Credits view closes
        if self.credits.is_some() {
            match code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.credits = None;
                    return;
                }
                _ => {}
            }
            return;
        }

        // 4.5 Esc during a background scan cancels it (overlay advertises it)
        if self.scan_state.is_some() && code == KeyCode::Esc {
            if let Some(scan) = &self.scan_state {
                scan.job.cancel();
            }
            self.scan_state = None;
            self.set_notification("✕ Scan cancelled");
            return;
        }

        // 5. Metadata editor typing
        if self.active_tab == AppTab::MetadataEditor {
            match code {
                KeyCode::Char('s') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    match self.tag_editor.save_to_disk() {
                        Ok(_) => {}
                        Err(e) => self.set_notification(format!("⚠️ Save failed: {}", e)),
                    }
                    self.after_library_rebuild();
                    return;
                }
                KeyCode::Enter => {
                    match self.tag_editor.save_to_disk() {
                        Ok(_) => {}
                        Err(e) => self.set_notification(format!("⚠️ Save failed: {}", e)),
                    }
                    self.after_library_rebuild();
                    return;
                }
                KeyCode::Tab | KeyCode::Down => {
                    self.tag_editor.next_field();
                    return;
                }
                KeyCode::BackTab | KeyCode::Up => {
                    self.tag_editor.prev_field();
                    return;
                }
                KeyCode::Backspace => {
                    self.tag_editor.handle_backspace();
                    return;
                }
                KeyCode::Char('u') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                    self.tag_editor.clear_current_field();
                    return;
                }
                KeyCode::Esc => {
                    self.active_tab = AppTab::NowPlaying;
                    return;
                }
                KeyCode::Char(c @ ('1' | '2' | '3' | '5' | '6')) if modifiers.is_empty() => {
                    // Keep the editor's tab-jump shortcuts; note '4' still
                    // types into fields (the main tab bar covers 4 elsewhere).
                    self.switch_main_tab(c);
                    return;
                }
                KeyCode::Char(c) => {
                    self.tag_editor.handle_char(c);
                    return;
                }
                _ => {}
            }
            return;
        }

        // 6. Global hotkeys. Rebindable actions go through the settings
        // hotkey map; fixed keys (quit, seek, Tab focus, tab digits) follow.
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
                return;
            }
            KeyCode::Char('c') if modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            KeyCode::Tab => {
                self.focus = self.focus.next_in_tab(self.active_tab);
                while !self.focus_reachable(self.focus) {
                    self.focus = self.focus.next_in_tab(self.active_tab);
                }
                return;
            }
            KeyCode::BackTab => {
                self.focus = self.focus.prev_in_tab(self.active_tab);
                while !self.focus_reachable(self.focus) {
                    self.focus = self.focus.prev_in_tab(self.active_tab);
                }
                return;
            }
            KeyCode::Char(c @ ('1' | '2' | '3' | '4' | '5' | '6'))
                if (modifiers - crossterm::event::KeyModifiers::SHIFT).is_empty() =>
            {
                // Digits always switch main tabs (1 Now Playing … 6
                // Settings); the Categories sub-tab digits are display
                // labels only and are switched with ←/→ instead.
                self.switch_main_tab(c);
                return;
            }
            // Seek is global, except where a pane owns the arrows: the
            // Settings list adjusts its rows, and the Categories tab uses
            // ←/→ to switch category types.
            KeyCode::Left if !self.settings_owns_arrows() && self.active_tab != AppTab::Categories => {
                self.audio
                    .seek_relative(-(self.settings.seek_step_secs as f64));
                return;
            }
            KeyCode::Right if !self.settings_owns_arrows() && self.active_tab != AppTab::Categories => {
                self.audio
                    .seek_relative(self.settings.seek_step_secs as f64);
                return;
            }
            // Rebindable actions (exact char match, case sensitive; SHIFT is
            // part of the char itself, e.g. `S`).
            KeyCode::Char(c)
                if (modifiers - crossterm::event::KeyModifiers::SHIFT).is_empty() =>
            match self.settings.action_for_char(c) {
                Some(action) => {
                    self.perform_hotkey_action(action);
                    return;
                }
                // Permanent aliases: '=' / '_' are the unshifted forms of the
                // '+' / '-' volume keys.
                None if c == '=' => {
                    self.change_volume_setting(self.settings.volume_step as i32);
                    return;
                }
                None if c == '_' => {
                    self.change_volume_setting(-(self.settings.volume_step as i32));
                    return;
                }
                // Caps-Lock tolerance: an uppercase letter that no action is
                // bound to falls back to its lowercase action, so `O` with
                // Caps Lock on still opens the folder dialog. crossterm
                // reports SHIFT for every uppercase char (Caps Lock or not),
                // so the shift bit cannot distinguish them here — the
                // fallback is safe because it only runs when the exact char
                // has no binding (`S` = Rescan resolves in the Some arm
                // above and is never shadowed).
                None => {
                    if c.is_ascii_uppercase() {
                        if let Some(action) = self.settings.action_for_char(c.to_ascii_lowercase())
                        {
                            self.perform_hotkey_action(action);
                            return;
                        }
                    }
                }
            },
            _ => {}
        }

        // 7. Tab-specific navigation (focus-aware: keys land in the focused
        // pane; a dim/unfocused pane ignores navigation keys).
        match self.active_tab {
            AppTab::NowPlaying => match self.focus {
                Focus::NowTracklist => match code {
                    KeyCode::Up | KeyCode::Char('k') => self.select_prev_track(),
                    KeyCode::Down | KeyCode::Char('j') => self.select_next_track(),
                    KeyCode::Enter => self.play_selected_track(),
                    KeyCode::Char('e') => self.open_metadata_editor_for_current(),
                    _ => {}
                },
                _ => {}
            },
            AppTab::Categories => match code {
                KeyCode::Up | KeyCode::Char('k') => match self.focus {
                    Focus::CategoryList => self.select_prev_category_item(),
                    Focus::CategoryMid => self.select_prev_category_mid(),
                    Focus::CategoryTracks => self.select_prev_track(),
                    _ => {}
                },
                KeyCode::Down | KeyCode::Char('j') => match self.focus {
                    Focus::CategoryList => self.select_next_category_item(),
                    Focus::CategoryMid => self.select_next_category_mid(),
                    Focus::CategoryTracks => self.select_next_track(),
                    _ => {}
                },
                // ←/→ switch the category type from anywhere in the tab —
                // not just while the strip owns focus (they no longer seek).
                KeyCode::Left | KeyCode::Char('[') | KeyCode::Char('h') => {
                    self.prev_category_type()
                }
                KeyCode::Right | KeyCode::Char(']') | KeyCode::Char('l') => {
                    self.next_category_type()
                }
                KeyCode::Char('c')
                    if modifiers.is_empty()
                        && matches!(
                            self.focus,
                            Focus::CategoryList | Focus::CategoryMid | Focus::CategoryTabs
                        ) =>
                {
                    self.open_create_playlist_standalone()
                }
                KeyCode::Esc => {
                    // Walk focus back one level; from the tab bar, leave the
                    // tab (historical Esc behavior).
                    match self.focus {
                        Focus::CategoryTracks => {
                            self.focus = if self.category_type == CategoryType::AlbumArtist {
                                Focus::CategoryMid
                            } else {
                                Focus::CategoryList
                            };
                        }
                        Focus::CategoryMid => self.focus = Focus::CategoryList,
                        Focus::CategoryList => self.focus = Focus::CategoryTabs,
                        Focus::CategoryTabs => self.focus = Focus::TabBar,
                        _ => {
                            self.active_tab = AppTab::NowPlaying;
                            self.focus = Focus::default_for_tab(AppTab::NowPlaying);
                        }
                    }
                }
                KeyCode::Enter => match self.focus {
                    // The Song tab has no track pane: Enter plays the
                    // highlighted song in place (the info panel follows the
                    // highlight).
                    Focus::CategoryList if self.category_type == CategoryType::Song => {
                        self.play_selected_track();
                    }
                    // Enter on the list opens the next level.
                    Focus::CategoryList => {
                        self.selected_tracklist_idx = 0;
                        self.focus = if self.category_type == CategoryType::AlbumArtist {
                            Focus::CategoryMid
                        } else {
                            Focus::CategoryTracks
                        };
                    }
                    Focus::CategoryMid => {
                        self.selected_tracklist_idx = 0;
                        self.focus = Focus::CategoryTracks;
                    }
                    // Enter on the tracklist plays and goes to Now Playing.
                    Focus::CategoryTracks => {
                        self.active_tab = AppTab::NowPlaying;
                        self.focus = Focus::default_for_tab(AppTab::NowPlaying);
                        self.play_selected_track();
                    }
                    _ => {}
                },
                _ => {}
            },
            AppTab::Queue => {
                if self.focus == Focus::QueueList {
                    match code {
                        KeyCode::Up | KeyCode::Char('k') => self.select_prev_track(),
                        KeyCode::Down | KeyCode::Char('j') => self.select_next_track(),
                        KeyCode::Enter => {
                            self.active_tab = AppTab::NowPlaying;
                            self.focus = Focus::default_for_tab(AppTab::NowPlaying);
                            self.play_selected_track();
                        }
                        _ => {}
                    }
                }
            }
            AppTab::Settings => {
                if self.focus == Focus::SettingsList {
                    self.handle_settings_key(code);
                } else if code == KeyCode::Esc {
                    self.active_tab = AppTab::NowPlaying;
                    self.focus = Focus::default_for_tab(AppTab::NowPlaying);
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Settings tab input
    // ------------------------------------------------------------------

    /// True while the Settings list owns Left/Right (row adjustment).
    fn settings_owns_arrows(&self) -> bool {
        self.active_tab == AppTab::Settings && self.focus == Focus::SettingsList
    }

    /// Whether a focus target is actually rendered/reachable in the current
    /// state. Used to skip dead stops while cycling focus with Tab:
    /// - the lyrics pane is hidden while disabled in Settings;
    /// - the Categories middle list exists only in the Album Artist tab;
    /// - the Categories track list does not exist in the Song tab (its
    ///   right side is the song info panel).
    fn focus_reachable(&self, focus: Focus) -> bool {
        if self.active_tab == AppTab::NowPlaying
            && focus == Focus::NowLyrics
            && !self.settings.show_lyrics_pane
        {
            return false;
        }
        if self.active_tab != AppTab::Categories {
            return true;
        }
        match focus {
            Focus::CategoryMid => self.category_type == CategoryType::AlbumArtist,
            Focus::CategoryTracks => self.category_type != CategoryType::Song,
            _ => true,
        }
    }

    fn handle_settings_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        let rows = crate::settings::settings_rows();
        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_selected = self.settings_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.settings_selected + 1 < rows.len() {
                    self.settings_selected += 1;
                }
            }
            KeyCode::Enter => self.settings_activate(&rows[self.settings_selected.min(rows.len() - 1)]),
            KeyCode::Left => self.settings_adjust(&rows[self.settings_selected.min(rows.len() - 1)], -1),
            KeyCode::Right => self.settings_adjust(&rows[self.settings_selected.min(rows.len() - 1)], 1),
            KeyCode::Esc => {
                self.active_tab = AppTab::NowPlaying;
                self.focus = Focus::default_for_tab(AppTab::NowPlaying);
            }
            _ => {}
        }
    }

    /// Enter on a settings row: toggle booleans, cycle art size, start a
    /// hotkey rebind.
    fn settings_activate(&mut self, row: &crate::settings::SettingsRow) {
        use crate::settings::SettingsRow as Row;
        match row {
            // Edit the music folder: the same modal used by the `o` hotkey,
            // pre-filled with the current path. Submitting rescans, saves the
            // folder to config.json and refreshes the launch cache.
            Row::MusicFolder => {
                self.open_folder_dialog();
            }
            Row::StartupCache => {
                self.settings.load_library_cache_on_launch =
                    !self.settings.load_library_cache_on_launch;
                let state = if self.settings.load_library_cache_on_launch {
                    "on"
                } else {
                    "off"
                };
                self.save_settings();
                self.set_notification(format!(
                    "💾 Load cache on launch: {} (applies next launch)",
                    state
                ));
            }
            Row::RememberState => {
                self.settings.remember_repeat_shuffle = !self.settings.remember_repeat_shuffle;
                let state = if self.settings.remember_repeat_shuffle {
                    "on"
                } else {
                    "off"
                };
                // While on, snapshot the current state right away.
                if self.settings.remember_repeat_shuffle {
                    self.settings.saved_repeat = Some(self.audio.repeat_mode);
                    self.settings.saved_shuffle = Some(self.audio.shuffle);
                }
                self.save_settings();
                self.set_notification(format!("💾 Remember repeat/shuffle: {}", state));
            }
            Row::ShowLyrics => {
                self.settings.show_lyrics_pane = !self.settings.show_lyrics_pane;
                let state = if self.settings.show_lyrics_pane { "shown" } else { "hidden" };
                self.save_settings();
                self.set_notification(format!("🎤 Lyrics pane: {}", state));
            }
            Row::ArtSize => {
                self.settings.art_size = self.settings.art_size.next();
                self.save_settings();
                self.set_notification(format!(
                    "🖼 Album art size: {}",
                    self.settings.art_size.label()
                ));
            }
            Row::AmbientEnabled => {
                self.settings.ambient_enabled = !self.settings.ambient_enabled;
                let state = if self.settings.ambient_enabled { "On" } else { "Off" };
                self.save_settings();
                self.invalidate_ambient();
                self.set_notification(format!("🖼 Ambient background: {}", state));
            }
            Row::AmbientStyle => {
                self.settings.ambient_style = self.settings.ambient_style.next();
                self.save_settings();
                self.invalidate_ambient();
                self.set_notification(format!(
                    "🖼 Ambient style: {}",
                    self.settings.ambient_style.label()
                ));
            }
            Row::AmbientDimming => {
                self.adjust_ambient_dimming(5);
            }
            Row::Visualizer => {
                self.settings.visualizer_enabled = !self.settings.visualizer_enabled;
                let state = if self.settings.visualizer_enabled { "On" } else { "Off" };
                self.save_settings();
                if self.settings.visualizer_enabled {
                    // Fresh retry of the input-method ladder.
                    self.viz_unavailable = false;
                    self.viz_attempt = 0;
                } else {
                    self.viz.take(); // Drop kills the child
                    self.viz_frame = None;
                }
                self.set_notification(format!("📊 Spectrum visualizer: {}", state));
            }
            Row::VolumeStep => {
                self.adjust_volume_step(1);
            }
            Row::SeekStep => {
                self.adjust_seek_step(1);
            }
            Row::Accent => {
                self.settings.accent = self.settings.accent.next();
                self.save_settings();
                self.set_notification(format!(
                    "🎨 Accent color: {}",
                    self.settings.accent.label()
                ));
            }
            Row::DefaultVolume => {
                // Enter has no toggle for a number; nudge up as a convenience.
                self.change_default_volume(5);
            }
            Row::Hotkey(action) => {
                self.rebinding = Some(*action);
                self.set_notification(format!("⌨ Press a new key for {} (Esc cancels)", action.label()));
            }
        }
    }

    /// Left/Right on a settings row.
    fn settings_adjust(&mut self, row: &crate::settings::SettingsRow, dir: i32) {
        use crate::settings::{SettingsRow as Row};
        match row {
            Row::DefaultVolume => self.change_default_volume(dir * 5),
            Row::ArtSize => {
                self.settings.art_size = if dir >= 0 {
                    self.settings.art_size.next()
                } else {
                    self.settings.art_size.prev()
                };
                self.save_settings();
                self.set_notification(format!("🖼 Album art size: {}", self.settings.art_size.label()));
            }
            Row::VolumeStep => self.adjust_volume_step(dir),
            Row::SeekStep => self.adjust_seek_step(dir),
            Row::AmbientDimming => self.adjust_ambient_dimming(dir * 5),
            Row::Accent => {
                self.settings.accent = if dir >= 0 {
                    self.settings.accent.next()
                } else {
                    self.settings.accent.prev()
                };
                self.save_settings();
                self.set_notification(format!("🎨 Accent color: {}", self.settings.accent.label()));
            }
            // Booleans and cycled enums toggle/step on either direction.
            Row::AmbientEnabled | Row::AmbientStyle | Row::Visualizer => {
                self.settings_activate(row);
            }
            // Booleans toggle on either direction.
            Row::StartupCache | Row::RememberState | Row::ShowLyrics => {
                self.settings_activate(row);
            }
            Row::MusicFolder | Row::Hotkey(_) => {}
        }
    }

    /// Adjust the ambient backdrop dimming (25-75 %) from Settings.
    fn adjust_ambient_dimming(&mut self, delta: i32) {
        let next = (self.settings.ambient_dimming as i32 + delta).clamp(
            crate::settings::AMBIENT_DIMMING_MIN as i32,
            crate::settings::AMBIENT_DIMMING_MAX as i32,
        ) as u8;
        if next != self.settings.ambient_dimming {
            self.settings.ambient_dimming = next;
            self.save_settings();
            self.invalidate_ambient();
        }
        self.set_notification(format!("🌗 Background dimming: {}%", next));
    }

    /// Adjust the volume step (1-25 %) from Settings.
    fn adjust_volume_step(&mut self, delta: i32) {
        let current = self.settings.volume_step as i32;
        let next = (current + delta).clamp(1, 25) as u8;
        if next != self.settings.volume_step {
            self.settings.volume_step = next;
            self.save_settings();
        }
        self.set_notification(format!("🔊 Volume step: ±{}%", next));
    }

    /// Adjust the seek step (1-60 s) from Settings.
    fn adjust_seek_step(&mut self, delta: i32) {
        let current = self.settings.seek_step_secs as i32;
        let next = (current + delta).clamp(1, 60) as u8;
        if next != self.settings.seek_step_secs {
            self.settings.seek_step_secs = next;
            self.save_settings();
        }
        self.set_notification(format!("⏩ Seek step: ±{}s", next));
    }

    /// Change the launch default volume from Settings (applies live too).
    fn change_default_volume(&mut self, delta: i32) {
        let current = self.settings.default_volume as i32;
        let next = (current + delta).clamp(0, 100) as u8;
        if next != self.settings.default_volume {
            self.settings.default_volume = next;
            self.audio.volume = next as u32;
            self.save_settings();
        }
        self.set_notification(format!("🔊 Default volume: {}%", next));
    }

    /// Persist the settings file after a change.
    pub fn save_settings(&mut self) {
        self.settings.save_to(&self.config_dir);
    }

    /// Drop the prepared ambient backdrop so the next frame rebuilds it
    /// (track-independent knobs changed: style, dimming, on/off).
    pub fn invalidate_ambient(&self) {
        self.ambient_cache.borrow_mut().take();
    }

    /// Whether the cava binary is known to be missing/unusable (Settings
    /// shows an install hint instead of an empty lane).
    pub fn viz_missing(&self) -> bool {
        self.viz_unavailable
    }

    // ------------------------------------------------------------------
    // Tick
    // ------------------------------------------------------------------

    /// Start/stop the cava worker per the Settings toggle, escalate through
    /// input methods when the child dies (silently-broken audio system), and
    /// grab the latest spectrum frame. Restart attempts settle into
    /// `viz_unavailable` after the ladder is exhausted, so a broken setup
    /// costs nothing per tick.
    fn poll_visualizer(&mut self) {
        if !self.settings.visualizer_enabled {
            if self.viz.take().is_some() {
                self.viz_frame = None;
            }
            return;
        }
        if self.viz_unavailable {
            return;
        }
        if self.viz.is_none() {
            if self.viz_attempt as usize >= crate::viz::INPUT_METHODS.len() {
                self.viz_unavailable = true;
                self.set_notification(
                    "📊 cava unavailable — check your audio system / install: sudo apt install cava",
                );
                return;
            }
            match crate::viz::VizWorker::spawn(&self.config_dir, self.viz_attempt) {
                Ok(worker) => self.viz = Some(worker),
                Err(crate::viz::SpawnError::Missing) => {
                    self.viz_unavailable = true;
                    self.set_notification("📊 cava not found — install: sudo apt install cava");
                    return;
                }
                Err(crate::viz::SpawnError::Io(e)) => {
                    // IO trouble (config write, permissions): settle instead
                    // of respawning every tick.
                    self.viz_unavailable = true;
                    self.set_notification(format!("📊 visualizer off — cava error: {e}"));
                    return;
                }
            }
        }
        // A dead child advances the pipewire → pulse ladder via the
        // worker's own method index; the respawn happens on the next tick.
        let alive = self.viz.as_mut().map(|v| v.alive()).unwrap_or(false);
        if !alive {
            self.viz_attempt = self
                .viz
                .as_ref()
                .and_then(|v| v.next_method_idx())
                .unwrap_or(crate::viz::INPUT_METHODS.len());
            self.viz.take(); // Drop reaps the corpse
            self.viz_frame = None;
            return;
        }
        if let Some(worker) = &mut self.viz {
            self.viz_frame = worker.latest();
        }
    }

    /// Main event loop tick
    pub fn tick(&mut self) {
        self.poll_scan();
        self.poll_visualizer();
        let track_finished = self.audio.tick();
        if track_finished {
            match self.audio.repeat_mode {
                crate::models::RepeatMode::Track => {
                    if let Some(track) = self.audio.current_track.clone() {
                        self.audio.play(track, 0.0);
                    }
                }
                crate::models::RepeatMode::All => {
                    self.play_next();
                }
                crate::models::RepeatMode::None => {
                    self.audio.stop();
                }
            }
        }
    }
}

/// Remembered-folder lookup with migration semantics: the walkz-tui config
/// wins when present; the legacy myx-tui config is the fallback.
pub(crate) fn load_last_folder_from(walkz: &Path, legacy: &Path) -> Option<PathBuf> {
    read_last_folder(&walkz.join("config.json")).or_else(|| read_last_folder(&legacy.join("config.json")))
}

fn read_last_folder(path: &Path) -> Option<PathBuf> {
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    let folder_str = v.get("last_folder")?.as_str()?;
    let p = PathBuf::from(folder_str);
    if p.exists() && p.is_dir() {
        Some(p)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Track;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::fs;

    const NONE: KeyModifiers = KeyModifiers::empty();

    fn tmp_root(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("walkz-app-test-{tag}-{}", std::process::id()))
    }

    fn make_track(id: &str, title: &str) -> Track {
        Track {
            id: id.into(),
            path: PathBuf::from(format!("/music/{}/track.m4a", id)),
            title: title.into(),
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

    /// App with an injected config dir and a real (empty) scanned folder so
    /// the constructor's startup scan targets a tiny dir, not ~/Music.
    fn make_app(tag: &str) -> App {
        let cfg = tmp_root(tag);
        let _ = fs::remove_dir_all(&cfg);
        let music = cfg.join("music");
        fs::create_dir_all(&music).unwrap();
        App::new_in(cfg, Some(music))
    }

    fn two_track_payload() -> ScanPayload {
        ScanPayload {
            tracks: vec![make_track("a", "First Light"), make_track("b", "Second Wind")],
            m3us: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Digit keys (Task 5 plan area 1, revised: digits always switch tabs)
    // ------------------------------------------------------------------

    #[test]
    fn digits_always_switch_main_tabs() {
        let mut app = make_app("digits-tabs");
        // Route through the digit key so focus resets to the tab bar.
        app.handle_key(KeyCode::Char('2'), NONE);
        assert_eq!(app.active_tab, AppTab::Categories);
        assert_eq!(app.focus, Focus::TabBar);

        // Even with a Categories pane focused, digits switch MAIN tabs —
        // the sub-tab strip is driven by ←/→ only.
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::CategoryTabs);
        app.handle_key(KeyCode::Char('3'), NONE);
        assert_eq!(app.active_tab, AppTab::Queue, "digit jumps out of Categories");

        // Same from the category list itself.
        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Tab, NONE); // TabBar -> CategoryTabs
        app.handle_key(KeyCode::Tab, NONE); // CategoryTabs -> CategoryList
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Char('5'), NONE);
        assert_eq!(app.active_tab, AppTab::AudioOutput);

        fs::remove_dir_all(tmp_root("digits-tabs")).ok();
    }

    #[test]
    fn digits_outside_categories_switch_tabs() {
        let mut app = make_app("digits-other");

        app.handle_key(KeyCode::Char('3'), NONE);
        assert_eq!(app.active_tab, AppTab::Queue);
        app.handle_key(KeyCode::Char('4'), NONE);
        assert_eq!(app.active_tab, AppTab::MetadataEditor);
        app.handle_key(KeyCode::Char('5'), NONE);
        assert_eq!(app.active_tab, AppTab::AudioOutput);
        app.handle_key(KeyCode::Char('1'), NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);
        app.handle_key(KeyCode::Char('2'), NONE);
        assert_eq!(app.active_tab, AppTab::Categories);

        fs::remove_dir_all(tmp_root("digits-other")).ok();
    }

    // ------------------------------------------------------------------
    // Categories arrows and focus skipping
    // ------------------------------------------------------------------

    #[test]
    fn arrows_switch_category_type_from_anywhere_in_categories() {
        let mut app = make_app("cat-arrows");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("cat-arrows"), two_track_payload());

        app.handle_key(KeyCode::Char('2'), NONE); // Categories, TabBar focus
        assert_eq!(app.category_type, CategoryType::AlbumArtist);

        // ←/→ switch the sub-tab even while a list pane owns focus.
        app.handle_key(KeyCode::Tab, NONE);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Right, NONE);
        assert_eq!(app.category_type, CategoryType::Song);
        app.handle_key(KeyCode::Right, NONE);
        assert_eq!(app.category_type, CategoryType::Artist);
        app.handle_key(KeyCode::Left, NONE);
        assert_eq!(app.category_type, CategoryType::Song);
        app.handle_key(KeyCode::Left, NONE);
        assert_eq!(app.category_type, CategoryType::AlbumArtist);

        fs::remove_dir_all(tmp_root("cat-arrows")).ok();
    }

    #[test]
    fn arrows_seek_outside_categories() {
        let mut app = make_app("seek-arrows");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("seek-arrows"), two_track_payload());

        app.handle_key(KeyCode::Right, NONE); // Now Playing: seek, not tabs
        assert_eq!(app.active_tab, AppTab::NowPlaying);
        // (seek itself is a no-op with nothing playing; tab must not change)

        fs::remove_dir_all(tmp_root("seek-arrows")).ok();
    }

    #[test]
    fn tab_focus_skips_dead_category_panes() {
        let mut app = make_app("focus-skip");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("focus-skip"), two_track_payload());
        app.handle_key(KeyCode::Char('2'), NONE);

        // In the Album Artist tab the full ring is reachable.
        app.handle_key(KeyCode::Tab, NONE); // TabBar -> CategoryTabs
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryList
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryMid
        assert_eq!(app.focus, Focus::CategoryMid);
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryTracks
        app.handle_key(KeyCode::Tab, NONE); // -> TabBar
        assert_eq!(app.focus, Focus::TabBar);

        // In the Song tab there is no track pane: Mid and Tracks are
        // skipped, so Tab cycles TabBar -> Tabs -> List -> TabBar.
        app.handle_key(KeyCode::Right, NONE); // AlbumArtist -> Song
        assert_eq!(app.category_type, CategoryType::Song);
        app.handle_key(KeyCode::Tab, NONE); // TabBar -> CategoryTabs
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryList
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Tab, NONE); // skips Mid + Tracks -> TabBar
        assert_eq!(app.focus, Focus::TabBar);

        fs::remove_dir_all(tmp_root("focus-skip")).ok();
    }

    #[test]
    fn album_artist_enter_drills_down() {
        let mut app = make_app("aa-drill");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("aa-drill"), two_track_payload());

        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryTabs
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryList
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.focus, Focus::CategoryMid, "Enter opens the album list");
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.focus, Focus::CategoryTracks, "Enter opens the tracks");
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying, "Enter plays");
        assert!(app.audio.current_track.is_some());

        fs::remove_dir_all(tmp_root("aa-drill")).ok();
    }

    #[test]
    fn song_enter_plays_in_place() {
        let mut app = make_app("song-play");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("song-play"), two_track_payload());

        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Right, NONE); // -> Song
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryTabs
        app.handle_key(KeyCode::Tab, NONE); // -> CategoryList (songs)
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.active_tab, AppTab::Categories, "stays in the song list");
        assert!(app.audio.current_track.is_some(), "the highlighted song plays");

        fs::remove_dir_all(tmp_root("song-play")).ok();
    }

    #[test]
    fn album_artist_mid_navigation_repopulates_tracks() {
        let mut app = make_app("aa-mid");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("aa-mid"), two_track_payload());

        app.handle_key(KeyCode::Char('2'), NONE);
        assert_eq!(app.category_type, CategoryType::AlbumArtist);
        // Two tracks of the same album artist: the mid list has one album.
        assert_eq!(app.category_mid_items.len(), 1);
        assert_eq!(app.category_mid_items[0].0, "Night Drive");
        assert_eq!(app.active_playlist_tracks.len(), 2);

        // Arrow keys in the middle list walk the albums.
        app.focus = Focus::CategoryMid;
        app.handle_key(KeyCode::Down, NONE);
        assert_eq!(app.selected_category_mid_idx, 0, "single album stays put");

        fs::remove_dir_all(tmp_root("aa-mid")).ok();
    }

    #[test]
    fn song_listing_is_sorted_and_maps_selection_to_track() {
        let mut app = make_app("song-sort");
        app.scan_state = None;
        let payload = ScanPayload {
            tracks: vec![
                make_track("a", "Zebra Road"),
                make_track("b", "apple tree"),
                make_track("c", "Midnight Sun"),
            ],
            m3us: Vec::new(),
        };
        app.apply_scan_results_at(tmp_root("song-sort"), payload);

        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Right, NONE); // -> Song
        // Case-insensitive alphabetical order.
        assert_eq!(app.active_category_name, "apple tree", "first row sorts first");
        assert_eq!(app.active_playlist_tracks.len(), 1);
        assert_eq!(app.library.tracks[app.active_playlist_tracks[0]].id, "b");

        // Focus the song list and move down: the next song is selected
        // exactly (by index, not by title match).
        app.handle_key(KeyCode::Tab, NONE);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Down, NONE);
        assert_eq!(app.active_category_name, "Midnight Sun");
        assert_eq!(app.library.tracks[app.active_playlist_tracks[0]].id, "c");

        fs::remove_dir_all(tmp_root("song-sort")).ok();
    }

    #[test]
    fn playlist_category_shows_only_user_playlists() {
        let mut app = make_app("user-pl");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("user-pl"), two_track_payload());

        // No user playlists yet: the category is empty.
        app.category_type = CategoryType::Playlist;
        app.refresh_active_category();
        assert!(app.library.get_category_items(CategoryType::Playlist).is_empty());

        // Creating one makes it show up — exactly one, no folder entries.
        app.user_data.create_playlist("Road Trip", "").unwrap();
        app.after_library_rebuild();
        app.refresh_active_category();
        let items = app.library.get_category_items(CategoryType::Playlist);
        assert_eq!(items.len(), 1, "{items:?}");
        assert_eq!(items[0].0, "♪ Road Trip");

        fs::remove_dir_all(tmp_root("user-pl")).ok();
    }

    // ------------------------------------------------------------------
    // Esc handling (review: Important #1 — with scan precedence)
    // ------------------------------------------------------------------

    #[test]
    fn esc_from_categories_returns_to_now_playing() {
        let mut app = make_app("esc-cat");
        app.active_tab = AppTab::Categories;
        app.scan_state = None; // startup scan settled; no overlay up

        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);

        fs::remove_dir_all(tmp_root("esc-cat")).ok();
    }

    #[test]
    fn esc_while_scanning_cancels_scan_before_tab_behaviour() {
        let mut app = make_app("esc-scan");
        app.active_tab = AppTab::Categories;
        app.scan_state = Some(ScanState::new(tmp_root("esc-scan")));

        app.handle_key(KeyCode::Esc, NONE);

        // Scan-cancel wins while the overlay is up: scan dismissed, tab unchanged
        assert!(app.scan_state.is_none());
        assert_eq!(app.active_tab, AppTab::Categories);

        // Once no scan is running, Esc performs the tab behaviour
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);

        fs::remove_dir_all(tmp_root("esc-scan")).ok();
    }

    // ------------------------------------------------------------------
    // Favourites via menu action (Task 5 plan area 2)
    // ------------------------------------------------------------------

    #[test]
    fn menu_favourite_toggles_and_persists() {
        let mut app = make_app("fav");
        // Load the library the way a real scan would (tracks + rebuilt
        // indexes), then populate the tracklist from the album category.
        app.library.tracks = vec![make_track("a", "First Light")];
        app.library.rebuild_indexes();
        app.after_library_rebuild();
        assert_eq!(app.active_playlist_tracks, vec![0]);
        app.selected_tracklist_idx = 0;

        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 2; // ☆ Favourite
        app.menu_activate();

        let path = app.library.tracks[0].path.to_string_lossy().to_string();
        assert!(app.user_data.is_favourite(&path));
        assert!(
            app.get_active_notification().unwrap_or("").contains("Favourited"),
            "notification: {:?}",
            app.get_active_notification()
        );

        // Survives a reload from disk
        let reloaded = UserData::load(&app.config_dir);
        assert!(reloaded.is_favourite(&path));

        // Toggle again: unfavourite
        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 2;
        app.menu_activate();
        assert!(!app.user_data.is_favourite(&path));
        assert!(app
            .get_active_notification()
            .unwrap_or("")
            .contains("Removed from favourites"));

        fs::remove_dir_all(tmp_root("fav")).ok();
    }

    // ------------------------------------------------------------------
    // Playlist create / dedupe (Task 5 plan area 3)
    // ------------------------------------------------------------------

    #[test]
    fn create_playlist_via_form_persists_and_duplicates_rejected() {
        let mut app = make_app("create-pl");

        app.open_create_playlist_standalone();
        for ch in "Mix".chars() {
            app.handle_key(KeyCode::Char(ch), NONE);
        }
        app.handle_key(KeyCode::Tab, NONE); // description field
        for ch in "night drive".chars() {
            app.handle_key(KeyCode::Char(ch), NONE);
        }
        app.handle_key(KeyCode::Enter, NONE);

        // Form closed, playlist created, notification fired
        assert!(app.create_playlist.is_none());
        assert_eq!(app.user_data.playlists.len(), 1);
        assert_eq!(app.user_data.playlists[0].name, "Mix");
        assert_eq!(app.user_data.playlists[0].description, "night drive");
        assert!(app
            .get_active_notification()
            .unwrap_or("")
            .contains("Created playlist"));

        // Reload from disk
        let reloaded = UserData::load(&app.config_dir);
        assert_eq!(reloaded.playlists.len(), 1);

        // Duplicate (different case) → error, form stays open, inputs kept
        app.open_create_playlist_standalone();
        for ch in "mix".chars() {
            app.handle_key(KeyCode::Char(ch), NONE);
        }
        app.handle_key(KeyCode::Enter, NONE);
        let state = app.create_playlist.as_ref().unwrap();
        assert!(state.error.as_deref().unwrap_or("").contains("already exists"));
        assert_eq!(state.name_input, "mix");
        assert_eq!(app.user_data.playlists.len(), 1);

        fs::remove_dir_all(tmp_root("create-pl")).ok();
    }

    // ------------------------------------------------------------------
    // Add to playlist from menu (Task 5 plan area 4)
    // ------------------------------------------------------------------

    #[test]
    fn add_to_playlist_from_menu_persists() {
        let mut app = make_app("add-pl");
        app.library.tracks = vec![make_track("a", "First Light")];
        app.active_playlist_tracks = vec![0];
        app.selected_tracklist_idx = 0;
        app.user_data.create_playlist("Road Trip", "").unwrap();

        app.open_song_menu();
        assert_eq!(app.menu.as_ref().unwrap().section, MenuSection::Main);
        app.menu.as_mut().unwrap().selected = 0; // Add to a Playlist…
        app.menu_activate();
        assert_eq!(app.menu.as_ref().unwrap().section, MenuSection::PlaylistPicker);

        app.menu.as_mut().unwrap().selected = 0; // Road Trip
        app.menu_activate();
        assert!(app.menu.is_none());

        let path = app.library.tracks[0].path.to_string_lossy().to_string();
        let reloaded = UserData::load(&app.config_dir);
        assert!(reloaded.playlists[0].track_paths.contains(&path));

        fs::remove_dir_all(tmp_root("add-pl")).ok();
    }

    #[test]
    fn menu_esc_steps_back_through_sections() {
        let mut app = make_app("menu-flow");
        app.library.tracks = vec![make_track("a", "First Light")];
        app.active_playlist_tracks = vec![0];

        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 0;
        app.menu_activate(); // → picker
        assert_eq!(app.menu.as_ref().unwrap().section, MenuSection::PlaylistPicker);

        app.handle_key(KeyCode::Esc, NONE); // picker → main
        assert_eq!(app.menu.as_ref().unwrap().section, MenuSection::Main);

        app.handle_key(KeyCode::Esc, NONE); // main → closed
        assert!(app.menu.is_none());

        fs::remove_dir_all(tmp_root("menu-flow")).ok();
    }

    // ------------------------------------------------------------------
    // Focus cycling (Task 5 plan area 5)
    // ------------------------------------------------------------------

    #[test]
    fn now_playing_tab_focus_ring() {
        let mut app = make_app("focus");
        assert_eq!(app.focus, Focus::NowTracklist);

        // Tab: tracklist -> lyrics -> tab bar -> tracklist.
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::NowLyrics);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::TabBar);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::NowTracklist);

        // BackTab walks the other way.
        app.handle_key(KeyCode::BackTab, NONE);
        assert_eq!(app.focus, Focus::TabBar);

        fs::remove_dir_all(tmp_root("focus")).ok();
    }

    #[test]
    fn digit_routes_by_focus_on_categories() {
        let mut app = make_app("digits");
        // On Now Playing (default focus), digits switch main tabs directly.
        app.handle_key(KeyCode::Char('6'), NONE);
        assert_eq!(app.active_tab, AppTab::Settings);
        assert_eq!(app.focus, Focus::SettingsList);

        // Categories starts on the tab bar: digits switch main tabs.
        app.handle_key(KeyCode::Char('2'), NONE);
        assert_eq!(app.active_tab, AppTab::Categories);
        assert_eq!(app.focus, Focus::TabBar);
        app.handle_key(KeyCode::Char('3'), NONE);
        assert_eq!(app.active_tab, AppTab::Queue);

        // Even with the sub-tab strip focused, digits are main tabs only —
        // the strip is display-only (←/→ drives the category type).
        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::CategoryTabs);
        app.handle_key(KeyCode::Char('1'), NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);
        assert_eq!(app.category_type, CategoryType::AlbumArtist);

        fs::remove_dir_all(tmp_root("digits")).ok();
    }

    #[test]
    fn categories_enter_and_esc_walk_focus() {
        let mut app = make_app("catesc");
        app.scan_state = None;
        app.apply_scan_results_at(tmp_root("catesc"), two_track_payload());
        app.handle_key(KeyCode::Char('2'), NONE); // -> Categories, tab bar
        // Esc from the tab bar leaves Categories entirely.
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);

        // Enter walks down the drill: list -> mid (albums) -> tracks.
        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Tab, NONE);
        app.handle_key(KeyCode::Tab, NONE);
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.focus, Focus::CategoryMid);
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.focus, Focus::CategoryTracks);
        // Enter there plays and returns to Now Playing.
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);

        // Esc walks back up: from the tracklist over the mid list, list,
        // sub-tabs, then out of the tab.
        app.handle_key(KeyCode::Char('2'), NONE);
        app.handle_key(KeyCode::Tab, NONE);
        app.handle_key(KeyCode::Tab, NONE);
        app.handle_key(KeyCode::Enter, NONE);
        app.handle_key(KeyCode::Enter, NONE);
        assert_eq!(app.focus, Focus::CategoryTracks);
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.focus, Focus::CategoryMid);
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.focus, Focus::CategoryList);
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.focus, Focus::CategoryTabs);
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.focus, Focus::TabBar);
        app.handle_key(KeyCode::Esc, NONE);
        assert_eq!(app.active_tab, AppTab::NowPlaying);

        fs::remove_dir_all(tmp_root("catesc")).ok();
    }

    #[test]
    fn hotkey_rebind_flow() {
        let mut app = make_app("rebind");
        // Default global key: 'n' plays next.
        app.handle_key(KeyCode::Char('n'), NONE);
        // Rebind NextTrack to 'x' in Settings.
        app.handle_key(KeyCode::Char('6'), NONE);
        let rows = crate::settings::settings_rows();
        let next_row = rows
            .iter()
            .position(|r| matches!(r, crate::settings::SettingsRow::Hotkey(crate::settings::HotkeyAction::NextTrack)))
            .unwrap();
        app.settings_selected = next_row;
        app.handle_key(KeyCode::Enter, NONE); // enters rebind mode
        assert!(app.rebinding.is_some());
        app.handle_key(KeyCode::Char('x'), NONE);
        assert!(app.rebinding.is_none());
        assert_eq!(app.settings.hotkey_char(crate::settings::HotkeyAction::NextTrack), 'x');
        // The old key is released; the new key performs the action…
        app.handle_key(KeyCode::Char('x'), NONE);
        // …and 'n' no longer does anything global.
        app.notification = None;
        app.handle_key(KeyCode::Char('n'), NONE);
        assert!(app.notification.is_none());

        // Esc cancels a pending rebind.
        app.handle_key(KeyCode::Char('6'), NONE);
        app.handle_key(KeyCode::Enter, NONE);
        assert!(app.rebinding.is_some());
        app.handle_key(KeyCode::Esc, NONE);
        assert!(app.rebinding.is_none());

        // Settings persist to disk.
        assert!(app.config_dir.join("settings.json").exists());

        fs::remove_dir_all(tmp_root("rebind")).ok();
    }

    #[test]
    fn settings_toggles_and_defaults() {
        let mut app = make_app("setvar");
        app.handle_key(KeyCode::Char('6'), NONE);
        assert_eq!(app.focus, Focus::SettingsList);
        // Row 0 = Music folder (Enter opens the dialog), row 1 = StartupCache.
        app.settings_selected = 1;
        app.handle_key(KeyCode::Enter, NONE);
        assert!(!app.settings.load_library_cache_on_launch);
        // Left/Right on DefaultVolume adjust by 5.
        app.settings_selected = 2;
        app.handle_key(KeyCode::Left, NONE);
        assert_eq!(app.settings.default_volume, 80);
        app.handle_key(KeyCode::Right, NONE);
        assert_eq!(app.settings.default_volume, 85);
        // Volume changes applied live too.
        assert_eq!(app.audio.volume, 85);
        // ArtSize cycles with Right.
        app.settings_selected = 5;
        app.handle_key(KeyCode::Right, NONE);
        assert_eq!(app.settings.art_size, crate::settings::ArtSize::Small);
        // Reload from disk and check persistence.
        let reloaded = crate::settings::Settings::load_from(&app.config_dir);
        assert!(!reloaded.load_library_cache_on_launch);
        assert_eq!(reloaded.default_volume, 85);
        assert_eq!(reloaded.art_size, crate::settings::ArtSize::Small);

        fs::remove_dir_all(tmp_root("setvar")).ok();
    }

    #[test]
    fn remember_state_persists_repeat_shuffle() {
        let mut app = make_app("remember");
        // Enable remembering (row 3) while repeat=All/shuffle=Off.
        app.handle_key(KeyCode::Char('6'), NONE);
        app.settings_selected = 3;
        app.handle_key(KeyCode::Enter, NONE);
        assert!(app.settings.remember_repeat_shuffle);
        // Toggle shuffle via the global hotkey: snapshot saved.
        app.handle_key(KeyCode::Char('s'), NONE);
        let reloaded = crate::settings::Settings::load_from(&app.config_dir);
        assert_eq!(reloaded.saved_shuffle, Some(true));
        assert_eq!(reloaded.saved_repeat, Some(crate::models::RepeatMode::All));

        fs::remove_dir_all(tmp_root("remember")).ok();
    }

    // ------------------------------------------------------------------
    // Scan-apply semantics (Task 5 plan area 7)
    // ------------------------------------------------------------------

    #[test]
    fn scan_apply_preloads_first_track_only_when_idle() {
        let mut app = make_app("apply");
        let folder = tmp_root("apply");

        // Fresh connect (idle): first track preloaded at position 0
        app.apply_scan_results_at(folder.clone(), two_track_payload());
        assert_eq!(app.library.tracks.len(), 2);
        assert_eq!(app.audio.current_track.as_ref().unwrap().id, "a");
        assert_eq!(app.audio.position_secs, 0.0);

        // Simulate playback of the second track, then re-scan same folder:
        // current playback must survive
        let second = app.library.tracks[1].clone();
        app.audio.current_track = Some(second);
        app.apply_scan_results_at(folder.clone(), two_track_payload());
        assert_eq!(app.audio.current_track.as_ref().unwrap().id, "b");

        // Re-scan where the playing track no longer exists → preload first
        let gone_payload = ScanPayload {
            tracks: vec![make_track("c", "Replacement")],
            m3us: Vec::new(),
        };
        app.apply_scan_results_at(folder, gone_payload);
        assert_eq!(app.audio.current_track.as_ref().unwrap().id, "c");

        fs::remove_dir_all(tmp_root("apply")).ok();
    }

    /// A worker that dies without `Finished` (panic, cancelled mid-walk)
    /// must not leave the scan overlay up forever: the channel disconnects,
    /// and `poll_scan` clears the state with a failure notification.
    #[test]
    fn poll_scan_clears_overlay_when_worker_dies_without_finished() {
        let mut app = make_app("scan-died");
        app.scan_state = None;

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        tx.send(crate::scanner::ScanEvent::Walking { dirs: 3, files: 12 })
            .unwrap();
        // No Listed/Finished: the sender is dropped right away, as when a
        // worker thread panics during the walk.
        drop(tx);
        app.scan_state = Some(ScanState::new_empty(ScanJob::from_parts(rx, cancel)));

        app.poll_scan();
        assert!(app.scan_state.is_none(), "overlay must close on disconnect");
        assert!(
            app.get_active_notification().unwrap_or_default().contains("aborted"),
            "user must be told the scan died"
        );

        // Subsequent polls are no-ops (no state left to poll).
        app.poll_scan();
        assert!(app.scan_state.is_none());

        fs::remove_dir_all(tmp_root("scan-died")).ok();
    }

    /// Walking heartbeats flow into the overlay state while `listed` is
    /// still false, and never clobber parse progress afterwards.
    #[test]
    fn poll_scan_tracks_walking_counts() {
        let mut app = make_app("scan-walk");
        app.scan_state = None;

        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        tx.send(crate::scanner::ScanEvent::Walking { dirs: 12, files: 34 })
            .unwrap();
        tx.send(crate::scanner::ScanEvent::Listed(2)).unwrap();
        tx.send(crate::scanner::ScanEvent::Progress {
            files_done: 1,
            files_total: 2,
            tracks_found: 1,
            current: "x.mp3".into(),
        })
        .unwrap();
        tx.send(crate::scanner::ScanEvent::Finished(Box::new(two_track_payload())))
            .unwrap();
        drop(tx);
        app.scan_state = Some(ScanState::new_empty(ScanJob::from_parts(rx, cancel)));

        app.poll_scan();
        // Finished wins: results applied, overlay closed.
        assert!(app.scan_state.is_none());
        assert_eq!(app.library.tracks.len(), 2);

        fs::remove_dir_all(tmp_root("scan-walk")).ok();
    }

    /// A finished scan writes the launch cache into the config dir; the
    /// next `new_in` with the same folder restores the library instantly
    /// (no startup scan) and announces it.
    #[test]
    fn scan_finish_writes_cache_and_next_launch_restores_it() {
        let tag = "cache-launch";
        let cfg = tmp_root(tag);
        let _ = std::fs::remove_dir_all(&cfg);
        let music = cfg.join("music");
        std::fs::create_dir_all(&music).unwrap();

        // 1) A scan finishing writes the cache.
        let mut app = App::new_in(cfg.clone(), Some(music.clone()));
        app.scan_state = None;
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        tx.send(crate::scanner::ScanEvent::Finished(Box::new(two_track_payload())))
            .unwrap();
        drop(tx);
        app.scan_state = Some(ScanState::new_empty(ScanJob::from_parts(rx, cancel)));
        app.scan_state.as_mut().unwrap().folder = music.clone();
        app.poll_scan();

        let cache_path = cfg.join("library_cache.json");
        assert!(cache_path.exists(), "finished scan should write the cache");

        // 2) The next launch restores the library without a scan.
        let app2 = App::new_in(cfg.clone(), Some(music.clone()));
        assert!(
            app2.scan_state.is_none(),
            "cache hit must skip the startup scan"
        );
        assert_eq!(app2.library.tracks.len(), 2);
        assert_eq!(app2.library.folder_path, Some(music));
        let (note, _) = app2.notification.expect("cache hit announces itself");
        assert!(note.contains("Library ready"), "note was: {note}");

        std::fs::remove_dir_all(cfg).ok();
    }

    /// The cache is only valid for the folder it was scanned from: a
    /// different (existing) folder falls back to the startup scan.
    #[test]
    fn library_cache_ignored_when_folder_changes() {
        let tag = "cache-mismatch";
        let cfg = tmp_root(tag);
        let _ = std::fs::remove_dir_all(&cfg);
        let music_a = cfg.join("music-a");
        let music_b = cfg.join("music-b");
        std::fs::create_dir_all(&music_a).unwrap();
        std::fs::create_dir_all(&music_b).unwrap();

        let mut app = App::new_in(cfg.clone(), Some(music_a.clone()));
        app.scan_state = None;
        app.save_library_cache(&music_a, &two_track_payload().tracks, &[]);

        // Different folder -> no cache hit, scan starts.
        let app2 = App::new_in(cfg.clone(), Some(music_b));
        assert!(app2.scan_state.is_some(), "folder change must trigger a scan");

        std::fs::remove_dir_all(cfg).ok();
    }

    /// A corrupt or empty cache is skipped, never trusted.
    #[test]
    fn library_cache_ignored_when_corrupt_or_empty() {
        let tag = "cache-corrupt";
        let cfg = tmp_root(tag);
        let _ = std::fs::remove_dir_all(&cfg);
        let music = cfg.join("music");
        std::fs::create_dir_all(&music).unwrap();

        std::fs::write(cfg.join("library_cache.json"), "{not json").unwrap();
        let app = App::new_in(cfg.clone(), Some(music.clone()));
        assert!(app.scan_state.is_some(), "corrupt cache must not be trusted");

        // Valid JSON but zero tracks (e.g. an empty folder was cached).
        std::fs::write(
            cfg.join("library_cache.json"),
            r#"{"version":2,"folder":"/x","scanned_at":0,"tracks":[],"m3us":[]}"#,
        )
        .unwrap();
        let app2 = App::new_in(cfg, Some(music));
        assert!(app2.scan_state.is_some(), "empty cache must not be trusted");
    }

    /// Cover art is loaded lazily and cached per (path, mtime): the scan
    /// keeps no art bytes, the loader reads them on first display.
    #[test]
    fn cover_art_loads_lazily_and_is_cached() {
        let mut app = make_app("cover-lazy");
        app.scan_state = None; // startup scan settled
        app.apply_scan_results_at(tmp_root("cover-lazy"), two_track_payload());

        let track = app.library.tracks[0].clone();
        assert!(track.cover_art.is_none(), "scan must not embed art bytes");

        // The test fixture has no artwork next to it — the loader returns
        // None but caches that outcome per (path, mtime).
        assert!(app.cover_art_bytes(&track).is_none());
        assert!(app.art_cache.borrow().is_some(), "outcome cached");

        // A track with injected bytes (as tests/other callers may provide)
        // is served directly without touching the cache.
        let mut with_art = track.clone();
        with_art.cover_art = Some(vec![1, 2, 3]);
        assert_eq!(app.cover_art_bytes(&with_art).as_deref(), Some(&[1u8, 2, 3][..]));

        fs::remove_dir_all(tmp_root("cover-lazy")).ok();
    }

    // ------------------------------------------------------------------
    // Config migration fallback (Task 5 plan area 8)
    // ------------------------------------------------------------------

    #[test]
    fn legacy_config_migration_prefers_walkz_then_myx() {
        let base = tmp_root("migrate");
        let _ = fs::remove_dir_all(&base);
        let walkz = base.join("walkz-tui");
        let legacy = base.join("myx-tui");
        fs::create_dir_all(&walkz).unwrap();
        fs::create_dir_all(&legacy).unwrap();

        // Neither config → None
        assert_eq!(load_last_folder_from(&walkz, &legacy), None);

        // Legacy present (points at a REAL dir) and walkz absent → legacy wins
        let old_music = base.join("old-music");
        fs::create_dir_all(&old_music).unwrap();
        fs::write(
            legacy.join("config.json"),
            format!(r#"{{"last_folder":"{}"}}"#, old_music.display()),
        )
        .unwrap();
        assert_eq!(load_last_folder_from(&walkz, &legacy), Some(old_music.clone()));

        // walkz config now present → wins over legacy
        let new_music = base.join("new-music");
        fs::create_dir_all(&new_music).unwrap();
        fs::write(
            walkz.join("config.json"),
            format!(r#"{{"last_folder":"{}"}}"#, new_music.display()),
        )
        .unwrap();
        assert_eq!(load_last_folder_from(&walkz, &legacy), Some(new_music));

        // Stale paths (non-existent dirs) are rejected
        fs::write(walkz.join("config.json"), r#"{"last_folder":"/no/such/dir"}"#).unwrap();
        assert_eq!(load_last_folder_from(&walkz, &legacy), Some(old_music));

        fs::remove_dir_all(&base).ok();
    }

    // ------------------------------------------------------------------
    // Folder dialog validation (unit-level; happy path in integration test)
    // ------------------------------------------------------------------

    #[test]
    fn folder_dialog_validates_paths() {
        let mut app = make_app("dialog");
        app.open_folder_dialog();

        // Empty input
        app.handle_key(KeyCode::Enter, NONE);
        assert!(app.is_folder_dialog_open);
        assert!(app.folder_dialog_error.is_some());

        // Non-existent directory
        app.paste_text("/no/such/dir/anywhere");
        app.handle_key(KeyCode::Enter, NONE);
        assert!(app.is_folder_dialog_open);
        assert!(app.folder_dialog_error.clone().unwrap_or_default().contains("does not exist"));

        // Valid directory → dialog closes, async scan starts
        let music = tmp_root("dialog").join("music");
        app.folder_dialog_input.clear(); // paste appends; simulate a fresh paste
        app.paste_text(&music.display().to_string());
        app.handle_key(KeyCode::Enter, NONE);
        assert!(!app.is_folder_dialog_open);
        assert!(app.scan_state.is_some());

        fs::remove_dir_all(tmp_root("dialog")).ok();
    }

    // ------------------------------------------------------------------
    // Caps-Lock tolerance: unbound uppercase letters fall back to the
    // lowercase action, so `O` with Caps Lock on still opens the dialog.
    // crossterm reports SHIFT for every uppercase char byte, so these
    // tests pass SHIFT exactly the way the real event loop delivers it.
    // ------------------------------------------------------------------

    #[test]
    fn caps_lock_o_still_opens_folder_dialog() {
        let mut app = make_app("caps-o");
        assert!(!app.is_folder_dialog_open);
        // Caps Lock produces 'O' with SHIFT reported (crossterm always sets
        // SHIFT for uppercase chars).
        app.handle_key(KeyCode::Char('O'), KeyModifiers::SHIFT);
        assert!(app.is_folder_dialog_open);
    }

    #[test]
    fn caps_lock_a_still_opens_song_menu() {
        let mut app = make_app("caps-a");
        // The song menu needs a selected track, same as pressing `a`.
        app.library.tracks = vec![make_track("a", "First Light")];
        app.library.rebuild_indexes();
        app.after_library_rebuild();
        app.selected_tracklist_idx = 0;
        assert!(app.menu.is_none());
        // Caps Lock yields `A` (reported with SHIFT); no action is bound to
        // `A`, so the lowercase fallback routes it to Song Menu.
        app.handle_key(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert!(app.menu.is_some());
    }

    #[test]
    fn caps_and_shift_s_both_rescan() {
        // `S` arrives the same way from Shift+S and Caps Lock+S, and stays
        // bound to Rescan — the fallback must not shadow real bindings.
        let mut app = make_app("caps-s");
        app.scan_state = None;
        app.handle_key(KeyCode::Char('S'), KeyModifiers::SHIFT);
        assert!(app.scan_state.is_some());
    }

    #[test]
    fn unbound_uppercase_without_shift_falls_back_too() {
        // Some terminals may deliver an uppercase char without SHIFT;
        // the fallback must not depend on the modifier bit.
        let mut app = make_app("caps-o-nomod");
        app.handle_key(KeyCode::Char('O'), KeyModifiers::empty());
        assert!(app.is_folder_dialog_open);
    }

    // ------------------------------------------------------------------
    // Folder dialog typing: the first typed/pasted path replaces the
    // pre-filled current folder instead of appending to it.
    // ------------------------------------------------------------------

    #[test]
    fn dialog_typing_replaces_prefilled_path() {
        let mut app = make_app("dialog-replace");
        let music = tmp_root("dialog-replace").join("music");
        // Give the library a folder so the dialog pre-fills with it.
        app.apply_scan_results_at(music.clone(), two_track_payload());

        app.open_folder_dialog();
        assert_eq!(app.folder_dialog_input, music.display().to_string());

        // First keystroke replaces the whole prefill…
        app.handle_key(KeyCode::Char('/'), NONE);
        app.handle_key(KeyCode::Char('h'), NONE);
        assert_eq!(app.folder_dialog_input, "/h");

        // …and once the input differs, typing appends again.
        app.handle_key(KeyCode::Char('i'), NONE);
        assert_eq!(app.folder_dialog_input, "/hi");

        // Pasting over the untouched prefill replaces it too.
        app.close_folder_dialog();
        app.open_folder_dialog();
        app.paste_text("/somewhere/else");
        assert_eq!(app.folder_dialog_input, "/somewhere/else");

        fs::remove_dir_all(tmp_root("dialog-replace")).ok();
    }

    #[test]
    fn dialog_typing_after_clear_appends() {
        let mut app = make_app("dialog-clear");
        let music = tmp_root("dialog-clear").join("music");
        app.apply_scan_results_at(music.clone(), two_track_payload());

        app.open_folder_dialog();
        // Ctrl+U clears the prefill; the next paste appends to the empty
        // input (a deliberate way to build a path piece by piece).
        app.handle_key(KeyCode::Char('u'), KeyModifiers::CONTROL);
        assert!(app.folder_dialog_input.is_empty());
        app.paste_text("~/Music");
        assert_eq!(app.folder_dialog_input, "~/Music");

        fs::remove_dir_all(tmp_root("dialog-clear")).ok();
    }

    // ------------------------------------------------------------------
    // Save errors surface in the notification (review: Minor #6)
    // ------------------------------------------------------------------

    #[test]
    fn save_failures_surface_as_notification() {
        let mut app = make_app("save-err");
        // Point the config dir INSIDE a regular file → create_dir_all fails
        let blocker_parent = tmp_root("save-err-parent");
        let _ = fs::remove_dir_all(&blocker_parent);
        fs::write(&blocker_parent, b"x").unwrap();
        app.config_dir = blocker_parent.join("cfg");

        app.library.tracks = vec![make_track("a", "First Light")];
        app.active_playlist_tracks = vec![0];
        app.selected_tracklist_idx = 0;

        app.open_song_menu();
        app.menu.as_mut().unwrap().selected = 2; // favourite → save fails
        app.menu_activate();

        let notif = app.get_active_notification().unwrap_or("");
        assert!(
            notif.contains("couldn't persist"),
            "notification: {:?}",
            notif
        );

        // In-memory state still toggled despite the failed persist
        let path = app.library.tracks[0].path.to_string_lossy().to_string();
        assert!(app.user_data.is_favourite(&path));

        let _ = fs::remove_file(tmp_root("save-err-parent"));
        fs::remove_dir_all(tmp_root("save-err")).ok();
    }
}