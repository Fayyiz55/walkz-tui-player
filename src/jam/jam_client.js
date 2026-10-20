const { EventEmitter } = require('events');

/**
 * BitChord "Listen Together" / Jam Protocol WebSocket Client
 * Implements NTP-style clock offset calculation and monotonic anchor playback sync
 */
class JamClient extends EventEmitter {
  constructor() {
    super();
    this.ws = null;
    this.token = null;
    this.partyCode = null;
    this.isHost = false;
    this.members = [];
    this.you = null;
    this.playback = null;
    this.queue = [];
    this.queueIndex = -1;
    this.queueSeq = 0;
    this.lastSeq = 0;
    this.hostOnlyControl = false;

    // Clock synchronization
    this.clockOffset = 0;
    this.bestRtt = Infinity;
    this.pingInterval = null;
  }

  /**
   * Connect to an existing or newly created BitChord party
   */
  async joinParty(serverUrl, code, user = { userId: "user_tui", displayName: "TUI Listener" }) {
    this.disconnect();
    this.partyCode = code.toUpperCase().trim();

    // 1. Join party via REST API endpoint
    const cleanServer = serverUrl.replace(/\/+$/, '');
    const joinUrl = `${cleanServer}/api/parties/${this.partyCode}/join`;

    const res = await fetch(joinUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        userId: user.userId,
        deviceId: `tui_device_${Math.random().toString(36).slice(2, 8)}`,
        displayName: user.displayName
      })
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.message || `Failed to join party (${res.status})`);
    }

    const data = await res.json();
    this.token = data.token;
    this.you = data.you;
    this.isHost = data.you?.isHost || false;

    // 2. Open WebSocket connection
    const wsUrl = cleanServer.replace(/^http/, 'ws') + `/ws/parties/${this.partyCode}`;
    this.connectSocket(wsUrl);
  }

  /**
   * Create a new party as host
   */
  async createParty(serverUrl, user = { userId: "user_host", displayName: "TUI Host" }) {
    this.disconnect();
    const cleanServer = serverUrl.replace(/\/+$/, '');
    const createUrl = `${cleanServer}/api/parties`;

    const res = await fetch(createUrl, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        userId: user.userId,
        deviceId: `tui_host_${Math.random().toString(36).slice(2, 8)}`,
        displayName: user.displayName,
        maxMembers: 5
      })
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.message || `Failed to create party (${res.status})`);
    }

    const data = await res.json();
    this.partyCode = data.code;
    this.token = data.token;
    this.you = data.you;
    this.isHost = true;

    const wsUrl = cleanServer.replace(/^http/, 'ws') + `/ws/parties/${this.partyCode}`;
    this.connectSocket(wsUrl);
    return this.partyCode;
  }

  connectSocket(wsUrl) {
    try {
      // Browser/Node subprotocol carries token when headers cannot be set
      const subprotocol = `bitchord.token.${this.token}`;
      this.ws = new WebSocket(wsUrl, ["bitchord.v1", subprotocol]);

      this.ws.onopen = () => {
        this.emit('connected', this.partyCode);
        this.startPingLoop();
        this.sendSync();
      };

      this.ws.onmessage = (event) => {
        try {
          const frame = JSON.parse(event.data);
          this.handleFrame(frame);
        } catch {
          // Ignore malformed frames
        }
      };

      this.ws.onclose = () => {
        this.stopPingLoop();
        this.emit('disconnected');
      };

      this.ws.onerror = (err) => {
        this.emit('error', err);
      };
    } catch (e) {
      this.emit('error', e);
    }
  }

  startPingLoop() {
    this.stopPingLoop();
    this.sendPing();
    this.pingInterval = setInterval(() => this.sendPing(), 4000);
  }

  stopPingLoop() {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
  }

  sendPing() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({
        type: "ping",
        clientMs: Date.now()
      }));
    }
  }

  sendSync() {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: "sync" }));
      this.ws.send(JSON.stringify({ type: "syncQueue" }));
    }
  }

  sendControl(action, payload = {}) {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
    this.ws.send(JSON.stringify({
      type: "control",
      action,
      ...payload
    }));
  }

  handleFrame(frame) {
    switch (frame.type) {
      case "pong": {
        const t0 = frame.clientMs;
        const ts = frame.serverMs;
        const t1 = Date.now();
        const rtt = t1 - t0;
        if (rtt < this.bestRtt) {
          this.bestRtt = rtt;
          this.clockOffset = Math.round(ts - (t0 + t1) / 2);
          this.emit('clockSync', { offset: this.clockOffset, rtt: this.bestRtt });
        }
        break;
      }

      case "welcome": {
        this.you = frame.you;
        if (frame.party) {
          if (frame.party.playback) this.applyPlayback(frame.party.playback);
          if (frame.party.queue) this.applyQueue(frame.party.queue);
          if (frame.party.members) this.members = frame.party.members;
          this.hostOnlyControl = !!frame.party.hostOnlyControl;
        }
        this.emit('welcome', frame);
        break;
      }

      case "state": {
        if (frame.playback) {
          this.applyPlayback(frame.playback);
        }
        break;
      }

      case "queue": {
        if (frame.queue) {
          this.applyQueue(frame.queue);
        }
        break;
      }

      case "members": {
        this.members = frame.members || [];
        this.emit('membersUpdate', this.members);
        break;
      }

      case "error": {
        this.emit('serverError', frame.message || frame.error);
        break;
      }

      case "bye": {
        this.disconnect();
        this.emit('kicked', frame.reason);
        break;
      }
    }
  }

  applyPlayback(state) {
    // Sequence check: ignore frames with seq <= last applied seq
    if (state.seq !== undefined && state.seq <= this.lastSeq) {
      return;
    }
    this.lastSeq = state.seq || 0;
    this.playback = state;

    // Calculate current playhead based on anchor
    const serverNow = Date.now() + this.clockOffset;
    let playheadMs = state.positionMs || 0;
    if (state.isPlaying && state.anchorMs) {
      const elapsed = Math.max(0, serverNow - state.anchorMs);
      playheadMs += elapsed;
      if (state.track && state.track.durationMs && playheadMs > state.track.durationMs) {
        playheadMs = state.track.durationMs;
      }
    }

    this.emit('playbackSync', {
      track: state.track,
      isPlaying: state.isPlaying,
      positionMs: playheadMs,
      seq: state.seq
    });
  }

  applyQueue(queueObj) {
    this.queue = queueObj.items || [];
    this.queueIndex = queueObj.index !== undefined ? queueObj.index : -1;
    this.queueSeq = queueObj.seq || 0;
    this.emit('queueSync', {
      items: this.queue,
      index: this.queueIndex,
      seq: this.queueSeq
    });
  }

  disconnect() {
    this.stopPingLoop();
    if (this.ws) {
      try {
        this.ws.close();
      } catch {
        // Ignore
      }
      this.ws = null;
    }
    this.token = null;
    this.partyCode = null;
  }
}

module.exports = new JamClient();
