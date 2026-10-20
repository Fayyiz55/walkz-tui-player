const fs = require('fs');
const path = require('path');
const os = require('os');

class ConfigStore {
  constructor() {
    this.dir = path.join(os.homedir(), '.config', 'bitchord-tui');
    this.file = path.join(this.dir, 'config.json');
    this.defaults = {
      volume: 80,
      repeatMode: 'none', // 'none', 'track', 'all'
      shuffle: false,
      eqPreset: 'Flat',
      crossfadeSec: 3,
      playbackSpeed: 1.0,
      jamServerUrl: 'http://localhost:8000',
      activeTab: 0,
      lastPlayedId: null
    };
    this.data = { ...this.defaults };
    this.load();
  }

  load() {
    try {
      if (fs.existsSync(this.file)) {
        const raw = fs.readFileSync(this.file, 'utf8');
        this.data = { ...this.defaults, ...JSON.parse(raw) };
      }
    } catch {
      this.data = { ...this.defaults };
    }
  }

  save() {
    try {
      if (!fs.existsSync(this.dir)) {
        fs.mkdirSync(this.dir, { recursive: true });
      }
      fs.writeFileSync(this.file, JSON.stringify(this.data, null, 2), 'utf8');
    } catch {
      // Ignore write errors
    }
  }

  get(key) {
    return this.data[key];
  }

  set(key, value) {
    this.data[key] = value;
    this.save();
  }
}

module.exports = new ConfigStore();
