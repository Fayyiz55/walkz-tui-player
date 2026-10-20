use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppTab {
    NowPlaying = 0,
    Categories = 1,
    Queue = 2,
    MetadataEditor = 3,
    AudioOutput = 4,
    Settings = 5,
}

#[allow(dead_code)]
impl AppTab {
    pub fn next(&self) -> Self {
        match self {
            Self::NowPlaying => Self::Categories,
            Self::Categories => Self::Queue,
            Self::Queue => Self::MetadataEditor,
            Self::MetadataEditor => Self::AudioOutput,
            Self::AudioOutput => Self::Settings,
            Self::Settings => Self::NowPlaying,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::NowPlaying => Self::Settings,
            Self::Categories => Self::NowPlaying,
            Self::Queue => Self::Categories,
            Self::MetadataEditor => Self::Queue,
            Self::AudioOutput => Self::MetadataEditor,
            Self::Settings => Self::AudioOutput,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::NowPlaying,
            1 => Self::Categories,
            2 => Self::Queue,
            3 => Self::MetadataEditor,
            4 => Self::AudioOutput,
            5 => Self::Settings,
            _ => Self::NowPlaying,
        }
    }
}

/// Where keyboard focus sits inside the active tab. Tab/BackTab move along a
/// per-tab ring ([`Focus::next_in_tab`]/[`Focus::prev_in_tab`]); number keys
/// act on the focused area (tab bar digits switch main tabs, category
/// sub-tab digits switch the category type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// The main tab bar in the header (digits 1-6 switch tabs).
    TabBar,
    // --- Now Playing ---
    NowTracklist,
    NowLyrics,
    // --- Categories ---
    /// The sub-tab row: [1] Album Artist [2] Song [3] Artists [4] Album [5] Playlist
    CategoryTabs,
    /// The category item list (left column).
    CategoryList,
    /// The "Tracks in '<category>'" list (right column).
    CategoryTracks,
    // --- Queue ---
    QueueList,
    // --- Settings ---
    SettingsList,
}

impl Focus {
    /// Next focusable area within the given tab's ring (Tab key).
    pub fn next_in_tab(&self, tab: AppTab) -> Self {
        use AppTab as T;
        match tab {
            T::NowPlaying => match self {
                Self::NowTracklist => Self::NowLyrics,
                Self::NowLyrics => Self::TabBar,
                _ => Self::NowTracklist,
            },
            T::Categories => match self {
                Self::TabBar => Self::CategoryTabs,
                Self::CategoryTabs => Self::CategoryList,
                Self::CategoryList => Self::CategoryTracks,
                _ => Self::TabBar,
            },
            T::Queue => match self {
                Self::QueueList => Self::TabBar,
                _ => Self::QueueList,
            },
            T::AudioOutput | T::MetadataEditor => Self::TabBar,
            T::Settings => match self {
                Self::SettingsList => Self::TabBar,
                _ => Self::SettingsList,
            },
        }
    }

    /// Previous focusable area within the given tab's ring (Shift+Tab).
    pub fn prev_in_tab(&self, tab: AppTab) -> Self {
        use AppTab as T;
        match tab {
            T::NowPlaying => match self {
                Self::NowTracklist => Self::TabBar,
                Self::NowLyrics => Self::NowTracklist,
                _ => Self::NowLyrics,
            },
            T::Categories => match self {
                Self::TabBar => Self::CategoryTracks,
                Self::CategoryTabs => Self::TabBar,
                Self::CategoryList => Self::CategoryTabs,
                _ => Self::CategoryList,
            },
            T::Queue => match self {
                Self::QueueList => Self::TabBar,
                _ => Self::QueueList,
            },
            T::AudioOutput | T::MetadataEditor => Self::TabBar,
            T::Settings => match self {
                Self::SettingsList => Self::TabBar,
                _ => Self::SettingsList,
            },
        }
    }

