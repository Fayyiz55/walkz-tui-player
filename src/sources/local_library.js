const fs = require('fs');
const path = require('path');
const os = require('os');
const { execFile } = require('child_process');

const SUPPORTED_EXTENSIONS = new Set(['.mp3', '.flac', '.wav', '.m4a', '.ogg', '.opus', '.aac', '.alac', '.wma']);

class LocalLibrary {
  constructor() {
    this.connectedFolder = null;
    this.tracks = [];
  }

  /**
   * Set an exclusive folder for the player to access
   */
  setFolder(folderPath) {
    if (!folderPath) {
      this.connectedFolder = null;
      return null;
    }
    // Expand tilde if present
    let resolved = folderPath;
    if (resolved.startsWith('~')) {
      resolved = path.join(os.homedir(), resolved.slice(1));
    }
    resolved = path.resolve(process.cwd(), resolved);
    if (!fs.existsSync(resolved) || !fs.statSync(resolved).isDirectory()) {
      throw new Error(`Directory does not exist: ${folderPath}`);
    }
    this.connectedFolder = resolved;
    return this.connectedFolder;
  }

  getFolder() {
    return this.connectedFolder;
  }

  /**
   * Scan folder recursively and extract audio files
   */
  async scanDirectory(targetDir = null, maxDepth = 4) {
    const dir = targetDir || this.connectedFolder || path.join(os.homedir(), 'Music');
    if (!fs.existsSync(dir)) return [];

    const foundFiles = [];
    const walk = (currentDir, depth) => {
      if (depth > maxDepth) return;
      try {
        const entries = fs.readdirSync(currentDir, { withFileTypes: true });
        for (const entry of entries) {
          const fullPath = path.join(currentDir, entry.name);
          if (entry.isDirectory() && !entry.name.startsWith('.')) {
            walk(fullPath, depth + 1);
          } else if (entry.isFile()) {
            const ext = path.extname(entry.name).toLowerCase();
            if (SUPPORTED_EXTENSIONS.has(ext)) {
              foundFiles.push({ fullPath, name: entry.name, ext });
            }
          }
        }
      } catch {
        // Skip inaccessible subdirectories
      }
    };

    walk(dir, 0);

    // Process files and extract tags
    const tracks = [];
    for (const f of foundFiles) {
      const track = await this.createLocalTrack(f.fullPath, f.name, f.ext);
      tracks.push(track);
    }

    this.tracks = tracks;
    return tracks;
  }

  probeAudio(filePath) {
    return new Promise((resolve) => {
      execFile('ffprobe', [
        '-v', 'quiet',
        '-print_format', 'json',
        '-show_format',
        '-show_streams',
        filePath
      ], (err, stdout) => {
        if (err || !stdout) return resolve(null);
        try {
          const data = JSON.parse(stdout);
          const format = data.format || {};
          const stream = (data.streams || []).find(s => s.codec_type === 'audio') || {};
          const tags = format.tags || {};
          resolve({
            durationMs: Math.round(parseFloat(format.duration || 0) * 1000),
            title: tags.title || tags.TITLE || null,
            artist: tags.artist || tags.ARTIST || null,
            album: tags.album || tags.ALBUM || null,
            bitrate: format.bit_rate ? Math.round(parseInt(format.bit_rate, 10) / 1000) + ' kbps' : null,
            sampleRate: stream.sample_rate ? stream.sample_rate + ' Hz' : null,
            codec: stream.codec_name || null
          });
        } catch {
          resolve(null);
        }
      });
    });
  }

  async createLocalTrack(filePath, fileName, ext) {
    const nameWithoutExt = path.basename(fileName, ext);
    let title = nameWithoutExt;
    let artist = "Local Artist";
    let album = path.basename(path.dirname(filePath)) || "Local Music";

    // Parse "Artist - Title" filename convention if present
    if (nameWithoutExt.includes(' - ')) {
      const parts = nameWithoutExt.split(' - ');
      artist = parts[0].trim();
      title = parts.slice(1).join(' - ').trim();
    }

    let stats = { size: 0 };
    try {
      stats = fs.statSync(filePath);
    } catch {
      // Ignore
    }

    // Attempt probe for exact ID3 tags and duration
    let durationMs = 210000;
    let bitrate = ext === '.flac' || ext === '.wav' || ext === '.alac' ? "Lossless" : "320 kbps";
    let sampleRate = "44100 Hz";
    let codec = ext.replace('.', '');

    const probe = await this.probeAudio(filePath);
    if (probe) {
      if (probe.durationMs && probe.durationMs > 0) durationMs = probe.durationMs;
      if (probe.title) title = probe.title;
      if (probe.artist) artist = probe.artist;
      if (probe.album) album = probe.album;
      if (probe.bitrate) bitrate = probe.bitrate;
      if (probe.sampleRate) sampleRate = probe.sampleRate;
      if (probe.codec) codec = probe.codec;
    } else {
      // Fallback estimate
      if (ext === '.wav') durationMs = Math.max(10, Math.round(stats.size / (44100 * 2 * 2))) * 1000;
      else if (ext === '.mp3') durationMs = Math.max(10, Math.round((stats.size * 8) / (320 * 1000))) * 1000;
    }

    // Check for companion .lrc file
    let lyrics = null;
    const companionLrc = filePath.replace(/\.[^.]+$/, '.lrc');
    if (fs.existsSync(companionLrc)) {
      try {
        lyrics = fs.readFileSync(companionLrc, 'utf8');
      } catch {
        // Ignore read errors
      }
    }

    return {
      id: `local_${Buffer.from(filePath).toString('hex').slice(0, 16)}`,
      title,
      artist,
      album,
      durationMs,
      localPath: filePath,
      lyrics,
      source: "Local Folder",
      codec,
      bitrate,
      sampleRate,
      loudnessDb: -1.0
    };
  }

  search(query) {
    if (!query || !query.trim()) return this.tracks;
    const q = query.toLowerCase().trim();
    return this.tracks.filter(t =>
      t.title.toLowerCase().includes(q) ||
      t.artist.toLowerCase().includes(q) ||
      t.album.toLowerCase().includes(q)
    );
  }
}

module.exports = new LocalLibrary();
