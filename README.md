# Terminal Sequencer

An experimental terminal-based algorithmic MIDI sequencer written in Rust. The v0.0.2 prototype sends generated note patterns to an external MIDI output; it does not produce audio.

## Current capabilities

- MIDI output discovery and selection at startup
- Internal 24 PPQN clock, start/stop transport, and live BPM changes
- Three editable tracks: Euclidean pulse, seeded random walk, and note sequence
- Live editing for MIDI channel, probability, velocity, gate length, generator settings, and sequence notes/rests
- A global deterministic seed generates musical settings for all tracks while keeping their MIDI channels stable
- Scale quantization for chromatic, major, natural minor, Dorian, Phrygian, Lydian, and Mixolydian
- Note Offs on note duration, track mute, stop, and shutdown, plus All Notes Off cleanup

## Build and run

Install Rust and connect or enable a MIDI output, such as a hardware synth or a software MIDI port. Then run:

```sh
cargo run
```

Choose a listed MIDI output when prompted. The initial seed is `666`; pressing `R` creates a new seeded arrangement.

## Controls

- `Space`: start or stop
- `Up` / `Down`: select track
- `Left` / `Right`: select a setting on the selected track
- `[` / `]`: decrease or increase the selected setting
- `X`: toggle a sequence step between a note and a rest
- `M`: mute or unmute selected track
- `+` / `-`: raise or lower BPM
- `R`: generate a new seed and new musical settings for all tracks
- `Q`: quit

## Structure

`generators` and `music` contain hardware-independent musical logic. `engine` owns tracks, timing, and transport in a sequencer thread. `midi` handles port selection and MIDI messages. `ui` renders the terminal controls with ratatui and sends commands to the engine over standard channels.

## Limitations

This is experimental software. It has no persistence, MIDI input, audio engine, external clock sync, or real-time guarantees. MIDI output device behavior varies; test with a noncritical setup. The tracks use a sixteenth-note trigger grid, and settings are not saved when the application exits.
