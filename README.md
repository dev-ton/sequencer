# Terminal Sequencer

An experimental terminal-based algorithmic MIDI sequencer written in Rust. This v0.1 prototype sends generated note patterns to an external MIDI output; it does not produce audio.

## v0.1 capabilities

- MIDI output discovery and selection at startup
- Internal 24 PPQN clock, start/stop transport, and live BPM changes
- Three demo tracks: Euclidean pulse, seeded D Dorian random walk, and fixed sequence
- Per-track deterministic RNG and probability support in the engine
- Scale quantization for chromatic, major, natural minor, Dorian, Phrygian, Lydian, and Mixolydian
- Note Offs on note duration, track mute, stop, and shutdown, plus All Notes Off cleanup

## Build and run

Install Rust and connect or enable a MIDI output, such as a hardware synth or a software MIDI port. Then run:

```sh
cargo run
```

Choose a listed MIDI output when prompted. The initial seed is `666`.

## Controls

- `Space`: start or stop
- `Up` / `Down`: select track
- `M`: mute or unmute selected track
- `+` / `-`: raise or lower BPM
- `R`: generate a new seed and restart the patterns
- `Q`: quit

## Structure

`generators` and `music` contain hardware-independent musical logic. `engine` owns tracks, timing, and transport in a sequencer thread. `midi` handles port selection and MIDI messages. `ui` renders the terminal controls with ratatui and sends commands to the engine over standard channels.

## Limitations

This is experimental software. It has no persistence, MIDI input, audio engine, external clock sync, editing interface, or real-time guarantees. MIDI output device behavior varies; test with a noncritical setup. The demo currently uses a fixed sixteenth-note trigger grid.
