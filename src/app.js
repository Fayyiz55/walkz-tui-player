const Terminal = require('./tui/terminal');
const Components = require('./tui/components');
const NowPlayingView = require('./tui/views/now_playing');
const SearchView = require('./tui/views/search');
const QueueView = require('./tui/views/queue');
const JamPartyView = require('./tui/views/jam_party');
const EqualizerView = require('./tui/views/equalizer');
const LibraryView = require('./tui/views/library');

const audio = require('./player/audio_controller');
const { DspManager, EQ_PRESETS } = require('./player/dsp');
const lyricsService = require('./lyrics/lyrics_service');
const sourceManager = require('./sources/source_manager');
const localLib = require('./sources/local_library');
const jamClient = require('./jam/jam_client');
const JamServer = require('./jam/jam_server');
const config = require('./data/config_store');
const { DEMO_TRACKS } = require('./data/demo_tracks');

class App {
  constructor(options = {}) {
    this.options = options;
    this.terminal = new Terminal();
    this.activeTab = config.get('activeTab') || 0; // 0: Now Playing, 1: Search, 2: Queue, 3: Jam, 4: EQ, 5: Library
    this.queue = [...DEMO_TRACKS];
    this.currentIndex = 0;
    this.repeatMode = config.get('repeatMode') || 'none'; // 'none', 'track', 'all'
    this.shuffle = config.get('shuffle') || false;
    this.lyrics = [];
    this.history = [];

    // Search state
    this.searchQuery = "";
    this.isSearchingInput = false;
    this.isSearching = false;
    this.searchResults = sourceManager.getRecommendedTracks();
    this.searchSelectedIndex = 0;
    this.searchFilter = "all";
    this.searchSuggestions = [];

    // Queue view state
    this.queueSelectedIndex = 0;

    // Jam party view state
    this.jamInputMode = null; // null | 'join'
    this.jamInputVal = "";
    this.jamServer = null;
    this.jamServerRunning = false;

    // Equalizer view state
    this.selectedEqBand = 0;

    // Library view state
    this.localTracks = [];
    this.librarySelectedIndex = 0;
    this.isScanningLibrary = false;
    this.isInputtingFolder = false;
    this.folderInputVal = "";

    // Render loop
    this.renderTimer = null;
    this.statusMessage = null;
    this.statusTimeout = null;

    this.bindEvents();
  }

  bindEvents() {
    this.terminal.on('keypress', (str, key) => this.handleKeypress(str, key));
    this.terminal.on('resize', () => this.draw());

    audio.on('stateChange', () => this.draw());
    audio.on('volumeChange', (vol) => {
      config.set('volume', vol);
      this.draw();
    });
    audio.on('trackEnd', () => this.handleTrackEnd());
    audio.on('audioNotice', (msg) => this.setStatus(msg, 4000));

    // Jam Client events
    jamClient.on('playbackSync', (sync) => {
      if (sync.track) {
        audio.play(sync.track, sync.positionMs);
        if (!sync.isPlaying) audio.pause();
      }
    });
    jamClient.on('connected', () => {
      this.setStatus("Connected to BitChord Jam Party!");
      this.draw();
    });
    jamClient.on('disconnected', () => {
      this.setStatus("Disconnected from Jam Party");
      this.draw();
    });
  }

  async start() {
    this.terminal.init();
    audio.setVolume(config.get('volume') || 80);

    if (this.options.driver) {
      audio.setAudioDriver(this.options.driver);
    }

    // If user provided a folder to connect to:
    if (this.options.folder) {
      await this.connectFolder(this.options.folder, true);
    } else {
      if (this.queue.length > 0) {
        this.loadTrack(this.queue[0], false);
      }
      this.scanLocalLibrary();
    }

    // Redraw loop (10 fps for smooth visualizer and lyrics animation)
    this.renderTimer = setInterval(() => this.draw(), 100);
    this.draw();
  }

