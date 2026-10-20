const { spawn } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { EventEmitter } = require('events');
const { DspManager } = require('./dsp');

class AudioController extends EventEmitter {
  constructor() {
    super();
    this.currentTrack = null;
    this.isPlaying = false;
    this.positionMs = 0;
    this.durationMs = 0;
    this.volume = 80;
    this.process = null;
    this.updateTimer = null;
    this.startTime = 0;
    this.pausedAtMs = 0;
    this.tempAudioDir = path.join(os.tmpdir(), 'bitchord_audio_cache');
    this.ensureTempDir();

    // Default to real system audio driver (PipeWire, PulseAudio, ALSA)
    this.audioDriver = process.env.SDL_AUDIODRIVER || null;
    this.outputDeviceStatus = this.audioDriver ? `Custom (${this.audioDriver})` : "PC Speakers (Pulse/PipeWire/ALSA)";
    this.hasFallenBackToDummy = false;
  }

  ensureTempDir() {
    try {
      if (!fs.existsSync(this.tempAudioDir)) {
        fs.mkdirSync(this.tempAudioDir, { recursive: true });
      }
    } catch {
      // Ignore
    }
  }

  setAudioDriver(driverName) {
    if (driverName === 'default' || !driverName) {
      this.audioDriver = null;
      this.outputDeviceStatus = "PC Speakers (Pulse/PipeWire/ALSA)";
    } else {
      this.audioDriver = driverName;
      this.outputDeviceStatus = `Driver: ${driverName}`;
    }
    this.hasFallenBackToDummy = false;
    this.emit('driverChange', this.outputDeviceStatus);
  }

  /**
   * Generates or retrieves local audio file for demo synthesized tracks
   */
  async prepareTrackAudio(track) {
    if (track.localPath && fs.existsSync(track.localPath)) {
      return track.localPath;
    }

    if (track.streamUrl) {
      return track.streamUrl;
    }

    // Generate procedural rich waveform audio with ffmpeg if this is a demo track
    const targetFile = path.join(this.tempAudioDir, `${track.id}.wav`);
    if (fs.existsSync(targetFile) && fs.statSync(targetFile).size > 1000) {
      return targetFile;
    }

    const freqs = track.toneFreqs || [261.63, 329.63, 392.0];
    const durSec = Math.round((track.durationMs || 120000) / 1000);
    const filter = `
      sine=f=${freqs[0]}:d=${durSec}[b1];
      sine=f=${freqs[1] || freqs[0]*1.25}:d=${durSec}[b2];
      sine=f=${freqs[2] || freqs[0]*1.5}:d=${durSec}[b3];
      sine=f=${freqs[3] || freqs[0]*2}:d=${durSec}[b4];
      [b1][b2][b3][b4]amix=inputs=4:dropout_transition=0,volume=0.4,chorus=0.7:0.9:55:0.4:0.25:2[out]
    `.replace(/\s+/g, '');

    return new Promise((resolve) => {
      const gen = spawn('ffmpeg', [
        '-f', 'lavfi', '-i', filter.trim(),
        '-map', '[out]',
        '-c:a', 'pcm_s16le',
        '-y', targetFile
      ], { stdio: 'ignore' });

      gen.on('close', (code) => {
        if (code === 0 && fs.existsSync(targetFile)) {
          resolve(targetFile);
        } else {
          const fallback = spawn('ffmpeg', [
            '-f', 'lavfi', '-i', `sine=f=440:d=${durSec}`,
            '-c:a', 'pcm_s16le',
            '-y', targetFile
          ], { stdio: 'ignore' });
          fallback.on('close', () => resolve(targetFile));
        }
      });
    });
  }

  /**
   * Play a track starting at seekMs
   */
  async play(track, seekMs = 0) {
    this.stopProcess();

    this.currentTrack = track;
    this.durationMs = track.durationMs || 180000;
    this.positionMs = Math.max(0, Math.min(seekMs, this.durationMs));
    this.pausedAtMs = this.positionMs;
    this.isPlaying = true;
    this.startTime = Date.now() - this.positionMs;

    const audioSource = await this.prepareTrackAudio(track);
    this.spawnFfplay(audioSource, this.positionMs);
    this.startProgressTicker();
    this.emit('stateChange', this.getState());
  }

