use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use crate::{
    generators::{track_rng, Euclidean, RandomWalk, Sequence},
    midi::Output,
    music::scale::Scale,
};
use rand::Rng;

use super::track::{TrackControl, TrackOptions, TrackSnapshot};
use super::Track;

const INITIAL_SEED: u64 = 666;
const TICKS_PER_QUARTER: u64 = 24;
const TICKS_PER_STEP: u64 = TICKS_PER_QUARTER / 4;

#[derive(Debug)]
pub enum Command {
    Toggle,
    SetBpm(u16),
    ToggleMute(usize),
    AdjustTrack {
        index: usize,
        control: TrackControl,
        direction: i8,
    },
    ToggleRest {
        index: usize,
        control: TrackControl,
    },
    Reseed(u64),
    Quit,
}

#[derive(Clone, Debug)]
pub struct SequencerSnapshot {
    pub bpm: u16,
    pub seed: u64,
    pub running: bool,
    pub tracks: Vec<TrackSnapshot>,
}

impl Default for SequencerSnapshot {
    fn default() -> Self {
        Self {
            bpm: 100,
            seed: INITIAL_SEED,
            running: false,
            tracks: Vec::new(),
        }
    }
}

pub struct Sequencer {
    pub tx: Sender<Command>,
    pub rx: Receiver<SequencerSnapshot>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Sequencer {
    pub fn start(mut output: Output) -> Self {
        let (tx, commands) = mpsc::channel();
        let (snapshots, rx) = mpsc::channel();
        let thread = thread::spawn(move || run(&mut output, commands, snapshots));
        Self {
            tx,
            rx,
            thread: Some(thread),
        }
    }

    pub fn shutdown(&mut self) {
        let _ = self.tx.send(Command::Quit);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Sequencer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn demo_tracks(seed: u64) -> Vec<Track> {
    let mut pulse_rng = track_rng(seed, 1);
    let pulse_steps = [8, 12, 16][pulse_rng.gen_range(0..3)];
    let pulse_hits = pulse_rng.gen_range(3..=pulse_steps.min(7));
    let pulse_note = [36, 38, 42, 46, 49, 51][pulse_rng.gen_range(0..6)];
    let pulse_velocity = pulse_rng.gen_range(70..=110);
    let pulse_probability = f64::from(pulse_rng.gen_range(12..=20)) / 20.0;
    let pulse_gate = pulse_rng.gen_range(1..=4);

    let mut walk_rng = track_rng(seed, 2);
    let walk_start = walk_rng.gen_range(45..=60);
    let walk_root = walk_rng.gen_range(0..12);
    let walk_scale = Scale::ALL[walk_rng.gen_range(0..Scale::ALL.len())];
    let walk_velocity = walk_rng.gen_range(60..=100);
    let walk_probability = f64::from(walk_rng.gen_range(12..=20)) / 20.0;
    let walk_gate = walk_rng.gen_range(1..=4);

    let mut sequence_rng = track_rng(seed, 3);
    let sequence_root = sequence_rng.gen_range(0..12);
    let sequence_scale = Scale::ALL[sequence_rng.gen_range(0..Scale::ALL.len())];
    let pitches = scale_pitches(sequence_root, sequence_scale, 48, 72);
    let sequence_length = sequence_rng.gen_range(4..=8);
    let sequence_notes = (0..sequence_length)
        .map(|_| {
            if sequence_rng.gen_range(0..5) == 0 {
                None
            } else {
                Some(pitches[sequence_rng.gen_range(0..pitches.len())])
            }
        })
        .collect();
    let sequence_velocity = sequence_rng.gen_range(65..=105);
    let sequence_probability = f64::from(sequence_rng.gen_range(12..=20)) / 20.0;
    let sequence_gate = sequence_rng.gen_range(1..=4);

    vec![
        Track::new(
            1,
            seed,
            "PULSE",
            9,
            TrackOptions {
                velocity: pulse_velocity,
                gate_steps: pulse_gate,
                probability: pulse_probability,
            },
            Box::new(Euclidean::new(pulse_hits, pulse_steps, pulse_note)),
        ),
        Track::new(
            2,
            seed,
            "WALK",
            0,
            TrackOptions {
                velocity: walk_velocity,
                gate_steps: walk_gate,
                probability: walk_probability,
            },
            Box::new(RandomWalk::new(
                walk_start,
                walk_root,
                walk_scale,
                seed ^ 0xA0761D6478BD642F,
            )),
        ),
        Track::new(
            3,
            seed,
            "SEQUENCE",
            1,
            TrackOptions {
                velocity: sequence_velocity,
                gate_steps: sequence_gate,
                probability: sequence_probability,
            },
            Box::new(Sequence::new(sequence_notes)),
        ),
    ]
}

fn run(output: &mut Output, commands: Receiver<Command>, snapshots: Sender<SequencerSnapshot>) {
    let mut seed = INITIAL_SEED;
    let mut bpm = 100u16;
    let mut running = false;
    let mut tracks = demo_tracks(seed);
    let mut tick = 0u64;
    let mut next_tick = Instant::now();
    let mut active: Vec<(usize, u8, u8, u64)> = Vec::new();
    send_snapshot(&snapshots, bpm, seed, running, &tracks);

    loop {
        let tick_duration = Duration::from_secs_f64(60.0 / f64::from(bpm) / 24.0);
        let wait = if running {
            next_tick.saturating_duration_since(Instant::now())
        } else {
            Duration::from_millis(100)
        };
        let mut changed = false;
        match commands.recv_timeout(wait) {
            Ok(Command::Quit) => break,
            Ok(Command::Toggle) => {
                running = !running;
                if running {
                    next_tick = Instant::now();
                } else {
                    cleanup(output, &mut active);
                }
                changed = true;
            }
            Ok(Command::SetBpm(value)) => {
                bpm = value.clamp(30, 300);
                changed = true;
            }
            Ok(Command::ToggleMute(index)) => {
                if let Some(track) = tracks.get_mut(index) {
                    track.muted = !track.muted;
                    cleanup_track(output, &mut active, index);
                    changed = true;
                }
            }
            Ok(Command::AdjustTrack {
                index,
                control,
                direction,
            }) => {
                if let Some(track) = tracks.get_mut(index) {
                    track.adjust(control, direction);
                    cleanup_track(output, &mut active, index);
                    changed = true;
                }
            }
            Ok(Command::ToggleRest { index, control }) => {
                if let Some(track) = tracks.get_mut(index) {
                    track.toggle_rest(control);
                    cleanup_track(output, &mut active, index);
                    changed = true;
                }
            }
            Ok(Command::Reseed(value)) => {
                cleanup(output, &mut active);
                let mute_states: Vec<bool> = tracks.iter().map(|track| track.muted).collect();
                seed = value;
                tick = 0;
                tracks = demo_tracks(seed);
                for (track, muted) in tracks.iter_mut().zip(mute_states) {
                    track.muted = muted;
                }
                changed = true;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        if running && Instant::now() >= next_tick {
            if tick % TICKS_PER_STEP == 0 {
                for (index, track) in tracks.iter_mut().enumerate() {
                    let note = track.next();
                    if track.enabled && !track.muted {
                        if let Some(note) = note {
                            let _ = output.note_on(track.channel, note, track.velocity);
                            active.push((
                                index,
                                track.channel,
                                note,
                                tick + u64::from(track.gate_steps) * TICKS_PER_STEP,
                            ));
                        }
                    }
                }
                changed = true;
            }
            for index in (0..active.len()).rev() {
                if active[index].3 <= tick {
                    let (_, channel, note, _) = active.remove(index);
                    let _ = output.note_off(channel, note);
                }
            }
            tick = tick.wrapping_add(1);
            next_tick += tick_duration;
            if next_tick + tick_duration * 4 < Instant::now() {
                next_tick = Instant::now() + tick_duration;
            }
        }

        if changed {
            send_snapshot(&snapshots, bpm, seed, running, &tracks);
        }
    }
    cleanup(output, &mut active);
    output.all_notes_off();
}

fn send_snapshot(
    sender: &Sender<SequencerSnapshot>,
    bpm: u16,
    seed: u64,
    running: bool,
    tracks: &[Track],
) {
    let snapshot = SequencerSnapshot {
        bpm,
        seed,
        running,
        tracks: tracks.iter().map(Track::snapshot).collect(),
    };
    let _ = sender.send(snapshot);
}

fn cleanup(output: &mut Output, active: &mut Vec<(usize, u8, u8, u64)>) {
    for (_, channel, note, _) in active.drain(..) {
        let _ = output.note_off(channel, note);
    }
    output.all_notes_off();
}

fn cleanup_track(output: &mut Output, active: &mut Vec<(usize, u8, u8, u64)>, index: usize) {
    let mut keep = Vec::new();
    for (track, channel, note, end) in active.drain(..) {
        if track == index {
            let _ = output.note_off(channel, note);
        } else {
            keep.push((track, channel, note, end));
        }
    }
    *active = keep;
}

fn scale_pitches(root: u8, scale: Scale, low: u8, high: u8) -> Vec<u8> {
    (low..=high)
        .filter(|note| scale.intervals().contains(&((note + 12 - root) % 12)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_recreates_all_track_settings() {
        let first: Vec<_> = demo_tracks(666).iter().map(Track::snapshot).collect();
        let second: Vec<_> = demo_tracks(666).iter().map(Track::snapshot).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn different_seed_changes_musical_settings_without_changing_midi_routing() {
        let first: Vec<_> = demo_tracks(666).iter().map(Track::snapshot).collect();
        let second: Vec<_> = demo_tracks(667).iter().map(Track::snapshot).collect();
        assert_ne!(first, second);
        for (first_track, second_track) in first.iter().zip(&second) {
            assert_ne!(first_track.controls, second_track.controls);
        }
        assert_eq!(
            first.iter().map(|track| track.channel).collect::<Vec<_>>(),
            vec![9, 0, 1]
        );
        assert_eq!(
            second.iter().map(|track| track.channel).collect::<Vec<_>>(),
            vec![9, 0, 1]
        );
    }

    #[test]
    fn scale_pitch_pool_has_notes_for_each_root_and_mode() {
        for root in 0..12 {
            for scale in Scale::ALL {
                let pitches = scale_pitches(root, scale, 48, 72);
                assert!(!pitches.is_empty());
                assert!(pitches
                    .iter()
                    .all(|note| { scale.intervals().contains(&((note + 12 - root) % 12)) }));
            }
        }
    }
}