    /// Focus used when a tab becomes active.
    pub fn default_for_tab(tab: AppTab) -> Self {
        use AppTab as T;
        match tab {
            T::NowPlaying => Self::NowTracklist,
            T::Categories => Self::TabBar,
            T::Queue => Self::QueueList,
            T::MetadataEditor => Self::TabBar,
            T::AudioOutput => Self::TabBar,
            T::Settings => Self::SettingsList,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(codec: &str, sample_rate: Option<u32>, bit_depth: Option<u8>) -> Track {
        Track {
            id: format!("t-{codec}-{sample_rate:?}-{bit_depth:?}"),
            path: PathBuf::from("/tmp/x.m4a"),
            title: "T".into(),
            artist: "A".into(),
            album: "AL".into(),
            album_artist: "AA".into(),
            genre: "G".into(),
            year: None,
            track_number: None,
            duration_secs: 200,
            codec: codec.into(),
            bitrate: "1000 kbps".into(),
            sample_rate,
            bit_depth,
            channels: Some(2),
            cover_art: None,
            lyrics: Vec::new(),
        }
    }

    #[test]
    fn khz_trims_trailing_zeros() {
        assert_eq!(
            track("ALAC", Some(44100), Some(16)).sample_rate_khz().unwrap(),
            "44.1"
        );
        assert_eq!(
            track("ALAC", Some(48000), Some(16)).sample_rate_khz().unwrap(),
            "48"
        );
        assert_eq!(
            track("ALAC", Some(96000), Some(24)).sample_rate_khz().unwrap(),
            "96"
        );
        assert_eq!(
            track("ALAC", Some(192000), Some(24)).sample_rate_khz().unwrap(),
            "192"
        );
        assert_eq!(
            track("ALAC", Some(22050), Some(16)).sample_rate_khz().unwrap(),
            "22.05"
        );
        assert!(track("MP3", None, None).sample_rate_khz().is_none());
    }

    #[test]
    fn depth_rate_combinations() {
        assert_eq!(
            track("ALAC", Some(44100), Some(16)).depth_rate_str(),
            "16-bit/44.1 kHz"
        );
        assert_eq!(
            track("ALAC", Some(96000), Some(24)).depth_rate_str(),
            "24-bit/96 kHz"
        );
        assert_eq!(track("ALAC", None, Some(24)).depth_rate_str(), "24-bit");
        assert_eq!(track("ALAC", Some(44100), None).depth_rate_str(), "44.1 kHz");
        assert_eq!(track("MP3", None, None).depth_rate_str(), "");
    }

    #[test]
    fn lossless_codec_set() {
        for codec in ["ALAC", "FLAC", "WAV", "PCM", "APE", "AIFF", "WavPack"] {
            assert!(track(codec, Some(44100), Some(16)).is_lossless(), "{codec}");
        }
        for codec in ["MP3", "AAC", "OGG", "Opus", "Vorbis", "WMA", "M4A"] {
            assert!(!track(codec, Some(44100), Some(16)).is_lossless(), "{codec}");
        }
    }

    #[test]
    fn quality_tiers() {
        assert_eq!(track("ALAC", Some(44100), Some(16)).quality_tier(), QualityTier::Lossless);
        assert_eq!(track("ALAC", Some(48000), Some(16)).quality_tier(), QualityTier::Lossless);
        // 24-bit at any sample rate is Hi-Res per user decision.
        assert_eq!(track("ALAC", Some(44100), Some(24)).quality_tier(), QualityTier::HiResLossless);
        assert_eq!(track("ALAC", Some(48000), Some(24)).quality_tier(), QualityTier::HiResLossless);
        assert_eq!(track("FLAC", Some(96000), Some(16)).quality_tier(), QualityTier::HiResLossless);
        assert_eq!(track("FLAC", Some(192000), Some(24)).quality_tier(), QualityTier::HiResLossless);
        assert_eq!(track("MP3", Some(44100), Some(16)).quality_tier(), QualityTier::Lossy);
        assert_eq!(track("AAC", Some(48000), None).quality_tier(), QualityTier::Lossy);
    }

    #[test]
    fn quality_badges() {
        assert_eq!(QualityTier::HiResLossless.badge(), "∿∿∿ Hi-Res Lossless");
        assert_eq!(QualityTier::Lossless.badge(), "∿∿∿ Lossless");
        assert_eq!(QualityTier::Lossy.badge(), "LOSSY");
    }

    #[test]
    fn now_playing_focus_ring() {
        // Tab bar -> tracklist -> lyrics -> back to tab bar.
        assert_eq!(
            Focus::NowTracklist.next_in_tab(AppTab::NowPlaying),
            Focus::NowLyrics
        );
        assert_eq!(Focus::NowLyrics.next_in_tab(AppTab::NowPlaying), Focus::TabBar);
        assert_eq!(Focus::TabBar.next_in_tab(AppTab::NowPlaying), Focus::NowTracklist);
        // Shift+Tab walks the ring the other way.
        assert_eq!(Focus::TabBar.prev_in_tab(AppTab::NowPlaying), Focus::NowLyrics);
        assert_eq!(
            Focus::NowLyrics.prev_in_tab(AppTab::NowPlaying),
            Focus::NowTracklist
        );
        assert_eq!(
            Focus::NowTracklist.prev_in_tab(AppTab::NowPlaying),
            Focus::TabBar
        );
    }

    #[test]
    fn categories_focus_ring() {
        // Tab bar -> sub-tabs -> category list -> tracklist -> wrap.
        assert_eq!(Focus::TabBar.next_in_tab(AppTab::Categories), Focus::CategoryTabs);
        assert_eq!(
            Focus::CategoryTabs.next_in_tab(AppTab::Categories),
            Focus::CategoryList
        );
        assert_eq!(
            Focus::CategoryList.next_in_tab(AppTab::Categories),
            Focus::CategoryTracks
        );
        assert_eq!(
            Focus::CategoryTracks.next_in_tab(AppTab::Categories),
            Focus::TabBar
        );
        // And backwards.
        assert_eq!(Focus::TabBar.prev_in_tab(AppTab::Categories), Focus::CategoryTracks);
        assert_eq!(
            Focus::CategoryList.prev_in_tab(AppTab::Categories),
            Focus::CategoryTabs
        );
    }

    #[test]
    fn default_focus_per_tab() {
        assert_eq!(Focus::default_for_tab(AppTab::NowPlaying), Focus::NowTracklist);
        assert_eq!(Focus::default_for_tab(AppTab::Categories), Focus::TabBar);
        assert_eq!(Focus::default_for_tab(AppTab::Queue), Focus::QueueList);
        assert_eq!(Focus::default_for_tab(AppTab::AudioOutput), Focus::TabBar);
        assert_eq!(Focus::default_for_tab(AppTab::Settings), Focus::SettingsList);
    }

    #[test]
    fn app_tab_settings_in_cycle() {
        assert_eq!(AppTab::AudioOutput.next(), AppTab::Settings);
        assert_eq!(AppTab::Settings.next(), AppTab::NowPlaying);
        assert_eq!(AppTab::Settings.prev(), AppTab::AudioOutput);
        assert_eq!(AppTab::from_index(5), AppTab::Settings);
        assert_eq!(AppTab::from_index(6), AppTab::NowPlaying);
    }

    #[test]
    fn category_type_numbering() {
        // Sub-tab numbering: 1 Album Artist, 2 Song, 3 Artists, 4 Album, 5 Playlist.
        let all = CategoryType::all();
        assert_eq!(
            all,
            [
                CategoryType::AlbumArtist,
                CategoryType::Song,
                CategoryType::Artist,
                CategoryType::Album,
                CategoryType::Playlist
            ]
        );
        assert_eq!(all[0].title(), "Album Artist");
        assert_eq!(all[1].title(), "Song");
        assert_eq!(all[2].title(), "Artists");
        assert_eq!(all[3].title(), "Album");
        assert_eq!(all[4].title(), "Playlist");
        // next()/prev() follow the same ring.
        assert_eq!(CategoryType::AlbumArtist.next(), CategoryType::Song);
        assert_eq!(CategoryType::Playlist.next(), CategoryType::AlbumArtist);
        assert_eq!(CategoryType::Song.prev(), CategoryType::AlbumArtist);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategoryType {
    Artist = 0,
    Song = 1,
    Playlist = 2,
    Album = 3,
    AlbumArtist = 4,
}

impl CategoryType {
    /// Sub-tab order: [1] Album Artist [2] Song [3] Artists [4] Album [5] Playlist.
    pub fn all() -> [Self; 5] {
        [Self::AlbumArtist, Self::Song, Self::Artist, Self::Album, Self::Playlist]
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::AlbumArtist => "Album Artist",
            Self::Song => "Song",
            Self::Artist => "Artists",
            Self::Album => "Album",
            Self::Playlist => "Playlist",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::AlbumArtist => Self::Song,
            Self::Song => Self::Artist,
            Self::Artist => Self::Album,
            Self::Album => Self::Playlist,
            Self::Playlist => Self::AlbumArtist,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::AlbumArtist => Self::Playlist,
            Self::Song => Self::AlbumArtist,
            Self::Artist => Self::Song,
            Self::Album => Self::Artist,
            Self::Playlist => Self::Album,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub year: Option<i32>,
    pub track_number: Option<u32>,
    pub duration_secs: u64,
    pub codec: String,
    pub bitrate: String,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<u8>,
    #[serde(default)]
    pub channels: Option<u8>,
    #[serde(skip)]
    pub cover_art: Option<Vec<u8>>,
    #[serde(skip)]
    pub lyrics: Vec<LyricsLine>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QualityTier {
    HiResLossless,
    Lossless,
    Lossy,
}

impl QualityTier {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::HiResLossless => "∿∿∿ Hi-Res Lossless",
            Self::Lossless => "∿∿∿ Lossless",
            Self::Lossy => "LOSSY",
        }
    }
}

const LOSSLESS_CODECS: &[&str] = &[
    "ALAC", "FLAC", "WAV", "PCM", "APE", "AIFF", "WAVPACK",
];

/// Codec labels considered lossless (used by media_probe and the UI).
pub fn lossless_codec_names() -> &'static [&'static str] {
    LOSSLESS_CODECS
}

impl Track {
    pub fn duration_formatted(&self) -> String {
        let mins = self.duration_secs / 60;
        let secs = self.duration_secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    /// Sample rate in kHz, trailing zeros trimmed: 44100 -> "44.1", 48000 -> "48"
    pub fn sample_rate_khz(&self) -> Option<String> {
        let khz = self.sample_rate? as f64 / 1000.0;
        let mut s = format!("{:.3}", khz);
        if s.contains('.') {
            while s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.pop();
            }
        }
        Some(s)
    }

    /// "24-bit/96 kHz", "24-bit", "44.1 kHz", or "" when nothing is known
    pub fn depth_rate_str(&self) -> String {
        let depth = self.bit_depth.map(|d| format!("{}-bit", d));
        let rate = self.sample_rate_khz().map(|k| format!("{} kHz", k));
        match (depth, rate) {
            (Some(d), Some(r)) => format!("{}/{}", d, r),
            (Some(d), None) => d,
            (None, Some(r)) => r,
            (None, None) => String::new(),
        }
    }

    pub fn is_lossless(&self) -> bool {
        LOSSLESS_CODECS.contains(&self.codec.to_uppercase().as_str())
    }

    pub fn quality_tier(&self) -> QualityTier {
        if !self.is_lossless() {
            return QualityTier::Lossy;
        }
        let hires = self.bit_depth.is_some_and(|d| d >= 24)
            || self.sample_rate.is_some_and(|sr| sr > 48_000);
        if hires {
            QualityTier::HiResLossless
        } else {
            QualityTier::Lossless
        }
    }

    pub fn artist_display(&self) -> &str {
        if self.artist.trim().is_empty() {
            "Unknown Artist"
        } else {
            &self.artist
        }
    }

    pub fn album_display(&self) -> &str {
        if self.album.trim().is_empty() {
            "Unknown Album"
        } else {
            &self.album
        }
    }

    pub fn album_artist_display(&self) -> &str {
        if self.album_artist.trim().is_empty() {
            if self.artist.trim().is_empty() {
                "Unknown Artist"
            } else {
                &self.artist
            }
        } else {
            &self.album_artist
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsLine {
    pub time_secs: f64,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatMode {
    None,
    Track,
    All,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match self {
            Self::None => Self::All,
            Self::All => Self::Track,
            Self::Track => Self::None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::None => "Off",
            Self::All => "All",
            Self::Track => "One",
        }
    }
}
