//! cava-backed spectrum visualizer.
//!
//! Spawns a `cava` child process that reads the system's default sink
//! monitor (input `method = pipewire|pulse`, `source = auto` — the monitor
//! of the default output, so the ffplay playback engine is untouched) and
//! streams bar heights through cava's raw ascii output: values `0..100`,
//! `;`-delimited, one frame per line on stdout.
//!
//! A reader thread parses frames into an mpsc channel; the app drains the
//! latest frame each tick (`App::poll_visualizer`) and the Now Playing tab
//! renders it as a spectrum lane above the transport bar. If `cava` is not
//! installed the caller shows an install hint and the feature quietly stays
//! off.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};

/// Bars streamed by cava (sampled across the terminal width at render time).
const CAVA_BARS: u32 = 48;
/// cava frames per second — the UI ticks at 20 Hz; 30 keeps the motion smooth.
const CAVA_FRAMERATE: u32 = 30;
/// `ascii_max_range`: bar heights arrive as 0..100.
const ASCII_MAX_RANGE: u32 = 100;
/// Input backends tried in order (Ubuntu/Debian: PipeWire drives the sound
/// graph, `pulse` is its widely-available compatibility fallback).
pub const INPUT_METHODS: [&str; 2] = ["pipewire", "pulse"];

#[derive(Debug)]
pub enum SpawnError {
    /// `cava` not found on PATH (Settings shows an install hint).
    Missing,
    Io(std::io::Error),
}

/// Running cava child + its frame reader thread.
pub struct VizWorker {
    child: Option<Child>,
    rx: Receiver<Vec<f32>>,
    #[allow(dead_code)]
    cfg_path: PathBuf,
    /// Index into [`INPUT_METHODS`] used by the running instance.
    pub method_idx: usize,
}

impl VizWorker {
    /// Generate `viz.conf` in the config dir and spawn `cava` with the input
    /// method at `method_idx`.
    pub fn spawn(config_dir: &Path, method_idx: usize) -> Result<Self, SpawnError> {
        let method = INPUT_METHODS
            .get(method_idx)
            .copied()
            .unwrap_or(INPUT_METHODS[0]);
        std::fs::create_dir_all(config_dir).map_err(SpawnError::Io)?;
        let cfg_path = config_dir.join("viz.conf");
        std::fs::write(&cfg_path, render_conf(method)).map_err(SpawnError::Io)?;

        let mut child = Command::new("cava")
            .arg("-p")
            .arg(&cfg_path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => SpawnError::Missing,
                _ => SpawnError::Io(e),
            })?;

        let (tx, rx) = channel();
        let stdout: ChildStdout = child.stdout.take().expect("piped stdout");
        let _reader = std::thread::Builder::new()
            .name("cava-reader".into())
            .spawn(move || reader_loop(stdout, tx))
            .expect("failed to spawn cava reader thread");

        Ok(Self {
            child: Some(child),
            rx,
            cfg_path,
            method_idx,
        })
    }

    /// Latest complete frame (drains queued frames, keeps the newest).
    /// `None` when no new frame arrived since the last tick.
    pub fn latest(&mut self) -> Option<Vec<f32>> {
        let mut frame = None;
        while let Ok(next) = self.rx.try_recv() {
            frame = Some(next);
        }
        frame
    }

    /// Whether the child process is still running.
    pub fn alive(&mut self) -> bool {
        self.child
            .as_mut()
            .and_then(|c| c.try_wait().ok())
            .map(|status| status.is_none())
            .unwrap_or(false)
    }

    /// Next input method for a restart (`pipewire` → `pulse`); `None` when
    /// the ladder is exhausted.
    pub fn next_method_idx(&self) -> Option<usize> {
        let next = self.method_idx + 1;
        if next < INPUT_METHODS.len() {
            Some(next)
        } else {
            None
        }
    }

    /// Kill the child and reap it. Dropping the worker also drops the
    /// channel receiver, which ends the reader thread on its next send.
    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for VizWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Parse cava's raw ascii stream: buffered lines of `;`-separated integers.
