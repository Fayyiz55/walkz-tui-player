const innertube = require('./innertube');
const jiosaavn = require('./jiosaavn');
const localLib = require('./local_library');
const { DEMO_TRACKS } = require('../data/demo_tracks');

class SourceManager {
  constructor() {
    this.exclusiveFolderMode = false;
  }

  setExclusiveFolder(isExclusive) {
    this.exclusiveFolderMode = isExclusive;
  }

  async search(query, filter = 'all') {
    // If exclusive folder mode is active, ONLY return tracks from that folder
    if (this.exclusiveFolderMode || localLib.getFolder()) {
      return localLib.search(query);
    }

    if (!query || !query.trim()) {
      return this.getRecommendedTracks();
    }

    const cleanQuery = query.toLowerCase().trim();
    const results = [];

    // 1. Search local folder tracks
    const localMatches = localLib.search(cleanQuery);
    results.push(...localMatches);

    // 2. Search built-in catalog
    const demoMatches = DEMO_TRACKS.filter(t =>
      t.title.toLowerCase().includes(cleanQuery) ||
      t.artist.toLowerCase().includes(cleanQuery) ||
      t.album.toLowerCase().includes(cleanQuery)
    );
    results.push(...demoMatches);

    // 3. Online Search (YouTube Music & JioSaavn in parallel if network permits)
    const promises = [];
    if (filter === 'all' || filter === 'youtube') {
      promises.push(innertube.search(query).catch(() => []));
    }
    if (filter === 'all' || filter === 'jiosaavn') {
      promises.push(jiosaavn.searchSongs(query).catch(() => []));
    }

    const onlineHits = await Promise.allSettled(promises);
    for (const res of onlineHits) {
      if (res.status === 'fulfilled' && Array.isArray(res.value)) {
        results.push(...res.value);
      }
    }

    return results;
  }

  getRecommendedTracks() {
    // In folder mode, return only the connected folder's tracks
    if (this.exclusiveFolderMode || localLib.getFolder()) {
      return localLib.tracks;
    }
    return localLib.tracks.length > 0 ? localLib.tracks : DEMO_TRACKS;
  }

  async getSearchSuggestions(input) {
    if (!input || !input.trim()) return [];
    if (this.exclusiveFolderMode || localLib.getFolder()) {
      return localLib.tracks
        .filter(t => t.title.toLowerCase().includes(input.toLowerCase()))
        .map(t => t.title)
        .slice(0, 5);
    }

    try {
      const suggestions = await innertube.getSearchSuggestions(input);
      if (suggestions && suggestions.length > 0) return suggestions;
    } catch {
      // Fallback
    }

    return DEMO_TRACKS
      .filter(t => t.title.toLowerCase().includes(input.toLowerCase()))
      .map(t => t.title)
      .slice(0, 5);
  }
}

module.exports = new SourceManager();
