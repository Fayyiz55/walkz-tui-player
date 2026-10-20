const Terminal = require('../terminal');
const Components = require('../components');
const { EQ_FREQUENCIES, EQ_PRESETS } = require('../../player/dsp');

class EqualizerView {
  static render(width, height, dspManager, selectedBandIndex) {
    const { Primary, Secondary, Text, TextMuted, Border, Accent } = Terminal.Colors;
    const reset = Terminal.reset();
    const rows = [];

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    const titleInfo = `${Terminal.fg(...Accent)}${Terminal.bold("EQUALIZER")}${reset} │ Preset: ${Terminal.fg(...Primary)}${dspManager.currentPreset}${reset} │ XFade: ${dspManager.crossfadeSec}s │ Speed: ${dspManager.speed.toFixed(1)}x`;
    rows.push(Terminal.truncate(titleInfo, innerWidth));
    rows.push(`${Terminal.fg(...Border)}${'─'.repeat(innerWidth)}${reset}`);

    // Sliders
    const isCompact = innerHeight < 16;
    const bandsToShow = isCompact ? 5 : 10;
    const stepInterval = isCompact ? 2 : 1;

    for (let i = 0; i < 10; i += stepInterval) {
      const freq = EQ_FREQUENCIES[i].padEnd(5, ' ');
      const gain = dspManager.bands[i];
      const isSelected = i === selectedBandIndex;

      // Adjust slider steps to fit innerWidth
      const sliderWidth = Math.max(8, Math.min(20, innerWidth - 20));
      const pos = Math.round(((gain + 12) / 24) * sliderWidth);
      const leftPart = '─'.repeat(Math.max(0, pos));
      const handle = isSelected ? `${Terminal.fg(...Primary)}●${reset}` : `${Terminal.fg(...Secondary)}○${reset}`;
      const rightPart = '─'.repeat(Math.max(0, sliderWidth - pos));

      const pointer = isSelected ? `${Terminal.fg(...Secondary)}▶${reset}` : " ";
      const gainStr = `${gain >= 0 ? '+' : ''}${gain}dB`.padStart(5, ' ');
      const slider = `[${leftPart}${handle}${rightPart}]`;

      const line = `${pointer} ${Terminal.fg(...TextMuted)}${freq}${reset} ${slider} ${Terminal.fg(...Text)}${gainStr}${reset}`;
      rows.push(Terminal.truncate(line, innerWidth));
      if (rows.length >= innerHeight - 2) break;
    }

    while (rows.length < innerHeight - 1) {
      rows.push("");
    }

    const keyHints = `${Terminal.fg(...TextMuted)}[↑/↓] Band  [←/→] Gain  [p] Preset  [x/X] Crossfade  [+/-] Speed${reset}`;
    rows.push(Terminal.truncate(keyHints, innerWidth));

    return Components.renderBox(width, height, "Equalizer & DSP", rows);
  }
}

module.exports = EqualizerView;
