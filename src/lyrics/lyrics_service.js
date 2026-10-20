const LrcParser = require('./lrc_parser');
const fs = require('fs');
const path = require('path');

class LyricsService {
  constructor() {
    this.cache = new Map();
  }

  /**
   * Cleans a track title or artist name by removing noisy video metadata
   * mirrors BitChord's TitleCleaning logic
   */
  static cleanTitle(title) {
    if (!title) return '';
    return title
      .replace(/\s*[\(\[](?:Official\s*(?:Music\s*)?(?:Video|Audio|Lyric\s*Video)|HQ|HD|4K|Remastered|Audio|Visualizer)[\)\]]/gi, '')
      .replace(/\s*\|\s*.*$/i, '')
      .replace(/\s*-\s*Official\s*.*$/i, '')
      .trim();
  }

  /**
   * Fetch synchronized lyrics for a track
   * @param {object} track - { id, title, artist, durationMs, localPath, lyrics }
   * @returns {Promise<Array<{ timeMs: number, text: string }>>}
   */
  async getLyrics(track) {
    if (!track) return [];

    // 1. Direct embedded lyrics (e.g. from demo tracks or ID3 tags)
    if (track.lyrics) {
      return LrcParser.parse(track.lyrics);
    }

    const cacheKey = `${track.artist} - ${track.title}`;
    if (this.cache.has(cacheKey)) {
      return this.cache.get(cacheKey);
    }

    // 2. Check for local .lrc file next to audio file
    if (track.localPath) {
      try {
        const lrcPath = track.localPath.replace(/\.[^.]+$/, '.lrc');
        if (fs.existsSync(lrcPath)) {
          const content = fs.readFileSync(lrcPath, 'utf8');
          const parsed = LrcParser.parse(content);
          if (parsed.length > 0) {
            this.cache.set(cacheKey, parsed);
            return parsed;
          }
        }
      } catch {
        // Continue to network lookup
      }
    }

    // 3. Online LRCLIB API lookup (BitChord's primary lyrics source)
    const cleanTitle = LyricsService.cleanTitle(track.title);
    const cleanArtist = LyricsService.cleanTitle(track.artist);
    const durationSec = Math.round((track.durationMs || 180000) / 1000);

    try {
      // Step 3a: Exact match lookup
      const exactUrl = `https://lrclib.net/api/get?track_name=${encodeURIComponent(cleanTitle)}&artist_name=${encodeURIComponent(cleanArtist)}&duration=${durationSec}`;
      const res = await fetch(exactUrl, {
        headers: { 'User-Agent': 'BitChord-TUI (https://github.com/kushagrasinghx/BitChord)' },
        signal: AbortSignal.timeout(3000)
      });

      if (res.ok) {
        const data = await res.json();
        if (data && data.syncedLyrics) {
          const parsed = LrcParser.parse(data.syncedLyrics);
          this.cache.set(cacheKey, parsed);
          return parsed;
        }
      }

      // Step 3b: Search fallback
      const searchUrl = `https://lrclib.net/api/search?track_name=${encodeURIComponent(cleanTitle)}&artist_name=${encodeURIComponent(cleanArtist)}`;
      const searchRes = await fetch(searchUrl, {
        headers: { 'User-Agent': 'BitChord-TUI (https://github.com/kushagrasinghx/BitChord)' },
        signal: AbortSignal.timeout(3500)
      });

      if (searchRes.ok) {
        const hits = await searchRes.json();
        if (Array.isArray(hits) && hits.length > 0) {
          // Find hit with closest duration
          let bestHit = null;
          let minDiff = Infinity;
          for (const hit of hits) {
            if (hit.syncedLyrics) {
              const diff = Math.abs((hit.duration || 0) - durationSec);
              if (diff < minDiff) {
                minDiff = diff;
                bestHit = hit;
              }
            }
          }

          if (bestHit && bestHit.syncedLyrics) {
            const parsed = LrcParser.parse(bestHit.syncedLyrics);
            this.cache.set(cacheKey, parsed);
            return parsed;
          }
        }
      }
    } catch {
      // Network blocked or timeout - fall through to procedural lyrics
    }

    // 4. Procedural fallback lyrics if no online lyrics were found
    const fallbackLyrics = this.generateFallbackLyrics(track);
    this.cache.set(cacheKey, fallbackLyrics);
    return fallbackLyrics;
  }

  generateFallbackLyrics(track) {
    const duration = track.durationMs || 180000;
    const lines = [
      { timeMs: 0, text: `♫ Now Playing: ${track.title}` },
      { timeMs: 4000, text: `by ${track.artist}` },
      { timeMs: 8000, text: `Album: ${track.album || "Unknown Album"}` },
      { timeMs: 14000, text: "• • •", isInstrumental: true },
      { timeMs: 20000, text: `Enjoy the sound of ${track.title}` },
      { timeMs: 30000, text: "Feel the frequency and groove" },
      { timeMs: 45000, text: "• • •", isInstrumental: true },
      { timeMs: 60000, text: `BitChord TUI • High Fidelity Audio` },
      { timeMs: 80000, text: "Dynamic waveform in sync" },
      { timeMs: 100000, text: "• • •", isInstrumental: true },
      { timeMs: 120000, text: `Rhythm fading to conclusion...` },
      { timeMs: Math.max(0, duration - 5000), text: "♫ Track ending" }
    ].filter(l => l.timeMs < duration);

    return lines;
  }
}

module.exports = new LyricsService();
