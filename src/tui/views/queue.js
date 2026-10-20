const Terminal = require('../terminal');
const Components = require('../components');

class QueueView {
  static render(width, height, queue, currentIndex, selectedIndex, history = [], repeatMode = 'none', shuffle = false) {
    const { Primary, Secondary, Text, TextMuted, Border, Success } = Terminal.Colors;
    const reset = Terminal.reset();
    const rows = [];

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    const repStr = repeatMode === 'track' ? "1" : repeatMode === 'all' ? "All" : "Off";
    const shufStr = shuffle ? "On" : "Off";
    const headerInfo = `${Terminal.fg(...Secondary)}Tracks: ${queue.length}${reset} │ ${Terminal.fg(...TextMuted)}Rep: ${repStr} │ Shuf: ${shufStr}${reset}`;
    rows.push(Terminal.truncate(headerInfo, innerWidth));
    rows.push(`${Terminal.fg(...Border)}${'─'.repeat(innerWidth)}${reset}`);

    const reservedHeaderRows = 2;
    const reservedFooterRows = 1;
    const listCapacity = Math.max(2, innerHeight - reservedHeaderRows - reservedFooterRows);

    if (queue.length === 0) {
      rows.push(`${Terminal.fg(...TextMuted)}The queue is currently empty. Press [2] to discover music.${reset}`);
    } else {
      const startIdx = Math.max(0, Math.min(selectedIndex - Math.floor(listCapacity / 2), queue.length - listCapacity));
      const endIdx = Math.min(queue.length, startIdx + listCapacity);

      for (let i = startIdx; i < endIdx; i++) {
        const item = queue[i];
        const isCurrent = i === currentIndex;
        const isSelected = i === selectedIndex;

        const num = `${(i + 1).toString().padStart(2, ' ')}.`;
        const marker = isCurrent ? `${Terminal.fg(...Success)}♫${reset}` : isSelected ? `${Terminal.fg(...Secondary)}▶${reset}` : " ";

        const durSec = Math.floor((item.durationMs || 180000) / 1000);
        const durStr = `${Math.floor(durSec / 60)}:${(durSec % 60).toString().padStart(2, '0')}`;
        const rightPart = `${Terminal.fg(...TextMuted)}${durStr}${reset}`;
        const rightV = Terminal.visibleLength(rightPart);

        const overhead = 2 /* marker */ + 4 /* num */ + 4 /* " by " */ + rightV + 2;
        const availableText = Math.max(16, innerWidth - overhead);
        const titleMax = Math.max(10, Math.floor(availableText * 0.58));
        const artistMax = Math.max(6, Math.floor(availableText * 0.42));

        const title = Terminal.truncate(item.title, titleMax);
        const artist = Terminal.truncate(item.artist, artistMax);

        let leftPart = `${marker} ${Terminal.fg(...TextMuted)}${num}${reset} ${Terminal.bold(title)} ${Terminal.fg(...TextMuted)}by${reset} ${artist}`;
        if (isCurrent) {
          leftPart = `${Terminal.fg(...Primary)}${Terminal.bold(leftPart)}${reset}`;
        }
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

    const keyHints = `${Terminal.fg(...TextMuted)}[Enter] Jump  [d] Delete  [J/K] Move Down/Up  [c] Clear  [s] Shuf  [r] Rep${reset}`;
    rows.push(Terminal.truncate(keyHints, innerWidth));

    return Components.renderBox(width, height, "Queue", rows);
  }
}

module.exports = QueueView;