  async connectFolder(folderPath, autoPlay = true) {
    this.isScanningLibrary = true;
    this.draw();
    try {
      const resolved = localLib.setFolder(folderPath);
      sourceManager.setExclusiveFolder(true);
      const tracks = await localLib.scanDirectory(resolved);
      this.localTracks = tracks;
      this.isScanningLibrary = false;

      if (tracks.length > 0) {
        // Exclusively replace queue with ONLY files from this folder
        this.queue = [...tracks];
        this.currentIndex = 0;
        this.searchResults = [...tracks];
        this.loadTrack(this.queue[0], autoPlay);
        const folderName = require('path').basename(resolved);
        this.setStatus(`Connected: 📁 ${folderName} (${tracks.length} songs)`);
      } else {
        this.setStatus(`No audio files found in ${folderPath}`);
      }
    } catch (e) {
      this.isScanningLibrary = false;
      this.setStatus(`Error: ${e.message}`);
    }
    this.draw();
  }

  async scanLocalLibrary() {
    this.isScanningLibrary = true;
    try {
      this.localTracks = await localLib.scanDirectory();
    } catch {
      // Ignore
    } finally {
      this.isScanningLibrary = false;
      this.draw();
    }
  }

  setStatus(msg, durationMs = 3000) {
    this.statusMessage = msg;
    if (this.statusTimeout) clearTimeout(this.statusTimeout);
    this.statusTimeout = setTimeout(() => {
      this.statusMessage = null;
      this.draw();
    }, durationMs);
  }

  async loadTrack(track, autoPlay = true) {
    if (!track) return;
    this.lyrics = [];
    lyricsService.getLyrics(track).then(l => {
      this.lyrics = l;
      this.draw();
    });

    if (autoPlay) {
      await audio.play(track, 0);
      this.history.unshift(track);
      if (this.history.length > 50) this.history.pop();
    } else {
      audio.currentTrack = track;
      audio.durationMs = track.durationMs || 180000;
      audio.positionMs = 0;
    }

    // Broadcast to Jam party if active host
    if (jamClient.partyCode && jamClient.isHost) {
      jamClient.sendControl('setTrack', {
        track: {
          videoId: track.id,
          title: track.title,
          artist: track.artist,
          durationMs: track.durationMs
        },
        isPlaying: autoPlay
      });
    }
    this.draw();
  }

  handleTrackEnd() {
    if (this.repeatMode === 'track') {
      audio.seek(0);
      audio.resume();
      return;
    }
    this.nextTrack();
  }

  nextTrack() {
    if (this.queue.length === 0) return;
    if (this.shuffle) {
      this.currentIndex = Math.floor(Math.random() * this.queue.length);
    } else {
      this.currentIndex++;
      if (this.currentIndex >= this.queue.length) {
        if (this.repeatMode === 'all') {
          this.currentIndex = 0;
        } else {
          this.currentIndex = this.queue.length - 1;
          audio.pause();
          return;
        }
      }
    }
    this.loadTrack(this.queue[this.currentIndex], true);
  }

  prevTrack() {
    if (this.queue.length === 0) return;
    if (audio.positionMs > 3000) {
      audio.seek(0);
      return;
    }
    this.currentIndex--;
    if (this.currentIndex < 0) {
      this.currentIndex = this.repeatMode === 'all' ? this.queue.length - 1 : 0;
    }
    this.loadTrack(this.queue[this.currentIndex], true);
  }

