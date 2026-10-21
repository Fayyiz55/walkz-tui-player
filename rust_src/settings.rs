//! Persisted user settings (`settings.json` in the config dir).
//!
//! Everything defaults to the app's historical behavior so the file only
//! needs to exist when the user changes something. Hotkeys are stored as
//! `(action, char)` pairs — only rebindings are saved; unlisted actions keep
//! their [`DEFAULT_HOTKEYS`] value.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Actions triggerable by a global hotkey (rebindable in Settings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyAction {
    NextTrack,
    PrevTrack,
    PlayPause,
    VolumeUp,
    VolumeDown,
    ToggleShuffle,
    CycleRepeat,
    SongMenu,
    OpenFolder,
    Rescan,
}

#[allow(dead_code)]
impl HotkeyAction {
    pub fn all() -> [Self; 10] {
        [
            Self::NextTrack,
            Self::PrevTrack,
            Self::PlayPause,
            Self::VolumeUp,
            Self::VolumeDown,
            Self::ToggleShuffle,
            Self::CycleRepeat,
            Self::SongMenu,
            Self::OpenFolder,
            Self::Rescan,
        ]
    }

    /// Human label shown in the Settings hotkey rows.
    pub fn label(&self) -> &'static str {
        match self {
            Self::NextTrack => "Next track",
            Self::PrevTrack => "Previous track",
            Self::PlayPause => "Play / pause",
            Self::VolumeUp => "Volume up",
            Self::VolumeDown => "Volume down",
            Self::ToggleShuffle => "Toggle shuffle",
            Self::CycleRepeat => "Cycle repeat",
            Self::SongMenu => "Song menu",
            Self::OpenFolder => "Open folder",
            Self::Rescan => "Rescan library",
        }
    }
}

/// Default key for each action. `q` / Ctrl-C (quit) are fixed and not listed.
pub fn default_hotkey(action: HotkeyAction) -> char {
    match action {
        HotkeyAction::NextTrack => 'n',
        HotkeyAction::PrevTrack => 'p',
        HotkeyAction::PlayPause => ' ',
        HotkeyAction::VolumeUp => '+',
        HotkeyAction::VolumeDown => '-',
        HotkeyAction::ToggleShuffle => 's',
        HotkeyAction::CycleRepeat => 'r',
        HotkeyAction::SongMenu => 'a',
        HotkeyAction::OpenFolder => 'o',
        HotkeyAction::Rescan => 'S',
    }
}

/// Album art pane sizing: how much of the left column the bottom-left
/// playlist pane may claim (the rest goes to the album art, so a smaller
/// playlist cap means a larger, higher-resolution artwork).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtSize {
    Large,
    Medium,
    Small,
}

impl ArtSize {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Large => "Large",
            Self::Medium => "Medium",
            Self::Small => "Small",
        }
    }

    /// Maximum share (percent) of the left column the playlist pane may use.
    pub fn playlist_cap_percent(&self) -> u16 {
        match self {
            Self::Large => 30,
            Self::Medium => 45,
            Self::Small => 60,
        }
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Large => Self::Medium,
            Self::Medium => Self::Small,
            Self::Small => Self::Large,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Large => Self::Small,
            Self::Medium => Self::Large,
            Self::Small => Self::Medium,
        }
    }
}

/// One selectable row in the Settings tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRow {
    MusicFolder,
    StartupCache,
    DefaultVolume,
    RememberState,
    ShowLyrics,
    ArtSize,
    VolumeStep,
    SeekStep,
    Accent,
    Hotkey(HotkeyAction),
}

/// The settings list, shared by the app logic and the renderer.
pub fn settings_rows() -> Vec<SettingsRow> {
    let mut rows = vec![
        SettingsRow::MusicFolder,
        SettingsRow::StartupCache,
        SettingsRow::DefaultVolume,
        SettingsRow::RememberState,
        SettingsRow::ShowLyrics,
        SettingsRow::ArtSize,
        SettingsRow::VolumeStep,
        SettingsRow::SeekStep,
        SettingsRow::Accent,
    ];
    rows.extend(HotkeyAction::all().map(SettingsRow::Hotkey));
    rows
}

/// Accent color presets for the UI highlight color (Settings tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AccentColor {
    #[default]
    Cyan,
    Green,
    Pink,
    Amber,
    Violet,
}

#[allow(dead_code)]
impl AccentColor {
    pub const ALL: [AccentColor; 5] = [
        AccentColor::Cyan,
        AccentColor::Green,
        AccentColor::Pink,
        AccentColor::Amber,
        AccentColor::Violet,
    ];

