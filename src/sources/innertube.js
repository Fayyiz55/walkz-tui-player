/**
 * YouTube Music InnerTube reverse-engineered client
 * Uses WEB_REMIX client context (Client ID 67)
 */

const MUSIC_BASE = "https://music.youtube.com/youtubei/v1";
const WEB_REMIX_VERSION = "1.20250101.01.00";
const WEB_USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/134.0.0.0 Safari/537.36";

class InnertubeClient {
  constructor() {
    this.visitorData = null;
    this.webRemixVersion = WEB_REMIX_VERSION;
  }

  /**
   * Constructs InnerTube request body with proper client context
   */
  buildRequestBody(payload = {}) {
    return {
      context: {
        client: {
          clientName: "WEB_REMIX",
          clientVersion: this.webRemixVersion,
          hl: "en",
          gl: "US",
          ...(this.visitorData ? { visitorData: this.visitorData } : {})
        },
        user: {
          lockedSafetyMode: false
        },
        request: {
          useSsl: true
        }
      },
      ...payload
    };
  }

  getHeaders() {
    return {
      "Content-Type": "application/json",
      "X-Origin": "https://music.youtube.com",
      "Origin": "https://music.youtube.com",
      "Referer": "https://music.youtube.com/",
      "X-YouTube-Client-Name": "67",
      "X-YouTube-Client-Version": this.webRemixVersion,
      "User-Agent": WEB_USER_AGENT,
      ...(this.visitorData ? { "X-Goog-Visitor-Id": this.visitorData } : {})
    };
  }

  /**
   * Search for songs on YouTube Music
   * @param {string} query - Search query
   * @param {string} filterParam - Protobuf filter ('EgWKAQIIAWoKEAkQChAFEAMQBA==' for Songs)
   */
  async search(query, filterParam = "EgWKAQIIAWoKEAkQChAFEAMQBA==") {
    const url = `${MUSIC_BASE}/search?prettyPrint=false`;
    const body = this.buildRequestBody({
      query,
      params: filterParam
    });

    const res = await fetch(url, {
      method: "POST",
      headers: this.getHeaders(),
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(4000)
    });

    if (!res.ok) {
      throw new Error(`InnerTube search failed with HTTP ${res.status}`);
    }

    const data = await res.json();
    return this.parseSearchResults(data);
  }

  /**
   * Search suggestions (typeahead)
   */
  async getSearchSuggestions(input) {
    const url = `${MUSIC_BASE}/music/get_search_suggestions`;
    const body = this.buildRequestBody({ input });

    const res = await fetch(url, {
      method: "POST",
      headers: this.getHeaders(),
      body: JSON.stringify(body),
      signal: AbortSignal.timeout(2500)
    });

    if (!res.ok) return [];
    const data = await res.json();
    const suggestions = [];

    const walk = (node) => {
      if (!node || typeof node !== 'object') return;
      if (node.searchSuggestionRenderer) {
        const r = node.searchSuggestionRenderer;
        const q = r.navigationEndpoint?.searchEndpoint?.query;
        if (q) suggestions.push(q);
      }
      for (const val of Object.values(node)) {
        walk(val);
      }
    };
    walk(data);
    return [...new Set(suggestions)].slice(0, 7);
  }

  /**
   * Parse songs from InnerTube response
   */
  parseSearchResults(response) {
    const songs = [];
    const walk = (node) => {
      if (!node || typeof node !== 'object') return;
      if (node.musicResponsiveListItemRenderer) {
        const item = node.musicResponsiveListItemRenderer;
        const song = this.parseSongRenderer(item);
        if (song) songs.push(song);
      }
      for (const key of Object.keys(node)) {
        walk(node[key]);
      }
    };
    walk(response);
    return songs;
  }

  parseSongRenderer(renderer) {
    try {
      const flexColumns = renderer.flexColumns || [];
      if (flexColumns.length === 0) return null;

      // Extract title
      const titleRun = flexColumns[0]?.musicResponsiveListItemFlexColumnRenderer?.text?.runs?.[0];
      const title = titleRun?.text;
      const videoId = titleRun?.navigationEndpoint?.watchEndpoint?.videoId ||
                      renderer.playlistItemData?.videoId;

      if (!title || !videoId) return null;

      // Extract artist & album
      let artist = "Unknown Artist";
      let album = "Single";
      let durationStr = "3:30";

      const subtitleRuns = flexColumns[1]?.musicResponsiveListItemFlexColumnRenderer?.text?.runs || [];
      const parts = subtitleRuns.map(r => r.text).filter(t => t && t !== " • ");
      if (parts.length > 0) artist = parts[0];
      if (parts.length > 1) album = parts[1];
      if (parts.length > 2 && parts[2].includes(":")) durationStr = parts[2];

      const durationMs = this.parseDuration(durationStr);

      return {
        id: `yt_${videoId}`,
        videoId,
        title,
        artist,
        album,
        durationMs,
        source: "YouTube Music",
        codec: "opus (160 kbps)",
        bitrate: "160 kbps",
        sampleRate: "48000 Hz",
        loudnessDb: -1.5
      };
    } catch {
      return null;
    }
  }

  parseDuration(durationStr) {
    const parts = durationStr.split(':').map(Number);
    if (parts.length === 2) {
      return (parts[0] * 60 + parts[1]) * 1000;
    }
    if (parts.length === 3) {
      return (parts[0] * 3600 + parts[1] * 60 + parts[2]) * 1000;
    }
    return 180000;
  }
}

module.exports = new InnertubeClient();
