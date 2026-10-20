#!/usr/bin/env node

/**
 * BitChord TUI - Aesthetic Terminal Music Player
 */

const App = require('./app');
const JamServer = require('./jam/jam_server');
const audio = require('./player/audio_controller');
const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');

const args = process.argv.slice(2);

if (args.includes('--help') || args.includes('-h')) {
  console.log(`
BitChord TUI - Aesthetic Terminal Music Player
Inspired by BitChord (https://github.com/kushagrasinghx/BitChord)

Usage:
  bitchord-tui [options] [folder_path]

Folder Mode:
  bitchord-tui /path/to/music      Connect directly to your music folder
  bitchord-tui --folder <path>     Connect to specified folder only (-f, --dir)

Audio & Speaker Options:
  --driver <name>                  Select audio output driver (pulse, pipewire, alsa, dummy)
  --check-audio, --test-audio      Test PC speakers with audio chime and report sound status

Jam / Server Options:
  --server, -s                     Run standalone Listen Together Jam server on port 8000
  --port <number>                  Port for Jam server (default: 8000)
  --version, -v                    Show version

Keybindings:
  1 - 6                            Switch tabs (Now Playing, Discover, Queue, Jam, EQ, Library)
  Space                            Play / Pause
  n / p                            Next / Previous Track
  ← / →                            Seek backward / forward 5s
  ↑ / ↓                            Volume Up / Down (or navigate lists)
  f                                Connect to another music folder (in Library view)
  /                                Focus search bar
  Enter                            Play selected track
  a                                Add to queue
  q                                Quit
`);
  process.exit(0);
}

if (args.includes('--version') || args.includes('-v')) {
  console.log('BitChord TUI v1.0.0');
  process.exit(0);
}

// Speaker Test Utility
if (args.includes('--check-audio') || args.includes('--test-audio')) {
  console.log('🔊 Testing PC Speaker Audio Output with ffplay...');
  console.log(`Current SDL_AUDIODRIVER: ${process.env.SDL_AUDIODRIVER || 'System Default (Pulse/PipeWire/ALSA)'}`);
  
  const testWav = '/tmp/bitchord_test_chime.wav';
  // Generate 440Hz / 880Hz two-tone chime
  const gen = spawn('ffmpeg', [
    '-f', 'lavfi', '-i', 'sine=f=587.33:d=0.3[s1];sine=f=880:d=0.5[s2];[s1][s2]concat=n=2:v=0:a=1[out]',
    '-map', '[out]',
    '-c:a', 'pcm_s16le',
    '-y', testWav
  ], { stdio: 'ignore' });

  gen.on('close', () => {
    console.log('Playing chime through PC speakers...');
    const play = spawn('ffplay', ['-nodisp', '-autoexit', testWav]);
    let stderr = '';
    play.stderr.on('data', d => stderr += d.toString());
    play.on('close', (code) => {
      if (code === 0 && !stderr.includes('audio open failed')) {
        console.log('✅ Speaker audio check succeeded! Audio sent to PC sound device.');
      } else {
        console.log('⚠️ Could not open physical audio output device. Stderr:\n', stderr.trim());
        console.log('\nTip: On Linux, ensure your user has access to sound devices:');
        console.log('  pactl info  (or wpctl status)');
        console.log('  bitchord-tui --driver pulse');
      }
      process.exit(0);
    });
  });
  return;
}

if (args.includes('--server') || args.includes('-s')) {
  const portIdx = args.indexOf('--port');
  const port = portIdx !== -1 && args[portIdx + 1] ? parseInt(args[portIdx + 1], 10) : 8000;
  const server = new JamServer(port);
  server.start();
  console.log(`BitChord Jam Server started on http://localhost:${port}`);
  return;
}

// Parse Folder and Audio Driver options
let folderPath = null;
let driverName = null;

for (let i = 0; i < args.length; i++) {
  const arg = args[i];
  if (arg === '--folder' || arg === '-f' || arg === '--dir') {
    folderPath = args[i + 1];
    i++;
  } else if (arg === '--driver') {
    driverName = args[i + 1];
    i++;
  } else if (!arg.startsWith('-')) {
    // Positional folder argument
    folderPath = arg;
  }
}

const app = new App({
  folder: folderPath,
  driver: driverName
});
app.start();