  handleKeypress(str, key) {
    // 1. Text input mode in Search view
    if (this.isSearchingInput) {
      if (key.name === 'return') {
        this.isSearchingInput = false;
        this.isSearching = true;
        this.draw();
        sourceManager.search(this.searchQuery, this.searchFilter).then(results => {
          this.searchResults = results;
          this.searchSelectedIndex = 0;
        }).catch(() => {
          this.searchResults = [];
        }).finally(() => {
          this.isSearching = false;
          this.draw();
        });
        return;
      } else if (key.name === 'escape') {
        this.isSearchingInput = false;
      } else if (key.name === 'tab') {
        // Tab in search input cycles filter instead of inserting raw \t
        const filterList = ["all", "youtube", "jiosaavn", "local"];
        const curr = filterList.indexOf(this.searchFilter);
        this.searchFilter = filterList[(curr + 1) % filterList.length];
      } else if (key.name === 'backspace') {
        this.searchQuery = this.searchQuery.slice(0, -1);
      } else if (str && str.length === 1 && !key.ctrl && !key.meta) {
        const code = str.charCodeAt(0);
        if (code >= 32 && code !== 127) {
          this.searchQuery += str;
        }
      }
      this.draw();
      return;
    }

    // 2. Text input mode in Jam Party view
    if (this.jamInputMode === 'join') {
      if (key.name === 'return') {
        const code = this.jamInputVal.trim();
        this.jamInputMode = null;
        if (code) {
          this.setStatus(`Joining party ${code}...`);
          jamClient.joinParty(config.get('jamServerUrl') || 'http://localhost:8000', code)
            .catch(err => this.setStatus(`Error: ${err.message}`));
        }
      } else if (key.name === 'escape') {
        this.jamInputMode = null;
      } else if (key.name === 'backspace') {
        this.jamInputVal = this.jamInputVal.slice(0, -1);
      } else if (str && str.length === 1 && str.match(/[A-Za-z0-9]/)) {
        if (this.jamInputVal.length < 6) this.jamInputVal += str.toUpperCase();
      }
      this.draw();
      return;
    }

    // 3. Text input mode in Library view (Connecting to folder)
    if (this.isInputtingFolder) {
      if (key.name === 'return') {
        const target = this.folderInputVal.trim();
        this.isInputtingFolder = false;
        if (target) {
          this.connectFolder(target, true);
        }
      } else if (key.name === 'escape') {
        this.isInputtingFolder = false;
      } else if (key.name === 'backspace') {
        this.folderInputVal = this.folderInputVal.slice(0, -1);
      } else if (str && str.length === 1 && !key.ctrl && !key.meta) {
        this.folderInputVal += str;
      }
      this.draw();
      return;
    }

    // 3. Tab Navigation (1 - 6)
    if (str >= '1' && str <= '6') {
      this.activeTab = parseInt(str, 10) - 1;
      config.set('activeTab', this.activeTab);
      this.draw();
      return;
    }

    // 4. Global Playback Shortcuts
    if (key.name === 'space') {
      audio.togglePlayPause();
      if (jamClient.partyCode) {
        jamClient.sendControl(audio.isPlaying ? 'play' : 'pause', { positionMs: audio.positionMs });
      }
      return;
    }

    if (str === 'n') {
      this.nextTrack();
      return;
    }

    if (str === 'p') {
      this.prevTrack();
      return;
    }

    if (str === 'q') {
      this.terminal.restore();
      if (this.jamServer) this.jamServer.stop();
      process.exit(0);
    }

    if (str === '/') {
      this.activeTab = 1; // Switch to Search tab
      this.isSearchingInput = true;
      this.draw();
      return;
    }

    if (str === 's' && this.activeTab !== 3) {
      this.shuffle = !this.shuffle;
      config.set('shuffle', this.shuffle);
      this.setStatus(`Shuffle: ${this.shuffle ? 'ON' : 'OFF'}`);
      this.draw();
      return;
    }

    if (str === 'r') {
      this.repeatMode = this.repeatMode === 'none' ? 'track' : this.repeatMode === 'track' ? 'all' : 'none';
      config.set('repeatMode', this.repeatMode);
      this.setStatus(`Repeat: ${this.repeatMode.toUpperCase()}`);
      this.draw();
      return;
    }

    if (key.name === 'left') {
      audio.seekRelative(-5000);
      if (jamClient.partyCode) jamClient.sendControl('seek', { positionMs: audio.positionMs });
      return;
    }

    if (key.name === 'right') {
      audio.seekRelative(5000);
      if (jamClient.partyCode) jamClient.sendControl('seek', { positionMs: audio.positionMs });
      return;
    }

    // 5. Tab-specific controls
    switch (this.activeTab) {
      case 1: // Search View
        if (key.name === 'tab') {
          const filterList = ["all", "youtube", "jiosaavn", "local"];
          const curr = filterList.indexOf(this.searchFilter);
          this.searchFilter = filterList[(curr + 1) % filterList.length];
          this.isSearching = true;
          this.draw();
          sourceManager.search(this.searchQuery, this.searchFilter).then(results => {
            this.searchResults = results;
            this.searchSelectedIndex = 0;
          }).catch(() => {
            this.searchResults = [];
          }).finally(() => {
            this.isSearching = false;
            this.draw();
          });
          return;
        } else if (key.name === 'up') {
          this.searchSelectedIndex = Math.max(0, this.searchSelectedIndex - 1);
        } else if (key.name === 'down') {
          this.searchSelectedIndex = Math.min(Math.max(0, this.searchResults.length - 1), this.searchSelectedIndex + 1);
        } else if (key.name === 'return') {
          const selected = this.searchResults[this.searchSelectedIndex];
          if (selected) {
            this.queue.splice(this.currentIndex + 1, 0, selected);
            this.currentIndex++;
            this.loadTrack(selected, true);
          }
        } else if (str === 'a') {
          const selected = this.searchResults[this.searchSelectedIndex];
          if (selected) {
            this.queue.push(selected);
            this.setStatus(`Added "${selected.title}" to queue`);
          }
        }
        break;

      case 2: // Queue View
        if (key.name === 'up') {
          this.queueSelectedIndex = Math.max(0, this.queueSelectedIndex - 1);
        } else if (key.name === 'down') {
          this.queueSelectedIndex = Math.min(this.queue.length - 1, this.queueSelectedIndex + 1);
        } else if (key.name === 'return') {
          this.currentIndex = this.queueSelectedIndex;
          this.loadTrack(this.queue[this.currentIndex], true);
        } else if (str === 'd') {
          if (this.queue.length > 0 && this.queueSelectedIndex < this.queue.length) {
            const removed = this.queue.splice(this.queueSelectedIndex, 1)[0];
            if (this.queueSelectedIndex < this.currentIndex) this.currentIndex--;
            this.setStatus(`Removed "${removed.title}" from queue`);
          }
        } else if (str === 'c') {
          this.queue = [];
          this.currentIndex = -1;
          audio.pause();
          this.setStatus("Cleared queue");
        } else if (str === 'J') {
          // Move down
          const idx = this.queueSelectedIndex;
          if (idx < this.queue.length - 1) {
            const temp = this.queue[idx];
            this.queue[idx] = this.queue[idx + 1];
            this.queue[idx + 1] = temp;
            this.queueSelectedIndex = idx + 1;
          }
        } else if (str === 'K') {
          // Move up
          const idx = this.queueSelectedIndex;
          if (idx > 0) {
            const temp = this.queue[idx];
            this.queue[idx] = this.queue[idx - 1];
            this.queue[idx - 1] = temp;
            this.queueSelectedIndex = idx - 1;
          }
        }
        break;

      case 3: // Jam Party View
        if (str === 'c') {
          this.setStatus("Creating Jam party room...");
          const serverUrl = config.get('jamServerUrl') || 'http://localhost:8000';
          jamClient.createParty(serverUrl)
            .then(code => this.setStatus(`Room Created! Code: ${code}`))
            .catch(err => this.setStatus(`Error: ${err.message}`));
        } else if (str === 'j') {
          this.jamInputMode = 'join';
          this.jamInputVal = "";
        } else if (str === 'l') {
          jamClient.disconnect();
          this.setStatus("Left party room");
        } else if (str === 's') {
          if (!this.jamServerRunning) {
            this.jamServer = new JamServer(8000);
            this.jamServer.start();
            this.jamServerRunning = true;
            this.setStatus("Local Jam Server started on port 8000");
          }
        } else if (str === 'h') {
          if (jamClient.partyCode && jamClient.isHost) {
            const nextVal = !jamClient.hostOnlyControl;
            jamClient.sendControl('setHostOnlyControl', { enabled: nextVal });
          }
        }
        break;

      case 4: // Equalizer View
        if (key.name === 'up') {
          this.selectedEqBand = Math.max(0, this.selectedEqBand - 1);
        } else if (key.name === 'down') {
          this.selectedEqBand = Math.min(9, this.selectedEqBand + 1);
        } else if (key.name === 'right') {
          DspManager.setBand(this.selectedEqBand, DspManager.bands[this.selectedEqBand] + 1);
        } else if (key.name === 'left') {
          DspManager.setBand(this.selectedEqBand, DspManager.bands[this.selectedEqBand] - 1);
        } else if (str === 'p') {
          const presets = Object.keys(EQ_PRESETS);
          const curr = presets.indexOf(DspManager.currentPreset);
          const next = presets[(curr + 1) % presets.length];
          DspManager.setPreset(next);
          this.setStatus(`Preset: ${next}`);
        } else if (str === 'x') {
          DspManager.crossfadeSec = Math.min(12, DspManager.crossfadeSec + 1);
        } else if (str === 'X') {
          DspManager.crossfadeSec = Math.max(0, DspManager.crossfadeSec - 1);
        } else if (str === '+') {
          DspManager.speed = Math.min(2.0, DspManager.speed + 0.1);
        } else if (str === '-') {
          DspManager.speed = Math.max(0.5, DspManager.speed - 0.1);
        }
        break;

      case 5: // Local Library View
        if (str === 'f' || str === 'F') {
          this.isInputtingFolder = true;
          this.folderInputVal = localLib.getFolder() || "";
        } else if (key.name === 'up') {
          this.librarySelectedIndex = Math.max(0, this.librarySelectedIndex - 1);
        } else if (key.name === 'down') {
          this.librarySelectedIndex = Math.min(this.localTracks.length - 1, this.librarySelectedIndex + 1);
        } else if (key.name === 'return') {
          const track = this.localTracks[this.librarySelectedIndex];
          if (track) {
            this.queue.push(track);
            this.currentIndex = this.queue.length - 1;
            this.loadTrack(track, true);
          }
        } else if (str === 'a') {
          const track = this.localTracks[this.librarySelectedIndex];
          if (track) {
            this.queue.push(track);
            this.setStatus(`Added "${track.title}" to queue`);
          }
        } else if (str === 'r') {
          this.scanLocalLibrary();
        }
        break;

      default:
        // Volume adjustment when in Now Playing or other tabs
        if (key.name === 'up') audio.volumeUp();
        if (key.name === 'down') audio.volumeDown();
        break;
    }

    this.draw();
  }

