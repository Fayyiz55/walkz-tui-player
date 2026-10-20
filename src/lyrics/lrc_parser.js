/**
 * Parser for standard and enhanced LRC synchronized lyrics.
 */

class LrcParser {
  /**
   * Parse an LRC formatted string into an array of LyricLine objects
   * @param {string} lrcString - Raw LRC format content
   * @returns {Array<{ timeMs: number, text: string, syllables?: Array<{ startMs: number, text: string }> }>}
   */
  static parse(lrcString) {
    if (!lrcString || typeof lrcString !== 'string') return [];

    const lines = lrcString.split(/\r?\n/);
    const parsed = [];
    // Matches [mm:ss.xx] or [mm:ss.xxx]
    const timeRegex = /\[(\d{1,2}):(\d{1,2})(?:\.(\d{1,3}))?\]/g;
    const syllableRegex = /<(\d{1,2}):(\d{1,2})(?:\.(\d{1,3}))?>([^<]+)/g;

    for (const rawLine of lines) {
      const line = rawLine.trim();
      if (!line) continue;

      // Check if line contains enhanced syllable timings: e.g. <00:12.34> word
      const hasSyllables = line.includes('<') && line.includes('>');

      // Find all timestamps at the start of the line (some lines have multiple tags)
      const matches = [...line.matchAll(timeRegex)];
      if (matches.length === 0) continue;

      // Extract the text after all timestamp tags
      const textPart = line.replace(timeRegex, '').trim();

      for (const match of matches) {
        const mins = parseInt(match[1], 10);
        const secs = parseInt(match[2], 10);
        let fracMs = 0;
        if (match[3]) {
          const frac = match[3];
          if (frac.length === 1) fracMs = parseInt(frac, 10) * 100;
          else if (frac.length === 2) fracMs = parseInt(frac, 10) * 10;
          else if (frac.length === 3) fracMs = parseInt(frac, 10);
        }
        const timeMs = mins * 60000 + secs * 1000 + fracMs;

        let syllables = null;
        if (hasSyllables) {
          syllables = [];
          const sylMatches = [...textPart.matchAll(syllableRegex)];
          for (const sMatch of sylMatches) {
            const sMin = parseInt(sMatch[1], 10);
            const sSec = parseInt(sMatch[2], 10);
            let sFrac = 0;
            if (sMatch[3]) {
              const f = sMatch[3];
              sFrac = f.length === 2 ? parseInt(f, 10) * 10 : parseInt(f, 10);
            }
            const sTime = sMin * 60000 + sSec * 1000 + sFrac;
            syllables.push({
              startMs: sTime,
              text: sMatch[4].trim()
            });
          }
        }

        parsed.push({
          timeMs,
          text: hasSyllables ? textPart.replace(/<[^>]+>/g, '').trim() : textPart,
          syllables
        });
      }
    }

    // Sort by timestamp
    parsed.sort((a, b) => a.timeMs - b.timeMs);

    // Insert instrumental breaks where gap is > 4.5 seconds
    const withInstrumentals = [];
    for (let i = 0; i < parsed.length; i++) {
      withInstrumentals.push(parsed[i]);
      if (i < parsed.length - 1) {
        const gap = parsed[i + 1].timeMs - parsed[i].timeMs;
        if (gap > 5000) {
          withInstrumentals.push({
            timeMs: parsed[i].timeMs + 2000,
            text: "• • •",
            isInstrumental: true,
            endMs: parsed[i + 1].timeMs - 500
          });
        }
      }
    }

    return withInstrumentals;
  }

  /**
   * Find index of active lyric line given playback position in milliseconds
   */
  static getActiveLineIndex(lyrics, positionMs) {
    if (!lyrics || lyrics.length === 0) return -1;
    let active = -1;
    for (let i = 0; i < lyrics.length; i++) {
      if (positionMs >= lyrics[i].timeMs) {
        active = i;
      } else {
        break;
      }
    }
    return active;
  }
}

module.exports = LrcParser;
