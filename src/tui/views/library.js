const Terminal = require('../terminal');
const Components = require('../components');

class LibraryView {
  static render(width, height, localTracks, selectedIndex, isScanning, connectedFolder, isInputtingFolder, inputVal) {
    const { Primary, Secondary, Text, TextMuted, Border, Accent, Success } = Terminal.Colors;
    const reset = Terminal.reset();
    const rows = [];

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    const folderTitle = connectedFolder
      ? `${Terminal.fg(...Success)}📁 Connected: ${Terminal.bold(connectedFolder)}${reset}`
      : `${Terminal.fg(...TextMuted)}Default: ~/Music, ~/Downloads, current dir${reset}`;

    rows.push(Terminal.truncate(`${Terminal.fg(...Accent)}${Terminal.bold("LOCAL AUDIO REPOSITORY")}${reset} │ ${folderTitle}`, innerWidth));
    rows.push(`${Terminal.fg(...Border)}${'─'.repeat(innerWidth)}${reset}`);

    // If user is entering a folder path
    if (isInputtingFolder) {
      const cursor = `${Terminal.bg(255, 255, 255)}${Terminal.fg(0, 0, 0)} ${reset}`;
      rows.push(Terminal.truncate(`${Terminal.fg(...Secondary)}Enter Folder Path:${reset} ${Terminal.bold(inputVal)}${cursor}`, innerWidth));
      rows.push(Terminal.truncate(`${Terminal.fg(...TextMuted)}Press [Enter] to connect, [Esc] to cancel${reset}`, innerWidth));
      rows.push("");
    }

    const reservedHeaderRows = isInputtingFolder ? 5 : 2;
    const reservedFooterRows = 1;
    const listCapacity = Math.max(2, innerHeight - reservedHeaderRows - reservedFooterRows);

    if (isScanning) {
      rows.push(`${Terminal.fg(...Secondary)}Scanning files in ${connectedFolder || "local directories"}...${reset}`);
    } else if (!localTracks || localTracks.length === 0) {
      rows.push(`${Terminal.fg(...TextMuted)}No audio files found. Supported: .mp3, .flac, .wav, .m4a, .ogg, .opus${reset}`);
      rows.push(`${Terminal.fg(...Primary)}Press [f] to connect your specific music folder!${reset}`);
    } else {
      const startIdx = Math.max(0, Math.min(selectedIndex - Math.floor(listCapacity / 2), localTracks.length - listCapacity));
      const endIdx = Math.min(localTracks.length, startIdx + listCapacity);

      for (let i = startIdx; i < endIdx; i++) {
        const item = localTracks[i];
        const isSelected = i === selectedIndex;
        const pointer = isSelected ? `${Terminal.fg(...Secondary)}▶${reset}` : " ";

        const durSec = Math.floor((item.durationMs || 0) / 1000);
        const durStr = durSec > 0 ? `${Math.floor(durSec / 60)}:${(durSec % 60).toString().padStart(2, '0')}` : "--:--";
        const codecBadge = `[${item.codec || "audio"}]`;

        const rightPart = `${Terminal.fg(...TextMuted)}${durStr}${reset}`;
        const rightV = Terminal.visibleLength(rightPart);
        const codecV = codecBadge.length;

        const overhead = 2 /* pointer */ + 4 /* " by " */ + 1 + codecV + 1 + rightV + 2;
        const availableText = Math.max(16, innerWidth - overhead);
        const titleMax = Math.max(10, Math.floor(availableText * 0.58));
        const artistMax = Math.max(6, Math.floor(availableText * 0.42));

        const title = Terminal.truncate(item.title, titleMax);
        const artist = Terminal.truncate(item.artist, artistMax);

        const leftPart = `${pointer} ${Terminal.bold(title)} ${Terminal.fg(...TextMuted)}by${reset} ${artist} ${Terminal.fg(...Secondary)}${codecBadge}${reset}`;
        const leftV = Terminal.visibleLength(leftPart);

        const space = Math.max(1, innerWidth - leftV - rightV);
        let line = `${leftPart}${' '.repeat(space)}${rightPart}`;

        if (isSelected) {
          line = `${Terminal.bg(40, 44, 55)}${line}${reset}`;
        }
        rows.push(line);
      }
    }

    while (rows.length < innerHeight - 1) {
      rows.push("");
    }

    const keyHints = `${Terminal.fg(...TextMuted)}[Enter] Play  [a] Add Queue  [f] Connect Folder  [r] Rescan  [↑/↓] Select${reset}`;
    rows.push(Terminal.truncate(keyHints, innerWidth));

    return Components.renderBox(width, height, "Local Library", rows);
  }
}

module.exports = LibraryView;