  spawnFfplay(source, startMs = 0) {
    const startSec = Math.max(0, startMs / 1000);
    const volumeFactor = Math.max(0, Math.min(1.0, this.volume / 100));

    const env = { ...process.env };
    if (this.audioDriver) {
      env.SDL_AUDIODRIVER = this.audioDriver;
    }

    const args = [
      '-nodisp',
      '-autoexit',
      '-loglevel', 'error',
      '-volume', Math.round(volumeFactor * 100).toString()
    ];

    if (startSec > 0) {
      args.push('-ss', startSec.toFixed(2));
    }

    const filters = DspManager.getFfmpegFilters();
    if (filters) {
      args.push('-af', filters);
    }

    args.push(source);

    try {
      this.process = spawn('ffplay', args, { env });
      let stderrBuffer = '';

      if (this.process.stderr) {
        this.process.stderr.on('data', (d) => {
          stderrBuffer += d.toString();
        });
      }

      this.process.on('close', (code) => {
        // Check if ffplay failed because audio hardware device is not accessible
        if (code !== 0 && (stderrBuffer.includes('audio open failed') || stderrBuffer.includes('Could not open audio') || stderrBuffer.includes('No such file or directory'))) {
          if (!this.hasFallenBackToDummy && this.audioDriver !== 'dummy') {
            // Gracefully retry with dummy driver for headless/containerized setups
            this.hasFallenBackToDummy = true;
            this.audioDriver = 'dummy';
            this.outputDeviceStatus = "No Sound Card (Dummy Sink)";
            this.emit('audioNotice', 'No physical audio card found. Running in silent/headless mode.');
            this.spawnFfplay(source, startMs);
            return;
          }
        }

        // Natural track completion
        if (this.isPlaying && code === 0) {
          this.positionMs = this.durationMs;
          this.isPlaying = false;
          this.stopProgressTicker();
          this.emit('trackEnd', this.currentTrack);
        }
      });

      this.process.on('error', () => {
        // Spawn error
      });
    } catch {
      // Spawn catch
    }
  }

  pause() {
    if (!this.isPlaying) return;
    this.isPlaying = false;
    this.pausedAtMs = this.positionMs;
    this.stopProcess();
    this.stopProgressTicker();
    this.emit('stateChange', this.getState());
  }

  resume() {
    if (this.isPlaying || !this.currentTrack) return;
    this.play(this.currentTrack, this.pausedAtMs);
  }

  togglePlayPause() {
    if (this.isPlaying) {
      this.pause();
    } else {
      this.resume();
    }
  }

  seek(newPositionMs) {
    if (!this.currentTrack) return;
    const clamped = Math.max(0, Math.min(this.durationMs, newPositionMs));
    this.positionMs = clamped;
    this.pausedAtMs = clamped;
    if (this.isPlaying) {
      this.play(this.currentTrack, clamped);
    } else {
      this.emit('stateChange', this.getState());
    }
  }

  seekRelative(deltaMs) {
    this.seek(this.positionMs + deltaMs);
  }

  setVolume(vol) {
    this.volume = Math.max(0, Math.min(100, Math.round(vol)));
    this.emit('volumeChange', this.volume);
  }

  volumeUp(step = 5) {
    this.setVolume(this.volume + step);
  }

  volumeDown(step = 5) {
    this.setVolume(this.volume - step);
  }

  stopProcess() {
    if (this.process) {
      try {
        this.process.kill('SIGTERM');
      } catch {
        // Ignore
      }
      this.process = null;
    }
  }

  startProgressTicker() {
    this.stopProgressTicker();
    this.updateTimer = setInterval(() => {
      if (this.isPlaying) {
        this.positionMs = Date.now() - this.startTime;
        if (this.positionMs >= this.durationMs) {
          this.positionMs = this.durationMs;
          this.pause();
          this.emit('trackEnd', this.currentTrack);
        } else {
          this.emit('timeUpdate', this.positionMs);
        }
      }
    }, 100);
  }

  stopProgressTicker() {
    if (this.updateTimer) {
      clearInterval(this.updateTimer);
      this.updateTimer = null;
    }
  }

  getState() {
    return {
      isPlaying: this.isPlaying,
      positionMs: this.positionMs,
      durationMs: this.durationMs,
      volume: this.volume,
      track: this.currentTrack,
      outputDeviceStatus: this.outputDeviceStatus
    };
  }
}

module.exports = new AudioController();
