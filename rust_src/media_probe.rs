//! Media probing via ffprobe: real codec, bitrate, bit depth, sample rate.
//!
//! One `ffprobe` process per file (JSON output), replacing the old
//! duration-only probe. Falls back gracefully when ffprobe is unavailable.

use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MediaInfo {
    pub duration_secs: Option<f64>,
    pub codec: Option<String>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
    /// Bits per second of the (lossy) stream, when reported.
    pub bit_rate: Option<u64>,
    pub file_size: u64,
}

/// Map a raw ffprobe codec name to a friendly display label.
pub fn friendly_codec(name: &str) -> String {
    match name {
        "alac" => "ALAC".into(),
        "flac" => "FLAC".into(),
        "mp3" => "MP3".into(),
        "aac" | "aac_latm" => "AAC".into(),
        "opus" => "Opus".into(),
        "vorbis" => "Vorbis".into(),
        "wmav1" | "wmav2" | "wmalossless" | "wmapro" => "WMA".into(),
        "ape" => "APE".into(),
        "wavpack" => "WavPack".into(),
        n if n.starts_with("pcm_") => "PCM".into(),
        other => other.to_uppercase(),
    }
}

/// Sample-format fallback for bit depth when bits_per_raw_sample /
/// bits_per_sample are missing.
fn depth_from_sample_fmt(fmt: &str) -> Option<u8> {
    let base = fmt.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    // "16" | "24" | "32" | "64" | "8" style suffixes ("s16", "flt", ...)
    match fmt {
        "u8" | "u8p" => Some(8),
        "s16" | "s16p" => Some(16),
        "s32" | "s32p" => Some(32),
        "flt" | "fltp" => Some(32),
        "dbl" | "dblp" => Some(64),
        _ => base.parse::<u8>().ok().filter(|&d| (8..=64).contains(&d)),
    }
}

fn parse_u64(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f.round() as u64)),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
}

/// Parse ffprobe JSON output (as produced by `probe()`) plus the file size
/// into a [`MediaInfo`].
pub fn parse_ffprobe_json(json: &str, file_size: u64) -> Option<MediaInfo> {
    let v: Value = serde_json::from_str(json).ok()?;
    let stream = v.get("streams")?.as_array()?.first()?.clone();

    let codec = stream
        .get("codec_name")
        .and_then(|c| c.as_str())
        .map(friendly_codec);
    let sample_rate = stream
        .get("sample_rate")
        .and_then(parse_u64)
        .map(|sr| sr as u32)
        .filter(|&sr| sr > 0);
    let channels = stream
        .get("channels")
        .and_then(parse_u64)
        .map(|c| c as u8)
        .filter(|&c| c > 0);
    let bit_depth = stream
        .get("bits_per_raw_sample")
        .and_then(parse_u64)
        .filter(|&d| d > 0)
        .or_else(|| {
            stream
                .get("bits_per_sample")
                .and_then(parse_u64)
                .filter(|&d| d > 0)
        })
        .or_else(|| {
            stream
                .get("sample_fmt")
                .and_then(|f| f.as_str())
                .and_then(depth_from_sample_fmt)
                .map(|d| d as u64)
        })
        .map(|d| d as u8);
    let bit_rate = stream
        .get("bit_rate")
        .and_then(parse_u64)
        .or_else(|| v.get("format").and_then(|f| f.get("bit_rate")).and_then(parse_u64));
    let duration_secs = v
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .or_else(|| {
            v.get("format")
                .and_then(|f| f.get("duration"))
                .and_then(|d| d.as_f64())
        });

    Some(MediaInfo {
        duration_secs,
        codec,
        sample_rate,
        channels,
        bit_depth,
        bit_rate,
        file_size,
    })
}

/// Resolve the display bitrate (kbps) for a probed file:
/// - lossless codecs: effective rate from file size and duration (no fake caps),
/// - lossy codecs: the stream/format bit rate reported by ffprobe.
pub fn display_bit_rate_kbps(info: &MediaInfo) -> Option<u64> {
    let lossless = info
        .codec
        .as_deref()
        .map(|c| crate::models::lossless_codec_names().contains(&c))
        .unwrap_or(false);

    if lossless {
        if let Some(dur) = info.duration_secs {
            if dur > 0.5 && info.file_size > 0 {
                return Some(((info.file_size as f64) * 8.0 / dur / 1000.0).round() as u64);
            }
        }
        if let (Some(depth), Some(sr), Some(ch)) = (info.bit_depth, info.sample_rate, info.channels) {
            return Some((depth as u64 * sr as u64 * ch as u64) / 1000);
        }
        return info.bit_rate.map(|br| br / 1000);
    }

    info.bit_rate.map(|br| br / 1000)
}

