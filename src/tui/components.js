const Terminal = require('./terminal');

class Components {
  /**
   * Top Header with Responsive Navigation Tabs, Volume & Clock
   */
  static renderHeader(width, activeTab, jamConnected, partyCode, volume, connectedFolder = null) {
    const { Primary, Secondary, Text, TextMuted, Accent, Success, Border } = Terminal.Colors;
    const bColor = Terminal.fg(...Border);
    const reset = Terminal.reset();

    const logo = `${Terminal.fg(...Primary)}${Terminal.bold("♫ BitChord")}${reset}`;

    // Responsive tabs according to screen width
    let tabLabels;
    if (width >= 115) {
      tabLabels = [" [1] Now Playing ", " [2] Discover ", " [3] Queue ", " [4] Listen Together ", " [5] Equalizer ", " [6] Library "];
    } else if (width >= 80) {
      tabLabels = [" [1] Playing ", " [2] Find ", " [3] Queue ", " [4] Jam ", " [5] EQ ", " [6] Lib "];
    } else {
      tabLabels = [" 1 ", " 2 ", " 3 ", " 4 ", " 5 ", " 6 "];
    }

    const renderedTabs = tabLabels.map((tab, idx) => {
      if (idx === activeTab) {
        return `${Terminal.bg(...Primary)}${Terminal.fg(255, 255, 255)}${Terminal.bold(tab)}${reset}`;
      } else {
        return `${Terminal.fg(...TextMuted)}${tab}${reset}`;
      }
    }).join('');

    let partyBadge = "";
    if (jamConnected) {
      partyBadge = ` ${Terminal.bg(...Success)}${Terminal.fg(0, 0, 0)}${Terminal.bold(` JAM:${partyCode} `)}${reset}`;
    }

    let folderBadge = "";
    if (connectedFolder) {
      let folderName = require('path').basename(connectedFolder);
      if (folderName.length > 16) folderName = folderName.slice(0, 15) + '…';
      folderBadge = ` ${Terminal.bg(45, 55, 75)}${Terminal.fg(255, 255, 255)}${Terminal.bold(` 📁 ${folderName} `)}${reset}`;
    }

    const volStr = `󰕾 ${volume}%`;
    const timeStr = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    const rightInfo = `${Terminal.fg(...TextMuted)}${volStr} │ ${timeStr}${reset}`;

    const leftPart = `${logo} ${renderedTabs}${partyBadge}${folderBadge}`;
    const leftLen = Terminal.visibleLength(leftPart);
    const rightLen = Terminal.visibleLength(rightInfo);

    const space = Math.max(1, width - leftLen - rightLen - 1);
    const rawLine = `${leftPart}${' '.repeat(space)}${rightInfo}`;
    const headerLine = Terminal.truncate(rawLine, width - 1);
    const border = `${bColor}${'─'.repeat(width - 1)}${reset}`;

    return `${headerLine}\n${border}`;
  }

  /**
   * Rounded border box wrapper with exact column geometry
   */
  static renderBox(width, height, title, contentLines) {
    const { Border, Primary, Text } = Terminal.Colors;
    const bColor = Terminal.fg(...Border);
    const reset = Terminal.reset();

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    const titleStr = title ? ` ${Terminal.fg(...Primary)}${Terminal.bold(title)}${reset} ` : "";
    const titleLen = Terminal.visibleLength(titleStr);
    const topBarLen = Math.max(0, boxWidth - 2 - titleLen);
    const top = `${bColor}╭${reset}${titleStr}${bColor}${'─'.repeat(topBarLen)}╮${reset}`;
    const bottom = `${bColor}╰${'─'.repeat(boxWidth - 2)}╯${reset}`;

    const rows = [top];
    for (let i = 0; i < innerHeight; i++) {
      const line = contentLines[i] || "";
      const padded = Terminal.pad(line, innerWidth);
      rows.push(`${bColor}│${reset} ${padded} ${bColor}│${reset}`);
    }
    rows.push(bottom);

    return rows.join('\n');
  }

