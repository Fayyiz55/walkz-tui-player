use std::path::PathBuf;
use lofty::config::WriteOptions;
use lofty::prelude::*;
use crate::models::Track;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TagEditorField {
    pub label: &'static str,
    pub key: &'static str,
    pub value: String,
}

pub struct TagEditorState {
    pub track_index: Option<usize>,
    pub track_path: Option<PathBuf>,
    pub fields: Vec<TagEditorField>,
    pub focused_field: usize,
    pub status_message: Option<(String, bool)>, // (message, is_success)
    pub is_editing: bool,
}

impl TagEditorState {
    pub fn new() -> Self {
        let fields = vec![
            TagEditorField { label: "Title", key: "title", value: String::new() },
            TagEditorField { label: "Artist", key: "artist", value: String::new() },
            TagEditorField { label: "Album", key: "album", value: String::new() },
            TagEditorField { label: "Album Artist", key: "album_artist", value: String::new() },
            TagEditorField { label: "Track Number", key: "track_number", value: String::new() },
            TagEditorField { label: "Year", key: "year", value: String::new() },
            TagEditorField { label: "Genre", key: "genre", value: String::new() },
        ];

        Self {
            track_index: None,
            track_path: None,
            fields,
            focused_field: 0,
            status_message: None,
            is_editing: false,
        }
    }

    /// Load a track into the editor fields
    pub fn load_track(&mut self, track_idx: usize, track: &Track) {
        self.track_index = Some(track_idx);
        self.track_path = Some(track.path.clone());
        self.focused_field = 0;
        self.is_editing = false;
        self.status_message = None;

        self.fields[0].value = track.title.clone();
        self.fields[1].value = track.artist.clone();
        self.fields[2].value = track.album.clone();
        self.fields[3].value = track.album_artist.clone();
        self.fields[4].value = track.track_number.map(|n| n.to_string()).unwrap_or_default();
        self.fields[5].value = track.year.map(|y| y.to_string()).unwrap_or_default();
        self.fields[6].value = track.genre.clone();
    }

    pub fn next_field(&mut self) {
        if self.focused_field + 1 < self.fields.len() {
            self.focused_field += 1;
        } else {
            self.focused_field = 0;
        }
    }

    pub fn prev_field(&mut self) {
        if self.focused_field > 0 {
            self.focused_field -= 1;
        } else {
            self.focused_field = self.fields.len().saturating_sub(1);
        }
    }

    pub fn handle_char(&mut self, c: char) {
        if let Some(field) = self.fields.get_mut(self.focused_field) {
            field.value.push(c);
        }
    }

    pub fn handle_backspace(&mut self) {
        if let Some(field) = self.fields.get_mut(self.focused_field) {
            field.value.pop();
        }
    }

    pub fn clear_current_field(&mut self) {
        if let Some(field) = self.fields.get_mut(self.focused_field) {
            field.value.clear();
        }
    }

