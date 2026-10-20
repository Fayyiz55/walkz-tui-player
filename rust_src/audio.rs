use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Instant;
use crate::models::{RepeatMode, Track};

pub struct AudioPlayer {
    current_child: Option<Child>,
    pub is_playing: bool,
    pub current_track: Option<Track>,
    pub position_secs: f64,
    pub duration_secs: u64,
    pub volume: u32, // 0..100
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,
    pub last_tick: Instant,
    pub output_driver: String,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let driver = Self::detect_audio_driver();
        Self {
            current_child: None,
            is_playing: false,
            current_track: None,
            position_secs: 0.0,
            duration_secs: 0,
            volume: 85,
            repeat_mode: RepeatMode::All,
            shuffle: false,
            last_tick: Instant::now(),
            output_driver: driver,
        }
    }

    /// Detect active audio subsystem on Linux (PipeWire, PulseAudio, ALSA)
    fn detect_audio_driver() -> String {
        if Path::new("/run/user/1000/pipewire-0").exists() || Path::new("/run/pipewire").exists() {
            "PipeWire (Low Latency / PC Speakers)".to_string()
        } else if Path::new("/run/user/1000/pulse").exists() || Path::new("/run/pulse").exists() {
            "PulseAudio (Default Desktop Output)".to_string()
        } else if Path::new("/proc/asound/cards").exists() {
            "ALSA (Direct Hardware Soundcard)".to_string()
        } else {
            "System Default Audio (ffplay / SDL2)".to_string()
        }
    }

    /// Play a track from start or specific position
    pub fn play(&mut self, track: Track, start_secs: f64) {
        self.stop();

        self.duration_secs = track.duration_secs;
        self.position_secs = start_secs.min(track.duration_secs as f64);
        self.is_playing = true;
        self.last_tick = Instant::now();
        self.current_track = Some(track);

        self.spawn_ffplay();
    }

    /// Toggle play / pause
    pub fn toggle_pause(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.resume();
        }
    }

    pub fn pause(&mut self) {
        if !self.is_playing {
            return;
        }
        self.is_playing = false;
        self.kill_child();
    }

    pub fn resume(&mut self) {
        if self.is_playing || self.current_track.is_none() {
            return;
        }
        self.is_playing = true;
        self.last_tick = Instant::now();
        self.spawn_ffplay();
    }

    pub fn stop(&mut self) {
        self.is_playing = false;
        self.position_secs = 0.0;
        self.kill_child();
    }

    pub fn seek_relative(&mut self, delta_secs: f64) {
        if self.current_track.is_none() {
            return;
        }
        let dur = self.duration_secs as f64;
        let new_pos = (self.position_secs + delta_secs).clamp(0.0, dur);
        self.position_secs = new_pos;

        if self.is_playing {
            self.kill_child();
            self.spawn_ffplay();
        }
    }

    pub fn set_volume(&mut self, new_vol: u32) {
        self.volume = new_vol.clamp(0, 100);
        // If actively playing, respawn with new volume
        if self.is_playing {
            self.kill_child();
            self.spawn_ffplay();
        }
    }

    pub fn change_volume(&mut self, delta: i32) {
        let current = self.volume as i32;
        let next = (current + delta).clamp(0, 100) as u32;
        self.set_volume(next);
    }

    /// Periodic tick to advance time and check process status
    /// Returns true if track naturally finished and should transition to next track
    pub fn tick(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64();
        self.last_tick = now;

        if !self.is_playing {
            return false;
        }

        self.position_secs += elapsed;

        if self.duration_secs > 0 && self.position_secs >= (self.duration_secs as f64) {
            // Track finished!
            return true;
        }

        // Check if child exited on its own
        if let Some(child) = &mut self.current_child {
            if let Ok(Some(_status)) = child.try_wait() {
                self.current_child = None;
            }
        }

        false
    }

    fn spawn_ffplay(&mut self) {
        let Some(track) = &self.current_track else {
            return;
        };

        // If it's a demo:// track without real audio file, simulate synthetic playback
        if track.path.to_string_lossy().starts_with("demo://") || !track.path.exists() {
            return;
        }

        let mut cmd = Command::new("ffplay");
        cmd.arg("-nodisp")
            .arg("-autoexit")
            .arg("-loglevel")
            .arg("quiet")
            .arg("-volume")
            .arg(self.volume.to_string());

        if self.position_secs > 0.5 {
            cmd.arg("-ss").arg(format!("{:.2}", self.position_secs));
        }

        cmd.arg(&track.path);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        if let Ok(child) = cmd.spawn() {
            self.current_child = Some(child);
        }
    }

    fn kill_child(&mut self) {
        if let Some(mut child) = self.current_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.kill_child();
    }
}
