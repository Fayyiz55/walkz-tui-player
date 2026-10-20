const Terminal = require('../terminal');
const Components = require('../components');

class SearchView {
  static render(width, height, query, isSearchingInput, results, selectedIndex, filter, suggestions, isSearching = false) {
    const { Primary, Secondary, Text, TextMuted, Border, Accent, Warning, Success } = Terminal.Colors;
    const reset = Terminal.reset();
    const rows = [];

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    // 1. Search Bar
    const promptIcon = isSearchingInput
      ? `${Terminal.fg(...Accent)}✎ Search:${reset} `
      : `${Terminal.fg(...Secondary)}🔍 Search:${reset} `;
    
    let inputDisplay = "";
    if (isSearching) {
      inputDisplay = `${Terminal.fg(...Warning)}[Searching sources...]${reset}`;
    } else if (isSearchingInput) {
      const cursor = `${Terminal.bg(255, 255, 255)}${Terminal.fg(0, 0, 0)} ${reset}`;
      inputDisplay = query
        ? `${Terminal.bold(query)}${cursor}`
        : `${cursor}${Terminal.fg(...TextMuted)}Type keywords & press [Enter] to search...${reset}`;
    } else {
      inputDisplay = query
        ? `${Terminal.bold(query)} ${Terminal.fg(...TextMuted)}(Press [/] to edit)${reset}`
        : `${Terminal.fg(...TextMuted)}Type query... (Press [/] to search)${reset}`;
    }

    rows.push(Terminal.truncate(`${promptIcon}${inputDisplay}`, innerWidth));

    // 2. Filter Bar
    const filters = ["all", "youtube", "jiosaavn", "local"];
    const filterLabels = { all: "All Sources", youtube: "YouTube Music", jiosaavn: "JioSaavn", local: "Local Files" };
    const filterRow = filters.map(f => {
      const active = f === filter;
      return active
        ? `${Terminal.bg(...Primary)}${Terminal.fg(255, 255, 255)}${Terminal.bold(` [${filterLabels[f]}] `)}${reset}`
        : `${Terminal.fg(...TextMuted)} [${filterLabels[f]}] ${reset}`;
    }).join(' ');

    const filterText = `Filter: ${filterRow}`;
    rows.push(Terminal.truncate(filterText, innerWidth));
    rows.push(`${Terminal.fg(...Border)}${'─'.repeat(innerWidth)}${reset}`);

    // 3. Results List
    const reservedHeaderRows = 3;
    const reservedFooterRows = 1;
    const listCapacity = Math.max(2, innerHeight - reservedHeaderRows - reservedFooterRows);

    if (isSearching) {
      rows.push(`${Terminal.fg(...Warning)}Scanning online & local catalog...${reset}`);
    } else if (!results || results.length === 0) {
      rows.push(`${Terminal.fg(...TextMuted)}No results found for "${query || ""}". Press [/] to search again.${reset}`);
    } else {
      const startIdx = Math.max(0, Math.min(selectedIndex - Math.floor(listCapacity / 2), results.length - listCapacity));
      const endIdx = Math.min(results.length, startIdx + listCapacity);

      for (let i = startIdx; i < endIdx; i++) {
        const item = results[i];
        const isSelected = i === selectedIndex;
        const pointer = isSelected ? `${Terminal.fg(...Secondary)}▶${reset}` : " ";

        const durSec = Math.floor((item.durationMs || 180000) / 1000);
        const durStr = `${Math.floor(durSec / 60)}:${(durSec % 60).toString().padStart(2, '0')}`;
        const sourceBadge = `[${item.source || "Web"}]`;

        // Responsive title & artist allocation
        const rightPart = `${Terminal.fg(...TextMuted)}${durStr}${reset}`;
        const rightV = Terminal.visibleLength(rightPart);
        const sourceV = sourceBadge.length;
        const overhead = 2 /* pointer */ + 1 /* space */ + 4 /* " by " */ + 1 /* space */ + sourceV + 1 + rightV + 2;
        const availableText = Math.max(16, innerWidth - overhead);

        const titleMax = Math.max(10, Math.floor(availableText * 0.58));
        const artistMax = Math.max(6, Math.floor(availableText * 0.42));

        const title = Terminal.truncate(item.title, titleMax);
        const artist = Terminal.truncate(item.artist, artistMax);

        const leftPart = `${pointer} ${Terminal.bold(title)} ${Terminal.fg(...TextMuted)}by${reset} ${artist} ${Terminal.fg(...Secondary)}${sourceBadge}${reset}`;
        const leftV = Terminal.visibleLength(leftPart);

        const space = Math.max(1, innerWidth - leftV - rightV);
        let lineContent = `${leftPart}${' '.repeat(space)}${rightPart}`;

        if (isSelected) {
          lineContent = `${Terminal.bg(40, 44, 55)}${lineContent}${reset}`;
        }
        rows.push(lineContent);
      }
    }

    // Pad remaining rows to strictly match innerHeight
    while (rows.length < innerHeight - 1) {
      rows.push("");
    }

    const keyHints = `${Terminal.fg(...TextMuted)}[Enter] Play  [a] Add Queue  [/] Search  [Tab] Filter  [↑/↓] Select${reset}`;
    rows.push(Terminal.truncate(keyHints, innerWidth));

    return Components.renderBox(width, height, "Discover & Search", rows);
  }
}

module.exports = SearchView;
