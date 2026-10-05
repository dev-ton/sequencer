use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::{Duration, Instant},
};

use super::Track;
use crate::{
    generators::{Euclidean, RandomWalk, Sequence},
    midi::Output,
    music::scale::Scale,
};

#[derive(Debug)]
pub enum Command {
    Toggle,
    SetBpm(u16),
    ToggleMute(usize),
    Reseed(u64),
    Quit,
}

pub struct Sequencer {
    pub tx: Sender<Command>,
    pub rx: Receiver<String>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Sequencer {
    pub fn start(mut output: Output) -> Self {
        let (tx, commands) = mpsc::channel();
        let (status, rx) = mpsc::channel();
        let thread = thread::spawn(move || run(&mut output, commands, status));
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
    vec![
        Track::new(1, seed, "PULSE", 9, 100, Box::new(Euclidean::new(5, 8, 36))),
        Track::new(
            2,
            seed,
            "WALK",
            0,
            80,
            Box::new(RandomWalk::new(50, 2, Scale::Dorian, seed ^ 2)),
        ),
        Track::new(
            3,
            seed,
            "SEQUENCE",
            1,
            90,
            Box::new(Sequence::new(vec![Some(50), Some(53), Some(55), Some(60)])),
        ),
    ]
}

fn run(output: &mut Output, commands: Receiver<Command>, status: Sender<String>) {
    let mut seed = 666;
    let mut bpm = 100u16;
    let mut running = false;
    let mut tracks = demo_tracks(seed);
    let mut tick = 0u64;
    let mut next_tick = Instant::now();
    let mut active: Vec<(usize, u8, u8, u64)> = Vec::new();
    let _ = status.send("STOPPED".into());
    loop {
        let tick_duration = Duration::from_secs_f64(60.0 / f64::from(bpm) / 24.0);
        let wait = if running {
            next_tick.saturating_duration_since(Instant::now())
        } else {
            Duration::from_millis(100)
        };
        match commands.recv_timeout(wait) {
            Ok(Command::Quit) => break,
            Ok(Command::Toggle) => {
                running = !running;
                if running {
                    next_tick = Instant::now();
                    let names = tracks
                        .iter()
                        .map(|track| track.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let _ = status.send(format!("RUNNING · {names}"));
                } else {
                    cleanup(output, &mut active);
                    let _ = status.send("STOPPED".into());
                }
            }
            Ok(Command::SetBpm(value)) => bpm = value.clamp(30, 300),
            Ok(Command::ToggleMute(index)) => {
                if let Some(track) = tracks.get_mut(index) {
                    track.muted = !track.muted;
                    cleanup_track(output, &mut active, index);
                }
            }
            Ok(Command::Reseed(value)) => {
                cleanup(output, &mut active);
                seed = value;
                tick = 0;
                tracks = demo_tracks(seed);
                let _ = status.send(format!("SEED {seed}"));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if running && Instant::now() >= next_tick {
            for (index, track) in tracks.iter_mut().enumerate() {
                if !track.enabled || track.muted || tick % 6 != 0 {
                    continue;
                }
                let step = (tick / 6) as usize;
                if let Some(note) = track.next() {
                    let _ = output.note_on(track.channel, note, track.velocity);
                    active.push((index, track.channel, note, tick + u64::from(track.duration)));
                }
                track.step = step.wrapping_add(1);
            }
            for i in (0..active.len()).rev() {
                if active[i].3 <= tick {
                    let (_, channel, note, _) = active.remove(i);
                    let _ = output.note_off(channel, note);
                }
            }
            tick = tick.wrapping_add(1);
            next_tick += tick_duration;
            if next_tick + tick_duration * 4 < Instant::now() {
                next_tick = Instant::now() + tick_duration;
            }
        }
    }
    cleanup(output, &mut active);
    output.all_notes_off();
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
