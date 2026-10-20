const http = require('http');
const crypto = require('crypto');

/**
 * Built-in BitChord Jam Party Server
 * Pure Node.js standard library implementation of backend/main.go & backend/party/party.go
 */

const JAM_PLAY_LEAD_MS = 350;
const JAM_STATE_HEARTBEAT_MS = 5000;
const JAM_MAX_MEMBERS = 5;

function generateCode() {
  const chars = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // readable code without 0/O/1/I
  let code = "";
  for (let i = 0; i < 6; i++) {
    code += chars[Math.floor(Math.random() * chars.length)];
  }
  return code;
}

class PartyRoom {
  constructor(code, hostUser) {
    this.code = code;
    this.hostMemberId = null;
    this.members = new Map(); // token -> member
    this.sockets = new Map(); // token -> socket
    this.hostOnlyControl = false;
    this.maxMembers = JAM_MAX_MEMBERS;
    this.createdAt = Date.now();

    const now = Date.now();
    this.playback = {
      track: null,
      isPlaying: false,
      positionMs: 0,
      anchorMs: now,
      seq: 1,
      queueSeq: 1,
      updatedBy: null,
      updatedAtMs: now
    };

    this.queue = [];
    this.queueIndex = -1;
    this.heartbeatTimer = setInterval(() => this.broadcastHeartbeat(), JAM_STATE_HEARTBEAT_MS);
  }

  addMember(user, isHost = false) {
    const token = crypto.randomBytes(16).toString('hex');
    const memberId = `mem_${crypto.randomBytes(6).toString('hex')}`;
    const member = {
      id: memberId,
      userId: user.userId || "anon",
      deviceId: user.deviceId || "dev",
      displayName: user.displayName || "Listener",
      isHost,
      token
    };
    this.members.set(token, member);
    if (isHost) this.hostMemberId = memberId;
    return member;
  }

  touch() {
    this.playback.seq++;
    this.playback.updatedAtMs = Date.now();
  }

  touchQueue() {
    this.playback.queueSeq++;
  }

  broadcast(frame, excludeToken = null) {
    const data = Buffer.from(JSON.stringify(frame), 'utf8');
    const msg = this.buildWsFrame(data);
    for (const [token, socket] of this.sockets.entries()) {
      if (token !== excludeToken && !socket.destroyed) {
        try {
          socket.write(msg);
        } catch {
          // Socket write error
        }
      }
    }
  }

  broadcastHeartbeat() {
    const frame = {
      type: "state",
      playback: this.playback,
      serverMs: Date.now()
    };
    this.broadcast(frame);
  }

  buildWsFrame(data) {
    const length = data.length;
    let header;
    if (length < 126) {
      header = Buffer.from([0x81, length]);
    } else if (length <= 65535) {
      header = Buffer.alloc(4);
      header[0] = 0x81;
      header[1] = 126;
      header.writeUInt16BE(length, 2);
    } else {
      header = Buffer.alloc(10);
      header[0] = 0x81;
      header[1] = 127;
      header.writeBigUInt64BE(BigInt(length), 2);
    }
    return Buffer.concat([header, data]);
  }

  destroy() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }
}

class JamServer {
  constructor(port = 8000) {
    this.port = port;
    this.parties = new Map(); // code -> PartyRoom
    this.server = null;
  }

  start() {
    this.server = http.createServer((req, res) => this.handleHttp(req, res));

    // Handle WebSocket upgrade manually
    this.server.on('upgrade', (req, socket, head) => this.handleUpgrade(req, socket, head));

    this.server.listen(this.port, () => {
      // Jam server listening
    });
    return this;
  }

  stop() {
    for (const party of this.parties.values()) {
      party.destroy();
    }
    if (this.server) {
      this.server.close();
    }
  }

