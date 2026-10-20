const Terminal = require('../terminal');
const Components = require('../components');
const LrcParser = require('../../lyrics/lrc_parser');

class NowPlayingView {
  static generateAsciiArt(track, compact = false) {
    const { Primary, Secondary, Accent } = Terminal.Colors;
    const p = Terminal.fg(...Primary);
    const s = Terminal.fg(...Secondary);
    const a = Terminal.fg(...Accent);
    const r = Terminal.reset();

    if (compact) {
      return [
        `${p}╭──────────────╮${r}`,
        `${p}│ ${s}◢██${a}♫ BITCHORD${s}██◣ ${p}│${r}`,
        `${p}│ ${s}◥████████████◤ ${p}│${r}`,
        `${p}╰──────────────╯${r}`
      ];
    }

    return [
      `${p}╭───────────────╮${r}`,
      `${p}│  ${s}◢█████████◣  ${p}│${r}`,
      `${p}│ ${s}███${a}▀▀▀▀▀▀▀${s}███ ${p}│${r}`,
      `${p}│ ${s}██ ${a}♫ BITCHORD${s} ██ ${p}│${r}`,
      `${p}│ ${s}██ ${a}▄▄▄▄▄▄▄${s} ██ ${p}│${r}`,
      `${p}│  ${s}◥█████████◤  ${p}│${r}`,
      `${p}│   ${a}● HI-RES ●  ${p}│${r}`,
      `${p}╰───────────────╯${r}`
    ];
  }

  static render(width, height, track, isPlaying, positionMs, durationMs, lyrics, outputStatus = "PC Speakers") {
    const { Primary, Secondary, Text, TextMuted, Border, Accent, Success } = Terminal.Colors;
    const bColor = Terminal.fg(...Border);
    const reset = Terminal.reset();

    if (!track) {
      const emptyLines = [
        "",
        `${Terminal.fg(...TextMuted)}No track currently loaded.${reset}`,
        "",
        `${Terminal.fg(...Primary)}Press [2] to search and discover music,${reset}`,
        `${Terminal.fg(...Secondary)}or press [Enter] on a track in the Queue.${reset}`
      ];
      return Components.renderBox(width, height, "Now Playing", emptyLines);
    }

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    // If terminal is narrow (< 65 cols), render single-column
    if (width < 65) {
      const singleLines = [
        `${Terminal.bold(Terminal.truncate(track.title, innerWidth))}`,
        `${Terminal.fg(...Primary)}${Terminal.truncate(track.artist, innerWidth)}${reset}`,
        `${Terminal.fg(...TextMuted)}Album: ${Terminal.truncate(track.album || "Single", innerWidth - 8)}${reset}`,
        `${Terminal.fg(...Secondary)}Bitrate: ${track.bitrate || "160 kbps"}${reset}`,
        Components.renderVisualizer(innerWidth, isPlaying)
      ];
      return Components.renderBox(width, height, "Now Playing", singleLines);
    }

    // Split View (Left: Art & Stats | Right: Synced Lyrics)
    const colWidth = Math.max(15, Math.floor((innerWidth - 3) / 2));
    const isCompact = height < 26;

    const leftLines = [];
    const rightLines = [];

    // Left Column: Artwork & Metadata
    const art = this.generateAsciiArt(track, isCompact);
    art.forEach(line => leftLines.push(line));

    leftLines.push(`${Terminal.bold(Terminal.truncate(track.title, colWidth - 2))}`);
    leftLines.push(`${Terminal.fg(...Primary)}${Terminal.truncate(track.artist, colWidth - 2)}${reset}`);

    if (!isCompact) {
      leftLines.push(`${Terminal.fg(...TextMuted)}Album: ${Terminal.truncate(track.album || "Single", colWidth - 8)}${reset}`);
      leftLines.push(`${Terminal.fg(...Secondary)}Source: ${track.source || "Local"}${reset}`);
      leftLines.push(`${Terminal.fg(...Success)}Output: ${Terminal.truncate(outputStatus || "PC Speakers", colWidth - 9)}${reset}`);
      leftLines.push(`${Terminal.fg(...Accent)}Codec: ${track.codec || "opus"} (${track.bitrate || "160k"})${reset}`);
      if (track.bpm) leftLines.push(`${Terminal.fg(...TextMuted)}BPM: ${track.bpm} │ Key: ${track.key || "C"}${reset}`);
    } else {
      leftLines.push(`${Terminal.fg(...Success)}🔊 ${Terminal.truncate(outputStatus || "PC Speakers", colWidth - 4)}${reset}`);
      leftLines.push(`${Terminal.fg(...Secondary)}${track.codec || "opus"} │ ${track.bitrate || "160k"}${reset}`);
    }

    leftLines.push(Components.renderVisualizer(colWidth, isPlaying));

    // Right Column: Synchronized Lyrics
    rightLines.push(`${Terminal.fg(...Secondary)}${Terminal.bold("LYRICS")}${reset}`);
    rightLines.push(`${bColor}${'─'.repeat(Math.max(6, colWidth - 2))}${reset}`);

    if (!lyrics || lyrics.length === 0) {
      rightLines.push(`${Terminal.fg(...TextMuted)}Searching LRCLIB...${reset}`);
    } else {
      const activeIdx = LrcParser.getActiveLineIndex(lyrics, positionMs);
      const availableLyricsSlots = Math.max(3, innerHeight - 2);
      const startIdx = Math.max(0, activeIdx - Math.floor(availableLyricsSlots / 2));
      const endIdx = Math.min(lyrics.length, startIdx + availableLyricsSlots);

      for (let i = startIdx; i < endIdx; i++) {
        const item = lyrics[i];
        const isCurrent = i === activeIdx;
        const lineText = Terminal.truncate(item.text, colWidth - 4);

        if (isCurrent) {
          rightLines.push(`${Terminal.bg(45, 30, 60)}${Terminal.fg(255, 255, 255)}${Terminal.bold("▶ " + lineText)}${reset}`);
        } else if (Math.abs(i - activeIdx) <= 1) {
          rightLines.push(`  ${Terminal.fg(...Text)}${lineText}${reset}`);
        } else {
          rightLines.push(`  ${Terminal.fg(...TextMuted)}${lineText}${reset}`);
        }
      }
    }

    // Merge into rows clamped to innerHeight
    const mergedRows = [];
    for (let r = 0; r < innerHeight; r++) {
      const leftStr = Terminal.pad(leftLines[r] || "", colWidth);
      const rightStr = Terminal.pad(rightLines[r] || "", colWidth);
      mergedRows.push(`${leftStr} ${bColor}│${reset} ${rightStr}`);
    }

    return Components.renderBox(width, height, "Now Playing", mergedRows);
  }
}

module.exports = NowPlayingView;