    /// Save the edited metadata tags to the audio file on disk
    pub fn save_to_disk(&mut self) -> Result<Track, String> {
        let Some(path) = &self.track_path else {
            return Err("No track selected to save".to_string());
        };

        if path.to_string_lossy().starts_with("demo://") {
            let title = self.fields[0].value.clone();
            let artist = self.fields[1].value.clone();
            let album = self.fields[2].value.clone();
            let album_artist = self.fields[3].value.clone();
            let track_number = self.fields[4].value.parse::<u32>().ok();
            let year = self.fields[5].value.parse::<i32>().ok();
            let genre = self.fields[6].value.clone();

            self.status_message = Some(("Demo track updated in-memory successfully".into(), true));

            return Ok(Track {
                id: path.to_string_lossy().to_string(),
                path: path.clone(),
                title,
                artist,
                album,
                album_artist,
                genre,
                year,
                track_number,
                duration_secs: 180,
                codec: "DEMO".to_string(),
                bitrate: "320 kbps".to_string(),
                sample_rate: None,
                bit_depth: None,
                channels: None,
                cover_art: None,
                lyrics: Vec::new(),
            });
        }

        if !path.exists() {
            return Err(format!("File does not exist: {}", path.display()));
        }

        let title = self.fields[0].value.trim().to_string();
        let artist = self.fields[1].value.trim().to_string();
        let album = self.fields[2].value.trim().to_string();
        let album_artist = self.fields[3].value.trim().to_string();
        let track_number = self.fields[4].value.trim().parse::<u32>().ok();
        let year = self.fields[5].value.trim().parse::<i32>().ok();
        let genre = self.fields[6].value.trim().to_string();

        // Write tags with lofty: the file keeps its native tag format
        // (ID3v2 for mp3, MP4 `ilst` for m4a, Vorbis comments for flac/ogg),
        // so edits on ALAC .m4a files are actually readable afterwards.
        match write_tags_with_lofty(
            path,
            &title,
            &artist,
            &album,
            &album_artist,
            track_number,
            year,
            &genre,
        ) {
            Ok(_) => {
                let msg = format!("✓ Saved tags to {}", path.file_name().and_then(|f| f.to_str()).unwrap_or("file"));
                self.status_message = Some((msg, true));

                Ok(Track {
                    id: path.to_string_lossy().to_string(),
                    path: path.clone(),
                    title,
                    artist,
                    album,
                    album_artist,
                    genre,
                    year,
                    track_number,
                    duration_secs: 180,
                    codec: path.extension().and_then(|e| e.to_str()).unwrap_or("MP3").to_uppercase(),
                    bitrate: "320 kbps".to_string(),
                    sample_rate: None,
                    bit_depth: None,
                    channels: None,
                    cover_art: None,
                    lyrics: Vec::new(),
                })
            }
            Err(e) => {
                let err_msg = format!("Failed to write tags: {}", e);
                self.status_message = Some((err_msg.clone(), false));
                Err(err_msg)
            }
        }
    }
}

/// Set `key` to `value`, removing the item when `value` is blank.
fn set_or_remove(tag: &mut lofty::tag::Tag, key: ItemKey, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        tag.remove_key(key);
    } else {
        tag.insert_text(key, value.to_string());
    }
}

/// Write the edited fields into the file's native tag format and save.
fn write_tags_with_lofty(
    path: &std::path::Path,
    title: &str,
    artist: &str,
    album: &str,
    album_artist: &str,
    track_number: Option<u32>,
    year: Option<i32>,
    genre: &str,
) -> Result<(), String> {
    let mut tagged = lofty::read_from_path(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    // Edit the file's primary tag, creating it when the file has none.
    let tag_type = tagged.primary_tag_type();
    if tagged.tag_mut(tag_type).is_none() {
        tagged.insert_tag(lofty::tag::Tag::new(tag_type));
    }
    let Some(tag) = tagged.tag_mut(tag_type) else {
        return Err("This file format does not accept tags".to_string());
    };

    set_or_remove(tag, ItemKey::TrackTitle, title);
    set_or_remove(tag, ItemKey::TrackArtist, artist);
    set_or_remove(tag, ItemKey::AlbumTitle, album);
    set_or_remove(tag, ItemKey::AlbumArtist, album_artist);
    set_or_remove(tag, ItemKey::Genre, genre);
    match track_number {
        Some(n) => {
            tag.insert_text(ItemKey::TrackNumber, n.to_string());
        }
        None => tag.remove_key(ItemKey::TrackNumber),
    }
    match year {
        Some(y) => {
            tag.insert_text(ItemKey::RecordingDate, y.to_string());
        }
        None => tag.remove_key(ItemKey::RecordingDate),
    }

    tagged
        .save_to_path(path, WriteOptions::default())
        .map_err(|e| format!("{}: {}", path.display(), e))
}
