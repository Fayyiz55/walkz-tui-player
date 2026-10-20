/**
 * Audio DSP settings, Equalizer presets, and Crossfade math
 * Modeled after BitChord's playback/EqualizerCurve.kt & CrossfadeController.kt
 */

const EQ_FREQUENCIES = [
  "31Hz", "62Hz", "125Hz", "250Hz", "500Hz",
  "1kHz", "2kHz", "4kHz", "8kHz", "16kHz"
];

const EQ_PRESETS = {
  "Flat": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  "Bass Boost": [6, 5, 4, 2, 0, 0, 0, 1, 2, 3],
  "Treble Boost": [0, 0, 0, 0, 1, 2, 4, 6, 8, 8],
  "Vocal & Acoustic": [-2, -1, 0, 2, 4, 5, 4, 2, 1, 0],
  "Electronic / EDM": [5, 4, 2, 0, -2, 2, 3, 4, 5, 5],
  "Rock / Metal": [4, 3, 2, 0, -1, 1, 3, 4, 5, 4],
  "Classical": [3, 2, 1, 1, -1, -1, 0, 2, 3, 4],
  "Hip-Hop / R&B": [7, 6, 4, 1, -1, -1, 1, 2, 3, 4]
};

class DspManager {
  constructor() {
    this.currentPreset = "Flat";
    this.bands = [...EQ_PRESETS["Flat"]];
    this.crossfadeSec = 3; // 0 to 12s
    this.speed = 1.0;      // 0.5 to 2.0x
    this.limiterEnabled = true;
  }

  setPreset(presetName) {
    if (EQ_PRESETS[presetName]) {
      this.currentPreset = presetName;
      this.bands = [...EQ_PRESETS[presetName]];
      return true;
    }
    return false;
  }

  setBand(bandIndex, gainDb) {
    if (bandIndex >= 0 && bandIndex < 10) {
      this.bands[bandIndex] = Math.max(-12, Math.min(12, gainDb));
      this.currentPreset = "Custom";
    }
  }

  /**
   * Generates ffmpeg audio filter arguments for the current EQ configuration
   */
  getFfmpegFilters() {
    const filters = [];

    // 1. Equalizer biquads if not flat
    const isFlat = this.bands.every(b => b === 0);
    if (!isFlat) {
      const freqs = [31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
      const eqStrings = [];
      for (let i = 0; i < 10; i++) {
        if (this.bands[i] !== 0) {
          eqStrings.push(`equalizer=f=${freqs[i]}:width_type=o:w=1:g=${this.bands[i]}`);
        }
      }
      if (eqStrings.length > 0) {
        filters.push(eqStrings.join(','));
      }
    }

    // 2. Playback speed adjustment via atempo
    if (this.speed !== 1.0) {
      filters.push(`atempo=${this.speed}`);
    }

    return filters.join(',');
  }

  /**
   * BitChord True Crossfade volume easing at time t in [0, crossfadeSec]
   * Uses equal-power cosine/sine easing curve to prevent perceived volume drop
   */
  static crossfadeWeights(elapsedSec, durationSec) {
    if (durationSec <= 0) return { outgoing: 0, incoming: 1 };
    const progress = Math.max(0, Math.min(1, elapsedSec / durationSec));
    // Equal-power crossfade curve
    const outgoing = Math.cos(progress * Math.PI * 0.5);
    const incoming = Math.sin(progress * Math.PI * 0.5);
    return { outgoing, incoming };
  }
}

module.exports = {
  EQ_FREQUENCIES,
  EQ_PRESETS,
  DspManager: new DspManager()
};
