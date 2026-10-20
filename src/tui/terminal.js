const readline = require('readline');
const { EventEmitter } = require('events');

/**
 * Terminal UI abstraction:
 * - Direct coordinate rendering (no trailing newlines, no scroll loops)
 * - Auto-wrap disabled during drawing (\x1b[?7l) to prevent wobble/jitter
 * - ANSI-aware string truncation and measurement
 * - Alternate screen buffer and raw keyboard input
 */
class Terminal extends EventEmitter {
  constructor() {
    super();
    this.width = process.stdout.columns || 80;
    this.height = process.stdout.rows || 24;
    this.isRaw = false;
    this.lastRenderedRows = [];

    process.stdout.on('resize', () => {
      this.width = process.stdout.columns || 80;
      this.height = process.stdout.rows || 24;
      this.lastRenderedRows = [];
      this.emit('resize', { width: this.width, height: this.height });
    });
  }

  init() {
    if (process.stdin.isTTY) {
      try {
        process.stdin.setRawMode(true);
        this.isRaw = true;
      } catch {
        // Non-interactive fallback
      }
    }
    process.stdin.resume();
    readline.emitKeypressEvents(process.stdin);

    process.stdin.on('keypress', (str, key) => {
      if (key && key.ctrl && key.name === 'c') {
        this.restore();
        process.exit(0);
      }
      this.emit('keypress', str, key);
    });

    // Enter alternate screen buffer, hide cursor, disable auto-wrap
    process.stdout.write('\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[2J');

    const cleanup = () => {
      this.restore();
      process.exit(0);
    };
    process.on('SIGINT', cleanup);
    process.on('SIGTERM', cleanup);
    process.on('exit', () => this.restore());
  }

  restore() {
    if (this.isRaw && process.stdin.isTTY) {
      try {
        process.stdin.setRawMode(false);
      } catch {
        // Ignore
      }
    }
    // Re-enable auto-wrap, show cursor, exit alternate screen, clear formatting
    process.stdout.write('\x1b[?7h\x1b[?25h\x1b[?1049l\x1b[0m');
  }

  clear() {
    this.lastRenderedRows = [];
    process.stdout.write('\x1b[2J\x1b[H');
  }

  /**
   * Flicker-free & wobble-free differential screen renderer
   * - Differential row updates (only lines that changed are sent)
   * - In-place overwrite with \x1b[K (no full-row \x1b[2K erase pulses)
   * - Strict column clamping and disabled auto-wrap
   */
  render(buffer) {
    const rows = Array.isArray(buffer) ? buffer : buffer.split('\n');
    const maxRows = Math.min(this.height, rows.length);
    let output = '';

    for (let i = 0; i < maxRows; i++) {
      const line = rows[i] || '';
      if (line !== this.lastRenderedRows[i]) {
        output += `\x1b[${i + 1};1H${line}\x1b[K`;
      }
    }

    // Clear any residual rows at bottom if screen height changed
    for (let i = maxRows; i < this.height; i++) {
      if (this.lastRenderedRows[i] !== '') {
        output += `\x1b[${i + 1};1H\x1b[2K`;
      }
    }

    this.lastRenderedRows = rows.slice(0, maxRows);
    while (this.lastRenderedRows.length < this.height) {
      this.lastRenderedRows.push('');
    }

    if (output) {
      process.stdout.write('\x1b[?7l' + output);
    }
  }

  // ---- 24-bit TrueColor Utilities ----
  static fg(r, g, b) {
    return `\x1b[38;2;${r};${g};${b}m`;
  }

  static bg(r, g, b) {
    return `\x1b[48;2;${r};${g};${b}m`;
  }

  static reset() {
    return '\x1b[0m';
  }

  static bold(text) {
    return `\x1b[1m${text}\x1b[22m`;
  }

  static dim(text) {
    return `\x1b[2m${text}\x1b[22m`;
  }

  static italic(text) {
    return `\x1b[3m${text}\x1b[23m`;
  }

  static Colors = {
    Primary: [168, 85, 247],     // Purple
    Secondary: [6, 182, 212],    // Cyan
    Accent: [236, 72, 153],      // Pink
    BgDark: [15, 17, 23],        // Dark Velvet
    BgCard: [26, 29, 39],        // Card
    Border: [60, 65, 85],        // Subtle Border
    Text: [243, 244, 246],       // White
    TextMuted: [156, 163, 175],  // Grey
    Success: [34, 197, 94],      // Green
    Warning: [234, 179, 8],      // Amber
    Red: [239, 68, 68]           // Red
  };

  /**
   * Computes visible glyph width ignoring all ANSI escape sequences
   */
  static visibleLength(str) {
    if (!str) return 0;
    return str.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '').length;
  }

  /**
   * Safely truncates a string to maxVisibleLen visible characters
   * without corrupting ANSI escape codes
   */
  static truncate(str, maxVisibleLen) {
    if (!str || maxVisibleLen <= 0) return '';
    let visible = 0;
    let result = '';
    let inAnsi = false;

    for (let i = 0; i < str.length; i++) {
      if (str[i] === '\x1b' && str[i + 1] === '[') {
        inAnsi = true;
      }
      if (inAnsi) {
        result += str[i];
        if ((str[i] >= 'A' && str[i] <= 'Z') || (str[i] >= 'a' && str[i] <= 'z')) {
          inAnsi = false;
        }
      } else {
        if (visible >= maxVisibleLen - 1 && visible < maxVisibleLen && maxVisibleLen > 1) {
          result += '…';
          visible++;
          break;
        } else if (visible >= maxVisibleLen) {
          break;
        }
        result += str[i];
        visible++;
      }
    }
    result += '\x1b[0m';
    return result;
  }

  /**
   * Pads or truncates string to exact target visible width
   */
  static pad(str, width, align = 'left') {
    const vLen = Terminal.visibleLength(str);
    if (vLen > width) {
      return Terminal.truncate(str, width);
    }
    const diff = Math.max(0, width - vLen);
    const padding = ' '.repeat(diff);
    return align === 'right' ? padding + str : str + padding;
  }
}

module.exports = Terminal;