    pub fn color(self) -> ratatui::style::Color {
        match self {
            AccentColor::Cyan => ratatui::style::Color::Cyan,
            AccentColor::Green => ratatui::style::Color::Green,
            AccentColor::Pink => ratatui::style::Color::Rgb(255, 120, 160),
            AccentColor::Amber => ratatui::style::Color::Rgb(255, 180, 70),
            AccentColor::Violet => ratatui::style::Color::Rgb(185, 130, 255),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AccentColor::Cyan => "Cyan",
            AccentColor::Green => "Green",
            AccentColor::Pink => "Pink",
            AccentColor::Amber => "Amber",
            AccentColor::Violet => "Violet",
        }
    }

    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&a| a == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ALL.iter().position(|&a| a == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

fn default_volume_step() -> u8 {
    5
}

fn default_seek_step_secs() -> u8 {
    5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Restore the library from `library_cache.json` at startup (off = scan).
    pub load_library_cache_on_launch: bool,
    /// Volume applied at launch (0-100).
    pub default_volume: u8,
    /// Persist repeat/shuffle across sessions.
    pub remember_repeat_shuffle: bool,
    /// Saved state, only honored when `remember_repeat_shuffle` is on.
    #[serde(default)]
    pub saved_repeat: Option<crate::models::RepeatMode>,
    #[serde(default)]
    pub saved_shuffle: Option<bool>,
    /// Show the synchronized lyrics pane on Now Playing.
    pub show_lyrics_pane: bool,
    /// Album art sizing preset.
    pub art_size: ArtSize,
    /// Volume change per volume keypress (percent, 1-25).
    #[serde(default = "default_volume_step")]
    pub volume_step: u8,
    /// Seek jump per ←/→ keypress (seconds, 1-60).
    #[serde(default = "default_seek_step_secs")]
    pub seek_step_secs: u8,
    /// Accent color used for highlights, focus borders and the active tab.
    #[serde(default)]
    pub accent: AccentColor,
    /// Only rebindings are stored; the rest use `default_hotkey`.
    #[serde(default)]
    pub hotkeys: Vec<(HotkeyAction, char)>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            load_library_cache_on_launch: true,
            default_volume: 85,
            remember_repeat_shuffle: false,
            saved_repeat: None,
            saved_shuffle: None,
            show_lyrics_pane: true,
            art_size: ArtSize::Medium,
            volume_step: default_volume_step(),
            seek_step_secs: default_seek_step_secs(),
            accent: AccentColor::default(),
            hotkeys: Vec::new(),
        }
    }
}

impl Settings {
    /// Load `settings.json`; missing or corrupt file → defaults.
    pub fn load_from(config_dir: &Path) -> Self {
        let path = config_dir.join("settings.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Atomically save to `settings.json`.
    pub fn save_to(&self, config_dir: &Path) {
        let path = config_dir.join("settings.json");
        let tmp = config_dir.join("settings.json.tmp");
        if let Ok(text) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }

    /// The active key for an action (rebinding or default).
    pub fn hotkey_char(&self, action: HotkeyAction) -> char {
        self.hotkeys
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, c)| *c)
            .unwrap_or_else(|| default_hotkey(action))
    }

    /// Reverse lookup: which action is bound to this exact char (case
    /// sensitive). Unbound chars → None.
    pub fn action_for_char(&self, c: char) -> Option<HotkeyAction> {
        HotkeyAction::all()
            .into_iter()
            .find(|&a| self.hotkey_char(a) == c)
    }