  /**
   * 8-Band Dynamic Audio Spectrum Visualizer
   */
  static renderVisualizer(width, isPlaying) {
    const { Primary, Secondary, Accent } = Terminal.Colors;
    const levels = isPlaying ? [
      Math.floor(Math.random() * 5) + 2,
      Math.floor(Math.random() * 6) + 3,
      Math.floor(Math.random() * 7) + 2,
      Math.floor(Math.random() * 6) + 3,
      Math.floor(Math.random() * 8) + 1,
      Math.floor(Math.random() * 5) + 3,
      Math.floor(Math.random() * 7) + 2,
      Math.floor(Math.random() * 5) + 2
    ] : [1, 1, 1, 1, 1, 1, 1, 1];

    const chars = [" ", " ", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
    const out = levels.map((lvl, idx) => {
      const color = idx % 2 === 0 ? Primary : Secondary;
      return `${Terminal.fg(...color)}${chars[Math.min(lvl, chars.length - 1)]}${Terminal.reset()}`;
    }).join(' ');

    return `${Terminal.fg(...Accent)}[SPECTRUM]${Terminal.reset()} ${out}`;
  }

  /**
   * Playback Scrub Bar: [━━━━━━●────────] 01:23 / 03:45
   */
  static renderProgressBar(width, positionMs, durationMs) {
    const { Primary, Border, TextMuted, Secondary } = Terminal.Colors;
    const posSec = Math.floor(positionMs / 1000);
    const durSec = Math.floor(durationMs / 1000);

    const fmtTime = (s) => {
      const m = Math.floor(s / 60);
      const rem = s % 60;
      return `${m.toString().padStart(2, '0')}:${rem.toString().padStart(2, '0')}`;
    };

    const timeStr = ` ${fmtTime(posSec)} / ${fmtTime(durSec)} `;
    const barWidth = Math.max(6, width - timeStr.length - 4);
    const progress = durationMs > 0 ? Math.min(1, positionMs / durationMs) : 0;
    const filledWidth = Math.round(progress * barWidth);

    const filled = '━'.repeat(Math.max(0, filledWidth - 1));
    const handle = '●';
    const empty = '─'.repeat(Math.max(0, barWidth - filledWidth));

    const bar = `${Terminal.fg(...Secondary)}${filled}${Terminal.fg(...Primary)}${handle}${Terminal.fg(...Border)}${empty}${Terminal.reset()}`;
    return `[${bar}]${Terminal.fg(...TextMuted)}${timeStr}${Terminal.reset()}`;
  }

  /**
   * Bottom Global Playback Controller Bar
   */
  static renderFooter(width, track, isPlaying, positionMs, durationMs, repeatMode, shuffle) {
    const { Primary, Secondary, Text, TextMuted, Border } = Terminal.Colors;
    const bColor = Terminal.fg(...Border);
    const reset = Terminal.reset();

    const playIcon = isPlaying ? "⏸ Pause" : "▶ Play";
    const repIcon = repeatMode === 'track' ? "1" : repeatMode === 'all' ? "All" : "Off";
    const shufIcon = shuffle ? "On" : "Off";

    let trackInfo = "No track playing";
    if (track) {
      const maxTitleLen = width >= 90 ? 28 : 18;
      const title = Terminal.truncate(track.title, maxTitleLen);
      const artist = Terminal.truncate(track.artist, 16);
      trackInfo = `${Terminal.fg(...Text)}${Terminal.bold(title)}${reset} ${Terminal.fg(...TextMuted)}by ${artist}${reset} ${Terminal.fg(...Secondary)}[${track.bitrate || "160k"}]${reset}`;
    }

    const stateRow = ` ${Terminal.fg(...Primary)}${playIcon}${reset} │ ${trackInfo} │ ${Terminal.fg(...TextMuted)}Rep: ${repIcon} │ Shuf: ${shufIcon}${reset}`;
    const pBar = ` ${this.renderProgressBar(width - 3, positionMs, durationMs)}`;

    const keyHints = width >= 95
      ? `${Terminal.fg(...TextMuted)} [Space] Play/Pause  [n/p] Next/Prev  [←/→] Seek  [↑/↓] Vol  [1-6] Tabs  [/] Search  [q] Quit${reset}`
      : `${Terminal.fg(...TextMuted)} [Space] Play  [n/p] Next  [←/→] Seek  [↑/↓] Vol  [1-6] Tabs  [/] Search  [q] Quit${reset}`;

    const topBorder = `${bColor}${'─'.repeat(width - 1)}${reset}`;

    return [
      topBorder,
      Terminal.truncate(stateRow, width - 1),
      Terminal.truncate(pBar, width - 1),
      Terminal.truncate(keyHints, width - 1)
    ].join('\n');
  }
}

module.exports = Components;
