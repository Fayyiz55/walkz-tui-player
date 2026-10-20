//! Persistent user playlists + favourites.
//!
//! Stored under the app config dir (default `~/.config/walkz-tui/`):
//! - `playlists.json`:  [{ name, description, track_paths: [...] }]
//! - `favourites.json`: [ "<track path>", ... ]

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserPlaylist {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub track_paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UserData {
    #[serde(default)]
    pub playlists: Vec<UserPlaylist>,
    #[serde(default)]
    pub favourites: Vec<String>,
}

impl UserData {
    /// Load playlists.json + favourites.json from `dir`.
    /// Missing or corrupt files yield empty collections.
    pub fn load(dir: &Path) -> Self {
        let playlists = fs::read_to_string(dir.join("playlists.json"))
            .ok()
            .and_then(|c| serde_json::from_str::<Vec<UserPlaylist>>(&c).ok())
            .unwrap_or_default();
        let favourites = fs::read_to_string(dir.join("favourites.json"))
            .ok()
            .and_then(|c| serde_json::from_str::<Vec<String>>(&c).ok())
            .unwrap_or_default();

        UserData {
            playlists,
            favourites,
        }
    }

    /// Persist both files into `dir` (created on demand).
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;
        let playlists_json = serde_json::to_string_pretty(&self.playlists)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let favourites_json = serde_json::to_string_pretty(&self.favourites)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        write_json_atomic(&dir.join("playlists.json"), &playlists_json)?;
        write_json_atomic(&dir.join("favourites.json"), &favourites_json)
    }

    /// Create a playlist; rejects empty or duplicate names.
    pub fn create_playlist(&mut self, name: &str, description: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Playlist name cannot be empty".to_string());
        }
        if self.playlists.iter().any(|p| p.name.eq_ignore_ascii_case(name)) {
            return Err(format!("Playlist '{}' already exists", name));
        }
        self.playlists.push(UserPlaylist {
            name: name.to_string(),
            description: description.trim().to_string(),
            track_paths: Vec::new(),
        });
        Ok(())
    }

    /// Append a track path to a playlist (no-op if playlist missing).
    /// Returns true when the path was newly added.
    pub fn add_track(&mut self, playlist: &str, path: &str) -> bool {
        let Some(pl) = self
            .playlists
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(playlist))
        else {
            return false;
        };
        if pl.track_paths.iter().any(|p| p == path) {
            return false;
        }
        pl.track_paths.push(path.to_string());
        true
    }

    /// Toggle favourite; returns the new state (true = favourited).
    pub fn toggle_favourite(&mut self, path: &str) -> bool {
        if let Some(pos) = self.favourites.iter().position(|f| f == path) {
            self.favourites.remove(pos);
            false
        } else {
            self.favourites.push(path.to_string());
            true
        }
    }

    pub fn is_favourite(&self, path: &str) -> bool {
        self.favourites.iter().any(|f| f == path)
    }

    /// Store API for deleting a user playlist (persisted immediately).
    #[allow(dead_code)] // reserved for the future playlist-management UI
    pub fn remove_playlist(&mut self, name: &str) {
        self.playlists.retain(|p| p.name != name);
    }
}

fn write_json_atomic(path: &Path, json: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("walkz-playlist-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tmp_dir("roundtrip");
        let mut data = UserData::default();
        data.create_playlist("Late Night Drive", "synthwave for the highway").unwrap();
        data.add_track("Late Night Drive", "/music/song.flac");
        data.toggle_favourite("/music/song.flac");
        data.save(&dir).unwrap();

        let loaded = UserData::load(&dir);
        assert_eq!(loaded, data);
        assert!(loaded.is_favourite("/music/song.flac"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_corrupt_files_load_empty() {
        let dir = tmp_dir("corrupt");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("playlists.json"), "{{{not json").unwrap();

        let data = UserData::load(&dir);
        assert!(data.playlists.is_empty());
        assert!(data.favourites.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn create_rejects_empty_and_duplicate_names() {
        let mut data = UserData::default();
        assert!(data.create_playlist("", "nope").is_err());
        assert!(data.create_playlist("   ", "nope").is_err());
        data.create_playlist("Mix", "").unwrap();
        assert!(data.create_playlist("Mix", "dup").is_err());
        // whitespace is trimmed
        assert!(data.create_playlist("  Mix  ", "dup").is_err());
        assert_eq!(data.playlists[0].name, "Mix");
    }

    #[test]
    fn add_track_no_duplicates() {
        let mut data = UserData::default();
        data.create_playlist("Mix", "").unwrap();
        assert!(data.add_track("Mix", "/a.flac"));
        assert!(!data.add_track("Mix", "/a.flac")); // already there
        assert!(!data.add_track("Ghost", "/a.flac")); // playlist missing
        assert_eq!(data.playlists[0].track_paths, vec!["/a.flac"]);
    }

    #[test]
    fn toggle_favourite_flips_state() {
        let mut data = UserData::default();
        assert!(data.toggle_favourite("/a.flac"));
        assert!(data.is_favourite("/a.flac"));
        assert!(!data.toggle_favourite("/a.flac"));
        assert!(!data.is_favourite("/a.flac"));
    }

    #[test]
    fn remove_playlist_works() {
        let mut data = UserData::default();
        data.create_playlist("Old", "").unwrap();
        data.remove_playlist("Old");
        assert!(data.playlists.is_empty());
    }
}