  handleHttp(req, res) {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const pathname = url.pathname;

    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type, Authorization');

    if (req.method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    if (pathname === '/healthz') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ status: 'ok', parties: this.parties.size }));
      return;
    }

    if (pathname === '/api/time') {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ serverMs: Date.now() }));
      return;
    }

    if (pathname === '/api/parties' && req.method === 'POST') {
      let body = '';
      req.on('data', chunk => body += chunk);
      req.on('end', () => {
        try {
          const user = JSON.parse(body || '{}');
          const code = generateCode();
          const party = new PartyRoom(code, user);
          const member = party.addMember(user, true);
          this.parties.set(code, party);

          res.writeHead(201, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({
            code,
            token: member.token,
            you: member,
            party: {
              code,
              playback: party.playback,
              queue: { seq: party.playback.queueSeq, index: party.queueIndex, items: party.queue },
              members: Array.from(party.members.values()),
              hostOnlyControl: party.hostOnlyControl
            }
          }));
        } catch (e) {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'invalid_json', message: e.message }));
        }
      });
      return;
    }

    const joinMatch = pathname.match(/^\/api\/parties\/([A-Za-z0-9]+)\/join$/);
    if (joinMatch && req.method === 'POST') {
      const code = joinMatch[1].toUpperCase();
      const party = this.parties.get(code);
      if (!party) {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'not_found', message: 'Party code not found' }));
        return;
      }

      let body = '';
      req.on('data', chunk => body += chunk);
      req.on('end', () => {
        try {
          const user = JSON.parse(body || '{}');
          if (party.members.size >= party.maxMembers) {
            res.writeHead(409, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: 'party_full', message: 'Party is full' }));
            return;
          }
          const member = party.addMember(user, false);
          res.writeHead(200, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({
            code,
            token: member.token,
            you: member,
            party: {
              code,
              playback: party.playback,
              queue: { seq: party.playback.queueSeq, index: party.queueIndex, items: party.queue },
              members: Array.from(party.members.values()),
              hostOnlyControl: party.hostOnlyControl
            }
          }));
        } catch (e) {
          res.writeHead(400, { 'Content-Type': 'application/json' });
          res.end(JSON.stringify({ error: 'invalid_json', message: e.message }));
        }
      });
      return;
    }

    res.writeHead(404);
    res.end();
  }

  handleUpgrade(req, socket, head) {
    const url = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const match = url.pathname.match(/^\/ws\/parties\/([A-Za-z0-9]+)$/);
    if (!match) {
      socket.destroy();
      return;
    }

    const code = match[1].toUpperCase();
    const party = this.parties.get(code);
    if (!party) {
      socket.destroy();
      return;
    }

    // Extract token from Sec-WebSocket-Protocol (bitchord.token.<token>) or Authorization header
    const protocolHeader = req.headers['sec-websocket-protocol'] || '';
    const authHeader = req.headers['authorization'] || '';
    let token = null;

    const tokenMatch = protocolHeader.match(/bitchord\.token\.([a-f0-9]+)/i);
    if (tokenMatch) {
      token = tokenMatch[1];
    } else if (authHeader.startsWith('Bearer ')) {
      token = authHeader.slice(7).trim();
    }

    if (!token || !party.members.has(token)) {
      socket.destroy();
      return;
    }

    // Complete standard WebSocket HTTP handshake
    const key = req.headers['sec-websocket-key'];
    const accept = crypto.createHash('sha1')
      .update(key + '258EAFA5-E914-47DA-95CA-C5AB0DC85B11')
      .digest('base64');

    const headers = [
      'HTTP/1.1 101 Switching Protocols',
      'Upgrade: websocket',
      'Connection: Upgrade',
      `Sec-WebSocket-Accept: ${accept}`,
      'Sec-WebSocket-Protocol: bitchord.v1'
    ];
    socket.write(headers.join('\r\n') + '\r\n\r\n');

    party.sockets.set(token, socket);
    const member = party.members.get(token);

    // Send Welcome Frame
    const welcomeFrame = {
      type: "welcome",
      you: member,
      party: {
        code,
        playback: party.playback,
        queue: { seq: party.playback.queueSeq, index: party.queueIndex, items: party.queue },
        members: Array.from(party.members.values()),
        hostOnlyControl: party.hostOnlyControl
      },
      serverMs: Date.now()
    };
    socket.write(party.buildWsFrame(Buffer.from(JSON.stringify(welcomeFrame))));

    // Broadcast updated member list
    party.broadcast({
      type: "members",
      members: Array.from(party.members.values()),
      maxMembers: party.maxMembers,
      serverMs: Date.now()
    });

    // Wire packet parser
    this.attachSocketHandler(socket, party, token);
  }

  attachSocketHandler(socket, party, token) {
    let buffer = Buffer.alloc(0);

    socket.on('data', chunk => {
      buffer = Buffer.concat([buffer, chunk]);
      while (buffer.length >= 2) {
        const opcode = buffer[0] & 0x0F;
        const isMasked = (buffer[1] & 0x80) !== 0;
        let payloadLen = buffer[1] & 0x7F;
        let offset = 2;

        if (payloadLen === 126) {
          if (buffer.length < 4) break;
          payloadLen = buffer.readUInt16BE(2);
          offset = 4;
        } else if (payloadLen === 127) {
          if (buffer.length < 10) break;
          payloadLen = Number(buffer.readBigUInt64BE(2));
          offset = 10;
        }

        let mask = null;
        if (isMasked) {
          if (buffer.length < offset + 4) break;
          mask = buffer.subarray(offset, offset + 4);
          offset += 4;
        }

        if (buffer.length < offset + payloadLen) break;

        const payload = buffer.subarray(offset, offset + payloadLen);
        buffer = buffer.subarray(offset + payloadLen);

        if (mask) {
          for (let i = 0; i < payload.length; i++) {
            payload[i] ^= mask[i % 4];
          }
        }

        if (opcode === 0x08) { // Close
          socket.destroy();
          return;
        }

        if (opcode === 0x01) { // Text
          try {
            const frame = JSON.parse(payload.toString('utf8'));
            this.processPartyMessage(frame, party, token, socket);
          } catch {
            // Ignore
          }
        }
      }
    });

    socket.on('close', () => {
      party.sockets.delete(token);
      party.members.delete(token);
      party.broadcast({
        type: "members",
        members: Array.from(party.members.values()),
        maxMembers: party.maxMembers,
        serverMs: Date.now()
      });
      if (party.members.size === 0) {
        this.parties.delete(party.code);
        party.destroy();
      }
    });
  }

  processPartyMessage(frame, party, token, socket) {
    const member = party.members.get(token);
    if (!member) return;

    if (frame.type === "ping") {
      const pong = {
        type: "pong",
        clientMs: frame.clientMs,
        serverMs: Date.now()
      };
      socket.write(party.buildWsFrame(Buffer.from(JSON.stringify(pong))));
      return;
    }

    if (frame.type === "sync") {
      const state = {
        type: "state",
        playback: party.playback,
        serverMs: Date.now()
      };
      socket.write(party.buildWsFrame(Buffer.from(JSON.stringify(state))));
      return;
    }

    if (frame.type === "syncQueue") {
      const qFrame = {
        type: "queue",
        queue: { seq: party.playback.queueSeq, index: party.queueIndex, items: party.queue },
        serverMs: Date.now()
      };
      socket.write(party.buildWsFrame(Buffer.from(JSON.stringify(qFrame))));
      return;
    }

    if (frame.type === "control") {
      const action = frame.action;
      if (party.hostOnlyControl && !member.isHost) {
        const err = { type: "error", error: "host_only", message: "Only the host can control playback" };
        socket.write(party.buildWsFrame(Buffer.from(JSON.stringify(err))));
        return;
      }

      const now = Date.now();
      switch (action) {
        case "play": {
          party.playback.isPlaying = true;
          party.playback.positionMs = frame.positionMs !== undefined ? frame.positionMs : party.playback.positionMs;
          party.playback.anchorMs = now + JAM_PLAY_LEAD_MS;
          party.touch();
          party.broadcast({ type: "state", playback: party.playback, serverMs: now });
          break;
        }
        case "pause": {
          party.playback.isPlaying = false;
          party.playback.positionMs = frame.positionMs !== undefined ? frame.positionMs : party.playback.positionMs;
          party.playback.anchorMs = now;
          party.touch();
          party.broadcast({ type: "state", playback: party.playback, serverMs: now });
          break;
        }
        case "seek": {
          party.playback.positionMs = frame.positionMs || 0;
          party.playback.anchorMs = now + (party.playback.isPlaying ? JAM_PLAY_LEAD_MS : 0);
          party.touch();
          party.broadcast({ type: "state", playback: party.playback, serverMs: now });
          break;
        }
        case "setTrack": {
          party.playback.track = frame.track;
          party.playback.positionMs = 0;
          party.playback.isPlaying = frame.isPlaying !== undefined ? frame.isPlaying : true;
          party.playback.anchorMs = now + JAM_PLAY_LEAD_MS;
          party.touch();
          party.broadcast({ type: "state", playback: party.playback, serverMs: now });
          break;
        }
        case "queueAdd": {
          if (Array.isArray(frame.tracks)) {
            party.queue.push(...frame.tracks);
            party.touchQueue();
            party.broadcast({
              type: "queue",
              queue: { seq: party.playback.queueSeq, index: party.queueIndex, items: party.queue },
              serverMs: now
            });
          }
          break;
        }
        case "setHostOnlyControl": {
          if (member.isHost) {
            party.hostOnlyControl = !!frame.enabled;
            party.broadcast({
              type: "members",
              members: Array.from(party.members.values()),
              maxMembers: party.maxMembers,
              serverMs: now
            });
          }
          break;
        }
      }
    }
  }
}

module.exports = JamServer;

// Start standalone if invoked directly
if (require.main === module) {
  const port = parseInt(process.env.PORT || '8000', 10);
  const server = new JamServer(port);
  server.start();
  console.log(`BitChord Jam Server listening on http://localhost:${port}`);
}
