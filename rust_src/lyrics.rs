use std::fs;
use std::path::Path;
use crate::models::LyricsLine;

pub struct LyricsProvider;

impl LyricsProvider {
    /// Load lyrics for a given audio file path
    pub fn load_for_track(track_path: &Path, title: &str, artist: &str, duration_secs: u64) -> Vec<LyricsLine> {
        Self::load_for_track_with_tag(track_path, title, artist, duration_secs, None)
    }

    /// Same as [`load_for_track`], but takes the embedded lyrics text the
    /// scanner already extracted from the file's tags (a second full file
    /// read would cost a round trip per track on network/WSL mounts).
    pub fn load_for_track_with_tag(
        track_path: &Path,
        title: &str,
        artist: &str,
        duration_secs: u64,
        embedded: Option<&str>,
    ) -> Vec<LyricsLine> {
        // 1. Try sidecar .lrc file in the same directory:
        //    e.g., song.mp3 -> song.lrc
        let lrc_path = track_path.with_extension("lrc");
        if lrc_path.exists() {
            if let Ok(content) = fs::read_to_string(&lrc_path) {
                let parsed = Self::parse_lrc(&content);
                if !parsed.is_empty() {
                    return parsed;
                }
            }
        }

        // 2. Try the embedded lyrics tag (USLT frame, `©lyr` atom, `LYRICS`
        //    comment — the caller extracts it from whichever format applies)
        if let Some(content) = embedded {
            let parsed = Self::parse_lrc(content);
            if !parsed.is_empty() {
                return parsed;
            }
        }

        // 3. Fallback: Generate aesthetic timed lyrics based on song details
        Self::generate_fallback_lyrics(title, artist, duration_secs)
    }

    /// Parse LRC format lines: [01:23.45] Some lyrics text
    pub fn parse_lrc(content: &str) -> Vec<LyricsLine> {
        let mut lines = Vec::new();

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if !line.starts_with('[') {
                continue;
            }

            // Find closing bracket
            if let Some(close_idx) = line.find(']') {
                let time_str = &line[1..close_idx];
                let text = line[close_idx + 1..].trim().to_string();

                if let Some(secs) = Self::parse_timestamp(time_str) {
                    lines.push(LyricsLine {
                        time_secs: secs,
                        text,
                    });
                }
            }
        }

        lines.sort_by(|a, b| a.time_secs.partial_cmp(&b.time_secs).unwrap_or(std::cmp::Ordering::Equal));
        lines
    }

    fn parse_timestamp(s: &str) -> Option<f64> {
        // Expected format mm:ss or mm:ss.xx or mm:ss:xx
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() < 2 {
            return None;
        }

        let mins: f64 = parts[0].parse().ok()?;
        let secs: f64 = parts[1].replace(',', ".").parse().ok()?;

        Some(mins * 60.0 + secs)
    }

    fn generate_fallback_lyrics(title: &str, artist: &str, duration_secs: u64) -> Vec<LyricsLine> {
        let dur = duration_secs.max(120) as f64;
        let step = dur / 8.0;

        vec![
            LyricsLine { time_secs: 0.0, text: format!("♫ [Instrumental Intro - {}]", title) },
            LyricsLine { time_secs: step * 1.0, text: format!("Echoes in the neon glow, {}", artist) },
            LyricsLine { time_secs: step * 2.0, text: "Feel the rhythm of the city lights below".into() },
            LyricsLine { time_secs: step * 3.0, text: format!("Every frequency aligns with {}", title) },
            LyricsLine { time_secs: step * 4.0, text: "Lost in the harmonics, drifting through time".into() },
            LyricsLine { time_secs: step * 5.0, text: "Crank the volume, let the bassline climb".into() },
            LyricsLine { time_secs: step * 6.0, text: "Golden synth waves washing over the night".into() },
            LyricsLine { time_secs: step * 7.0, text: format!("♫ [Outro Fade - {}]", artist) },
        ]
    }

    /// Find the index of the current active lyric line for a playback timestamp
    pub fn current_line_index(lines: &[LyricsLine], current_pos_secs: f64) -> usize {
        if lines.is_empty() {
            return 0;
        }

        let mut active_idx = 0;
        for (i, line) in lines.iter().enumerate() {
            if current_pos_secs >= line.time_secs {
                active_idx = i;
            } else {
                break;
            }
        }
        active_idx
    }
}
