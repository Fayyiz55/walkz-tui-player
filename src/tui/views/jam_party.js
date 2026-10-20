const Terminal = require('../terminal');
const Components = require('../components');

class JamPartyView {
  static render(width, height, jamClient, jamServerRunning, inputMode, inputVal) {
    const { Primary, Secondary, Text, TextMuted, Border, Accent, Success, Warning, Red } = Terminal.Colors;
    const reset = Terminal.reset();
    const rows = [];

    const boxWidth = Math.max(20, width - 1);
    const innerWidth = Math.max(10, boxWidth - 4);
    const innerHeight = Math.max(1, height - 2);

    rows.push(Terminal.truncate(`${Terminal.fg(...Accent)}${Terminal.bold("LISTEN TOGETHER (BITCHORD JAM)")}${reset}`, innerWidth));
    rows.push(`${Terminal.fg(...Border)}${'─'.repeat(innerWidth)}${reset}`);

    const isConnected = !!jamClient.partyCode;

    if (!isConnected) {
      rows.push(Terminal.truncate(`Status: ${Terminal.fg(...Red)}Disconnected${reset}`, innerWidth));
      rows.push("");

      if (inputMode === 'join') {
        const cursor = `${Terminal.bg(255, 255, 255)}${Terminal.fg(0, 0, 0)} ${reset}`;
        rows.push(Terminal.truncate(`${Terminal.fg(...Secondary)}Code:${reset} ${Terminal.bold(inputVal)}${cursor}`, innerWidth));
        rows.push(Terminal.truncate(`${Terminal.fg(...TextMuted)}Press [Enter] to join, [Esc] to cancel${reset}`, innerWidth));
      } else {
        rows.push(Terminal.truncate(`${Terminal.fg(...Primary)}[c] Create Party Room${reset}  (Host a room)`, innerWidth));
        rows.push(Terminal.truncate(`${Terminal.fg(...Secondary)}[j] Join Party Room${reset}    (Enter 6-char code)`, innerWidth));
        rows.push("");
        const srvStatus = jamServerRunning ? `${Terminal.fg(...Success)}Active (port 8000)${reset}` : `${Terminal.fg(...TextMuted)}Stopped (Press [s] to launch)${reset}`;
        rows.push(Terminal.truncate(`Local Jam Server: ${srvStatus}`, innerWidth));
      }
    } else {
      rows.push(Terminal.truncate(`PARTY CODE: ${Terminal.fg(...Secondary)}${Terminal.bold(jamClient.partyCode)}${reset} │ Role: ${jamClient.isHost ? "👑 Host" : "🎧 Member"}`, innerWidth));

      const rttStr = jamClient.bestRtt < Infinity ? `${jamClient.bestRtt}ms` : "Syncing...";
      const offsetStr = `${jamClient.clockOffset >= 0 ? '+' : ''}${jamClient.clockOffset}ms`;
      rows.push(Terminal.truncate(`${Terminal.fg(...TextMuted)}NTP Skew: ${offsetStr} │ RTT: ${rttStr} │ Lead: 350ms${reset}`, innerWidth));
      rows.push("");

      rows.push(Terminal.truncate(`${Terminal.fg(...Secondary)}Members:${reset}`, innerWidth));
      const members = jamClient.members.length > 0 ? jamClient.members : [jamClient.you].filter(Boolean);
      members.slice(0, Math.max(1, innerHeight - 8)).forEach((m, idx) => {
        const crown = m.isHost ? "👑" : "🎧";
        const youTag = (jamClient.you && m.id === jamClient.you.id) ? " (You)" : "";
        rows.push(Terminal.truncate(` ${idx + 1}. ${crown} ${m.displayName || "Listener"}${youTag}`, innerWidth));
      });
    }

    while (rows.length < innerHeight - 1) {
      rows.push("");
    }

    const keyHints = isConnected
      ? `${Terminal.fg(...TextMuted)}[l] Leave  [h] Toggle Host Control  [p] Ping${reset}`
      : `${Terminal.fg(...TextMuted)}[c] Create  [j] Join  [s] Start Local Server${reset}`;
    rows.push(Terminal.truncate(keyHints, innerWidth));

    return Components.renderBox(width, height, "Listen Together", rows);
  }
}

module.exports = JamPartyView;
