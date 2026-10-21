# ♫ Walkz TUI Player

A terminal music player I built for my own daily listening — fast, keyboard-driven, and easy on the eyes.

Written in **100% Rust** with `ratatui` + `crossterm`. It plays your local music with album art, synchronized lyrics, playlists and a tag editor — all inside the terminal.

```
♫ Walkz TUI Player v0.3.5   [1 Now Playing]   2 Categories   3 Queue   4 Metadata Editor   5 Audio Output    📁 Music │ 🔊 85%
┌─── 💽 Album Art ──────────────────────────────┐┌─── 📜 Synchronized Lyrics ───────────────────────────────────┐
│                                               ││                                                              │
│              ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                 ││         Driving through the glowing skyline                  │
│            ▀▀██████████████████▀▀             ││         ▶ Midnight city lights, hold us through the dark     │
│            ▀██████████████████████▀           ││         Every single beat is a sudden spark                  │
│              ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀                 ││                                                              │
│   ∿∿∿ Hi-Res Lossless                         ││                                                              │
│   ♪ Demo Tone                                 ││                                                              │
│   Test Artist  •  Test Album                  ││                                                              │
├─── 🎵 Playlist: Test Album ───────────────────┤│                                                              │
│ ▶ 01. Demo Tone                  Test Artist  ││                                                             │
│   02. Preview Tone            Score Composer  ││                                                             │
└───────────────────────────────────────────────┘└──────────────────────────────────────────────────────────────┘
  01:23 ━━━━━━━━━━━━━●────────────────────────────────────────────── 05:20
 [▶ PLAYING]  [o] Open Folder  [S] Scan  [a] Song Menu  [Space] Play/Pause  [n/p] Next/Prev  [←/→] Seek  [↑/↓] Volume  [Tab] Focus  [q] Quit
```

## Features

- **3-pane Now Playing** — hi-res album art (Kitty / Sixel / iTerm2 / half-blocks fallback, antialiased CatmullRom resampling so covers stay as sharp as the originals), tracklist, and auto-scrolling synced lyrics; panes reflow responsively — the lyrics pane hands its width back on narrow windows instead of clipping
- **Ambient album-art background** — the Now Playing backdrop follows the playing song's cover, blurred and darkened like a Walkman ambience, on *every* terminal: full-bleed pixel image on Kitty / Sixel / iTerm2, a soft cell mosaic on half-block terminals; toggle with `[b]`, style & dimming in Settings
- **cava spectrum visualizer** — optional live spectrum lane above the transport bar, powered by a `cava` subprocess tapping your system's default sink monitor (`sudo apt install cava`; ffplay playback stays untouched); toggle in Settings
- **Categories** — metadata-driven sub-tabs: Album Artist (3-pane drill-down), Song, Artists, Album, and in-app Playlists
- **Persistent playlists & favourites** — survive rescans and restarts
- **Settings tab** — set your music folder path, launch cache, volume/seek steps, accent color, hotkey rebinding, ambient background style/dimming, the image protocol (Auto / Kitty / Sixel / Half-blocks — Auto sniffs kitty, ghostty, WezTerm, Windows Terminal, Konsole and iTerm2; WSL sessions resolve via `WT_SESSION`), and the spectrum visualizer; the folder and library are remembered, so your songs are ready on the next launch
- **Metadata tag editor** — edit Title / Artist / Album / Album Artist / Year / Genre / Track, saved to the file's native tags (ID3v2, MP4, Vorbis, FLAC)
- **Quality badges** — `∿∿∿ Hi-Res Lossless` / `∿∿∿ Lossless` / `LOSSY` with real codec, bit depth, sample rate and bitrate
- **Lightweight** — < 10 MB RAM, 0% idle CPU, zero UI jitter
- **Universal audio output** — routes to your default soundcard via `ffplay` (PipeWire / PulseAudio / ALSA), works with speakers, Bluetooth or headphones

## Installation

### Prerequisites

- **Rust toolchain** — install via [rustup](https://rustup.rs)
- **FFmpeg** — provides `ffplay`, used for audio playback (on Debian/Ubuntu: `sudo apt install ffmpeg`)
- *(Optional)* **cava** — powers the spectrum visualizer (on Debian/Ubuntu: `sudo apt install cava`); without it everything else works and the visualizer row just shows "needs cava"

### 1. Build from source

```bash
git clone https://github.com/Fayyiz55/walkz-tui-player.git
cd walkz-tui-player
cargo build --release
```

The compiled binary will be placed at `target/release/walkz-tui`.

### 2. Run it

```bash
./walkz-tui
```

### 3. (Optional) Install on your PATH

So `walkz-tui` works from anywhere:

```bash
cargo install --path .
```

or symlink the included launcher instead:

```bash
ln -sf "$PWD/walkz-tui" ~/.local/bin/walkz-tui
```

## First run

Press `[o]` inside the player to connect your music folder (e.g. `~/Music`) — it scans, categorizes and remembers it for next time. You can also pass it directly:

```bash
./walkz-tui --folder ~/Music
```

## Keys

| Key | Action |
|-----|--------|
| `[o]` / `[f]` | Connect / change music folder |
| `[1]` – `[6]` | Switch tabs (Now Playing, Categories, Queue, Metadata Editor, Audio Output, Settings) |
| `[Space]` | Play / Pause |
| `[n]` / `[p]` | Next / previous track |
| `[←]` / `[→]` | Seek back / forward (step configurable, default 5s; inside Categories: switch sub-tab) |
| `[+]` / `[-]` | Volume up / down (step configurable, default 5%) |
| `[a]` | Song context menu (Add to Playlist / Credits / Favourite) |
| `[b]` | Toggle the ambient album-art background (Now Playing) |
| `[e]` | Edit metadata tags · `[Ctrl+S]` saves to file |
| `[q]` | Quit |

## Tests

```bash
cargo test
```

## License

[GPL-3.0](LICENSE) — see [LICENSE](LICENSE).