    /// Bind `c` to `action`. If `c` is already used by another action, the
    /// two swap keys so no action is ever orphaned.
    pub fn set_hotkey(&mut self, action: HotkeyAction, c: char) {
        let previous = self.hotkey_char(action);
        // Swap: the other action holding `c` takes `action`'s old key.
        for (a, key) in self.hotkeys.iter_mut() {
            if *key == c && *a != action {
                *key = previous;
            }
        }
        if let Some(slot) = self.hotkeys.iter_mut().find(|(a, _)| *a == action) {
            slot.1 = c;
        } else {
            self.hotkeys.push((action, c));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_config(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("walkz-settings-test-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_match_historical_behavior() {
        let s = Settings::default();
        assert!(s.load_library_cache_on_launch);
        assert_eq!(s.default_volume, 85);
        assert!(!s.remember_repeat_shuffle);
        assert!(s.show_lyrics_pane);
        assert_eq!(s.art_size, ArtSize::Medium);
        assert_eq!(s.volume_step, 5);
        assert_eq!(s.seek_step_secs, 5);
        assert_eq!(s.accent, AccentColor::Cyan);
        assert!(s.hotkeys.is_empty());
        // Defaults resolve to the historical keys.
        assert_eq!(s.hotkey_char(HotkeyAction::NextTrack), 'n');
        assert_eq!(s.hotkey_char(HotkeyAction::PlayPause), ' ');
        assert_eq!(s.hotkey_char(HotkeyAction::Rescan), 'S');
        assert_eq!(s.hotkey_char(HotkeyAction::VolumeUp), '+');
        assert_eq!(s.hotkey_char(HotkeyAction::OpenFolder), 'o');
    }

    #[test]
    fn roundtrip_through_disk() {
        let dir = tmp_config("roundtrip");
        let mut s = Settings::default();
        s.load_library_cache_on_launch = false;
        s.default_volume = 40;
        s.remember_repeat_shuffle = true;
        s.saved_repeat = Some(crate::models::RepeatMode::All);
        s.saved_shuffle = Some(true);
        s.show_lyrics_pane = false;
        s.art_size = ArtSize::Small;
        s.volume_step = 10;
        s.seek_step_secs = 15;
        s.accent = AccentColor::Pink;
        s.set_hotkey(HotkeyAction::NextTrack, 'x');
        s.save_to(&dir);

        let loaded = Settings::load_from(&dir);
        assert_eq!(loaded, s);
        assert_eq!(loaded.hotkey_char(HotkeyAction::NextTrack), 'x');
        assert_eq!(loaded.hotkey_char(HotkeyAction::PrevTrack), 'p');
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn old_settings_file_without_new_fields_loads() {
        // Settings written by an older build lack volume_step / seek_step /
        // accent; serde defaults must fill them in instead of discarding
        // the user's other choices.
        let dir = tmp_config("legacy");
        std::fs::write(
            dir.join("settings.json"),
            r#"{
                "load_library_cache_on_launch": false,
                "default_volume": 30,
                "remember_repeat_shuffle": true,
                "show_lyrics_pane": false,
                "art_size": "Small"
            }"#,
        )
        .unwrap();
        let s = Settings::load_from(&dir);
        assert!(!s.load_library_cache_on_launch);
        assert_eq!(s.default_volume, 30);
        assert!(s.remember_repeat_shuffle);
        assert!(!s.show_lyrics_pane);
        assert_eq!(s.art_size, ArtSize::Small);
        assert_eq!(s.volume_step, 5);
        assert_eq!(s.seek_step_secs, 5);
        assert_eq!(s.accent, AccentColor::Cyan);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn accent_color_ring() {
        assert_eq!(AccentColor::Cyan.next(), AccentColor::Green);
        assert_eq!(AccentColor::Violet.next(), AccentColor::Cyan);
        assert_eq!(AccentColor::Cyan.prev(), AccentColor::Violet);
        assert_eq!(AccentColor::Cyan.color(), ratatui::style::Color::Cyan);
        assert_eq!(AccentColor::Cyan.label(), "Cyan");
        assert_eq!(AccentColor::Green.label(), "Green");
        assert_eq!(AccentColor::Pink.label(), "Pink");
        assert_eq!(AccentColor::Amber.label(), "Amber");
        assert_eq!(AccentColor::Violet.label(), "Violet");
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tmp_config("corrupt");
        std::fs::write(dir.join("settings.json"), "{not json").unwrap();
        let s = Settings::load_from(&dir);
        assert_eq!(s, Settings::default());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rebind_and_conflict_swap() {
        let mut s = Settings::default();
        s.set_hotkey(HotkeyAction::NextTrack, 'x');
        assert_eq!(s.hotkey_char(HotkeyAction::NextTrack), 'x');
        // 'n' is now free; binding VolumeDown to 'n' works…
        s.set_hotkey(HotkeyAction::VolumeDown, 'n');
        assert_eq!(s.hotkey_char(HotkeyAction::VolumeDown), 'n');
        assert_eq!(s.hotkey_char(HotkeyAction::NextTrack), 'x');
        // …but binding NextTrack to 'n' swaps it back off VolumeDown.
        s.set_hotkey(HotkeyAction::NextTrack, 'n');
        assert_eq!(s.hotkey_char(HotkeyAction::NextTrack), 'n');
        assert_eq!(s.hotkey_char(HotkeyAction::VolumeDown), 'x');
        // Reverse lookup respects bindings and defaults.
        assert_eq!(s.action_for_char('n'), Some(HotkeyAction::NextTrack));
        assert_eq!(s.action_for_char('x'), Some(HotkeyAction::VolumeDown));
        assert_eq!(s.action_for_char('z'), None);
    }

    #[test]
    fn art_size_ring_and_caps() {
        assert_eq!(ArtSize::Large.next(), ArtSize::Medium);
        assert_eq!(ArtSize::Medium.next(), ArtSize::Small);
        assert_eq!(ArtSize::Small.next(), ArtSize::Large);
        assert_eq!(ArtSize::Large.prev(), ArtSize::Small);
        assert!(ArtSize::Large.playlist_cap_percent() < ArtSize::Medium.playlist_cap_percent());
        assert!(ArtSize::Medium.playlist_cap_percent() < ArtSize::Small.playlist_cap_percent());
    }

    #[test]
    fn rows_cover_every_hotkey() {
        let rows = settings_rows();
        let hotkey_rows: Vec<_> = rows
            .iter()
            .filter_map(|r| match r {
                SettingsRow::Hotkey(a) => Some(*a),
                _ => None,
            })
            .collect();
        assert_eq!(hotkey_rows.len(), HotkeyAction::all().len());
        assert_eq!(rows.first(), Some(&SettingsRow::MusicFolder));
        assert_eq!(rows.get(1), Some(&SettingsRow::StartupCache));
    }
}