  draw() {
    const width = this.terminal.width;
    const height = this.terminal.height;
    const state = audio.getState();

    // 1. Render Top Header (2 lines)
    const headerStr = Components.renderHeader(
      width,
      this.activeTab,
      !!jamClient.partyCode,
      jamClient.partyCode || "",
      state.volume,
      localLib.getFolder()
    );

    // 2. Render Main Body View
    // Header is 2 lines, Footer is 4 lines -> Total overhead = 6 lines
    const bodyHeight = Math.max(4, height - 6);
    let bodyStr = "";

    switch (this.activeTab) {
      case 0:
        bodyStr = NowPlayingView.render(
          width,
          bodyHeight,
          state.track,
          state.isPlaying,
          state.positionMs,
          state.durationMs,
          this.lyrics,
          state.outputDeviceStatus
        );
        break;
      case 1:
        bodyStr = SearchView.render(
          width,
          bodyHeight,
          this.searchQuery,
          this.isSearchingInput,
          this.searchResults,
          this.searchSelectedIndex,
          this.searchFilter,
          this.searchSuggestions,
          this.isSearching
        );
        break;
      case 2:
        bodyStr = QueueView.render(
          width,
          bodyHeight,
          this.queue,
          this.currentIndex,
          this.queueSelectedIndex,
          this.history,
          this.repeatMode,
          this.shuffle
        );
        break;
      case 3:
        bodyStr = JamPartyView.render(
          width,
          bodyHeight,
          jamClient,
          this.jamServerRunning,
          this.jamInputMode,
          this.jamInputVal
        );
        break;
      case 4:
        bodyStr = EqualizerView.render(
          width,
          bodyHeight,
          DspManager,
          this.selectedEqBand
        );
        break;
      case 5:
        bodyStr = LibraryView.render(
          width,
          bodyHeight,
          this.localTracks,
          this.librarySelectedIndex,
          this.isScanningLibrary,
          localLib.getFolder(),
          this.isInputtingFolder,
          this.folderInputVal
        );
        break;
    }

    // 3. Render Bottom Global Footer (4 lines)
    const footerStr = Components.renderFooter(
      width,
      state.track,
      state.isPlaying,
      state.positionMs,
      state.durationMs,
      this.repeatMode,
      this.shuffle
    );

    // Merge composite frame and strictly clamp rows to height
    const composite = `${headerStr}\n${bodyStr}\n${footerStr}`;
    const allRows = composite.split('\n');
    this.terminal.render(allRows.slice(0, height));
  }
}

module.exports = App;