/// Probe a media file with ffprobe. Returns None when ffprobe is missing,
/// the file is unreadable, or the output cannot be parsed.
pub fn probe(path: &Path) -> Option<MediaInfo> {
    let file_size = std::fs::metadata(path).ok()?.len();

    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("a:0")
        .arg("-show_entries")
        .arg("stream=codec_name,sample_rate,channels,bits_per_raw_sample,bits_per_sample,sample_fmt,bit_rate")
        .arg("-show_entries")
        .arg("format=duration,bit_rate")
        .arg("-of")
        .arg("json")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_ffprobe_json(&stdout, file_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Canned ffprobe output shapes observed for ALAC-in-M4A, MP3, FLAC.

    #[test]
    fn alac_m4a_24bit_without_stream_bitrate() {
        let json = r#"{
            "streams": [{
                "codec_name": "alac",
                "sample_rate": "44100",
                "channels": 2,
                "bits_per_raw_sample": "24",
                "bits_per_sample": 0,
                "bit_rate": "529200.00"
            }],
            "format": { "duration": "223.456000" }
        }"#;
        let info = parse_ffprobe_json(json, 1_476_704).unwrap();
        assert_eq!(info.codec.as_deref(), Some("ALAC"));
        assert_eq!(info.sample_rate, Some(44100));
        assert_eq!(info.channels, Some(2));
        assert_eq!(info.bit_depth, Some(24));
        assert_eq!(info.duration_secs, Some(223.456));
        // Lossless: display rate is the effective rate from the file itself.
        let br = display_bit_rate_kbps(&info).unwrap();
        let expected = ((1_476_704f64 * 8.0) / 223.456 / 1000.0).round() as u64;
        assert_eq!(br, expected);
        assert!(br > 0);
    }

    #[test]
    fn mp3_reports_stream_bitrate() {
        let json = r#"{
            "streams": [{
                "codec_name": "mp3",
                "sample_rate": "44100",
                "channels": 2,
                "bit_rate": "320000"
            }],
            "format": { "duration": "200.0", "bit_rate": "320000" }
        }"#;
        let info = parse_ffprobe_json(json, 8_000_000).unwrap();
        assert_eq!(info.codec.as_deref(), Some("MP3"));
        assert_eq!(info.bit_rate, Some(320_000));
        // Lossy: display rate is the reported stream rate.
        assert_eq!(display_bit_rate_kbps(&info), Some(320));
    }

    #[test]
    fn flac_16bit_via_bits_per_sample() {
        let json = r#"{
            "streams": [{
                "codec_name": "flac",
                "sample_rate": "44100",
                "channels": 2,
                "bits_per_raw_sample": "16"
            }],
            "format": { "duration": "180.0" }
        }"#;
        let info = parse_ffprobe_json(json, 15_000_000).unwrap();
        assert_eq!(info.codec.as_deref(), Some("FLAC"));
        assert_eq!(info.bit_depth, Some(16));
    }

    #[test]
    fn depth_falls_back_to_sample_fmt() {
        let json = r#"{
            "streams": [{ "codec_name": "pcm_s16le", "sample_rate": "48000", "channels": 2, "sample_fmt": "s16" }],
            "format": { "duration": "10.0" }
        }"#;
        let info = parse_ffprobe_json(json, 400_000).unwrap();
        assert_eq!(info.codec.as_deref(), Some("PCM"));
        assert_eq!(info.bit_depth, Some(16));
        assert_eq!(info.sample_rate, Some(48000));
    }

    #[test]
    fn garbage_json_is_none() {
        assert!(parse_ffprobe_json("not json at all", 0).is_none());
        assert!(parse_ffprobe_json("{}", 0).is_none());
    }

    #[test]
    fn friendly_codec_mapping() {
        assert_eq!(friendly_codec("alac"), "ALAC");
        assert_eq!(friendly_codec("pcm_s24le"), "PCM");
        assert_eq!(friendly_codec("wmav2"), "WMA");
        assert_eq!(friendly_codec("exotic"), "EXOTIC");
    }

    #[test]
    fn live_probe_on_real_mp3() {
        let p = Path::new("test_music/Test Artist/Test Album/01 - Demo Tone.mp3");
        if !p.exists() || Command::new("ffprobe").arg("-version").output().is_err() {
            return; // environment lacks fixture or ffprobe
        }
        let info = probe(p).expect("probe should succeed");
        assert_eq!(info.codec.as_deref(), Some("MP3"));
        // Fixture-dependent specifics (length/mono/stereo) vary; assert sane basics.
        assert!(info.duration_secs.unwrap_or(0.0) > 0.0);
        assert!(info.sample_rate.unwrap_or(0) > 0);
        assert!(info.bit_rate.unwrap_or(0) > 0);
    }
}
