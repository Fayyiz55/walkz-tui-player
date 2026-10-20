/**
 * Built-in audio track catalog with synchronized lyrics and metadata
 * Provides immediately playable tracks even in offline environments.
 */
const DEMO_TRACKS = [
  {
    id: "demo_1",
    title: "Midnight City Lights",
    artist: "Synthwave Collective",
    album: "Neon Horizons",
    durationMs: 142000,
    source: "Built-in / Lossless FLAC",
    codec: "flac (24-bit/48kHz)",
    bitrate: "920 kbps",
    sampleRate: "48000 Hz",
    loudnessDb: -1.2,
    bpm: 120,
    key: "8B (C Major)",
    toneFreqs: [261.63, 329.63, 392.00, 523.25], // C major synth arpeggios
    lyrics: `[00:00.00]Instrumental Intro - Neon Chords
[00:06.50]Driving through the glowing skyline
[00:11.20]Reflections on the hood of midnight chrome
[00:16.80]Signals flashing, purple horizons
[00:22.40]Never feeling so far from home
[00:28.10]And the bassline starts to breathe
[00:33.50]Electric dreams we cannot leave
[00:39.20]Midnight city lights, hold us through the dark
[00:44.80]Every single beat is a sudden spark
[00:50.50]Turn the volume high, let the frequencies align
[00:56.20]In the bit-stream of the night divine
[01:02.00]Synthesizers pulse in the quiet cold
[01:07.50]A story whispered that was never told
[01:13.20]Midnight city lights, running through our veins
[01:18.90]Washing off the shadows and the static rain
[01:25.00]Instrumental Bridge - Filter Sweep
[01:35.00]Midnight city lights, fading in the dawn
[01:40.50]The rhythm stays forever even when we're gone`
  },
  {
    id: "demo_2",
    title: "Lo-Fi Coffee Rain",
    artist: "Chillhop Room",
    album: "Cozy Afternoons",
    durationMs: 128000,
    source: "Built-in / Opus Audio",
    codec: "opus (stereo)",
    bitrate: "160 kbps",
    sampleRate: "48000 Hz",
    loudnessDb: -2.8,
    bpm: 85,
    key: "5A (C Minor)",
    toneFreqs: [220.0, 261.63, 329.63, 440.0],
    lyrics: `[00:00.00]Raindrops tapping softly on the glass
[00:08.00]Watching gentle seconds slowly pass
[00:16.50]Steam rising from an old ceramic cup
[00:24.00]World slows down, never in a rush
[00:32.00]Warm tape saturation in the breeze
[00:40.00]Rustling through the autumn yellow trees
[00:48.50]Soft rhodes chords drifting in the room
[00:56.50]Chasing out the lingering afternoon gloom
[01:05.00]Just sit back, take another sip
[01:13.50]Let the vinyl crackle through this journey trip
[01:22.00]Rain falls down... silence all around`
  },
  {
    id: "demo_3",
    title: "Quantum Funk Odyssey",
    artist: "The Groove Matrix",
    album: "Hyperdimensional Beats",
    durationMs: 156000,
    source: "Built-in / AAC 320",
    codec: "aac (320kbps)",
    bitrate: "320 kbps",
    sampleRate: "44100 Hz",
    loudnessDb: -0.5,
    bpm: 128,
    key: "11B (A Major)",
    toneFreqs: [110.0, 220.0, 277.18, 329.63],
    lyrics: `[00:00.00]Slap bass warming up...
[00:05.20]Quantum particles jump to the groove
[00:10.00]Watch the whole multiverse start to move
[00:15.10]Got the pocket tight, got the rhythm clean
[00:20.30]Funkiest dimension you have ever seen!
[00:25.80]One, two, punch the overdrive!
[00:31.00]That's the feeling when the groove is alive
[00:36.40]Space and time twisting in the sound
[00:41.80]Nobody standing still on this quantum ground
[00:47.20]Slap the bass, hit the snare, drop the beat!
[00:52.50]Feel the radiation rising in the heat
[00:58.00]Brass section soaring to the upper sky
[01:03.50]We're riding cosmic waves so high!`
  },
  {
    id: "demo_4",
    title: "Acoustic Forest Solitude",
    artist: "Willow & Pine",
    album: "Woodland Echoes",
    durationMs: 135000,
    source: "Built-in / Lossless ALAC",
    codec: "alac (24-bit/96kHz)",
    bitrate: "1150 kbps",
    sampleRate: "96000 Hz",
    loudnessDb: -3.5,
    bpm: 96,
    key: "9B (G Major)",
    toneFreqs: [196.0, 246.94, 293.66, 392.0],
    lyrics: `[00:00.00]Fingerpicking nylon strings
[00:07.20]Listen to the song the mountain sings
[00:14.50]Morning mist clinging to the pine
[00:22.00]Sunlight breaking in a golden line
[00:29.80]Step softly along the winding trail
[00:37.00]Echoes of a forgotten northern tale
[00:45.00]Wind through branches, gentle and slow
[00:52.50]Where only the whispering rivers flow
[01:00.20]Take a breath of wilderness so deep
[01:08.00]Secrets that the ancient valleys keep
[01:16.00]Harmony in every simple stone
[01:24.00]Never truly lost, never quite alone`
  }
];

module.exports = { DEMO_TRACKS };