fn reader_loop(stdout: ChildStdout, tx: Sender<Vec<f32>>) {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
        let Ok(line) = line else {
            break; // EOF (child exited) or IO error
        };
        let frame = parse_frame(&line, ASCII_MAX_RANGE as f32);
        if !frame.is_empty() && tx.send(frame).is_err() {
            break; // the UI dropped its receiver
        }
    }
}

/// Parse one cava ascii frame `"31;55;12;0"` → values normalized to
/// 0.0..1.0. Junk segments are skipped; out-of-range values are clamped.
pub fn parse_frame(line: &str, max: f32) -> Vec<f32> {
    if max <= 0.0 {
        return Vec::new();
    }
    line.split(';')
        .filter_map(|v| v.trim().parse::<f32>().ok())
        .map(|v| (v / max).clamp(0.0, 1.0))
        .collect()
}

/// Generated cava config: capture the default sink monitor and stream raw
/// ascii bars. Regenerated before every spawn, so users never edit it.
fn render_conf(method: &str) -> String {
    format!(
        r#"# Generated by walkz-tui — do not edit (rewritten on every launch).
[general]
framerate = {framerate}
autosens = 1
sensitivity = 100
bars = {bars}
lower_cutoff_freq = 50
higher_cutoff_freq = 10000
sleep_timer = 10

[input]
method = {method}
source = auto

[output]
method = raw
data_format = ascii
ascii_max_range = {max_range}
bar_delimiter = 59
frame_delimiter = 10

[smoothing]
noise_reduction = 77
"#,
        framerate = CAVA_FRAMERATE,
        bars = CAVA_BARS,
        max_range = ASCII_MAX_RANGE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_frame_normalizes_and_clamps() {
        let frame = parse_frame("31;55;12;0", 100.0);
        assert_eq!(frame, vec![0.31, 0.55, 0.12, 0.0]);
        // Over-range clamps to 1.0.
        assert_eq!(parse_frame("150", 100.0), vec![1.0]);
        // Whitespace and junk segments are tolerated.
        assert_eq!(parse_frame(" 10 ;; abc ; 50 ", 100.0), vec![0.10, 0.50]);
        // Empty frame.
        assert!(parse_frame("", 100.0).is_empty());
        // Degenerate max.
        assert!(parse_frame("10", 0.0).is_empty());
    }

    #[test]
    fn render_conf_contains_the_essentials() {
        let conf = render_conf("pipewire");
        assert!(conf.contains("method = pipewire"), "{conf}");
        assert!(conf.contains("source = auto"), "{conf}");
        assert!(conf.contains("method = raw"), "{conf}");
        assert!(conf.contains("data_format = ascii"), "{conf}");
        assert!(conf.contains("ascii_max_range = 100"), "{conf}");
        assert!(conf.contains("bars = 48"), "{conf}");
        assert!(conf.contains("framerate = 30"), "{conf}");
        assert!(conf.contains("bar_delimiter = 59"), "{conf}");
        assert!(conf.contains("frame_delimiter = 10"), "{conf}");
        // The pulse variant swaps only the input method.
        let conf = render_conf("pulse");
        assert!(conf.contains("method = pulse"), "{conf}");
        assert!(conf.contains("method = raw"), "{conf}");
    }

    #[test]
    fn next_method_steps_the_ladder() {
        let worker = VizWorker {
            child: None,
            rx: channel().1,
            cfg_path: PathBuf::from("/tmp/viz.conf"),
            method_idx: 0,
        };
        assert_eq!(worker.next_method_idx(), Some(1));
        let last = VizWorker {
            child: None,
            rx: channel().1,
            cfg_path: PathBuf::from("/tmp/viz.conf"),
            method_idx: INPUT_METHODS.len() - 1,
        };
        assert_eq!(last.next_method_idx(), None);
        // An out-of-range index still resolves to a sane method at spawn time
        // (clamped in `spawn`; here just check the table).
        assert_eq!(INPUT_METHODS[0], "pipewire");
        assert_eq!(INPUT_METHODS[1], "pulse");
    }
}
