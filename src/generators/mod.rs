use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::music::scale::{quantize, Scale};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeneratorControl {
    EuclideanPulses,
    EuclideanSteps,
    Note,
    WalkStart,
    WalkRoot,
    WalkScale,
    SequenceLength,
    SequenceNote(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlValue {
    pub key: GeneratorControl,
    pub label: String,
    pub value: String,
}

pub struct GeneratedStep {
    pub note: Option<u8>,
}

pub struct GeneratorContext {
    pub step: usize,
}

pub trait Generator: Send {
    fn next(&mut self, context: &GeneratorContext) -> GeneratedStep;
    fn controls(&self) -> Vec<ControlValue>;
    fn adjust(&mut self, control: GeneratorControl, direction: i8);
    fn toggle_rest(&mut self, _control: GeneratorControl) {}
    fn description(&self) -> String;
}

pub struct Sequence {
    notes: Vec<Option<u8>>,
    position: usize,
}

impl Sequence {
    pub fn new(notes: Vec<Option<u8>>) -> Self {
        Self { notes, position: 0 }
    }
}

impl Generator for Sequence {
    fn next(&mut self, _: &GeneratorContext) -> GeneratedStep {
        let note = self.notes.get(self.position).copied().flatten();
        if !self.notes.is_empty() {
            self.position = (self.position + 1) % self.notes.len();
        }
        GeneratedStep { note }
    }

    fn controls(&self) -> Vec<ControlValue> {
        let mut controls = Vec::with_capacity(self.notes.len() + 1);
        controls.push(ControlValue {
            key: GeneratorControl::SequenceLength,
            label: "Steps".into(),
            value: self.notes.len().to_string(),
        });
        controls.extend(
            self.notes
                .iter()
                .enumerate()
                .map(|(index, note)| ControlValue {
                    key: GeneratorControl::SequenceNote(index),
                    label: format!("Step {}", index + 1),
                    value: note.map(note_name).unwrap_or_else(|| "REST".into()),
                }),
        );
        controls
    }

    fn adjust(&mut self, control: GeneratorControl, direction: i8) {
        match control {
            GeneratorControl::SequenceLength => {
                let new_len =
                    (self.notes.len() as isize + isize::from(direction)).clamp(1, 16) as usize;
                self.notes.resize(new_len, None);
                self.position %= new_len;
            }
            GeneratorControl::SequenceNote(index) => {
                if let Some(note) = self.notes.get_mut(index) {
                    let value = note.unwrap_or(60);
                    *note = Some((i16::from(value) + i16::from(direction)).clamp(0, 127) as u8);
                }
            }
            _ => {}
        }
    }

    fn toggle_rest(&mut self, control: GeneratorControl) {
        if let GeneratorControl::SequenceNote(index) = control {
            if let Some(note) = self.notes.get_mut(index) {
                *note = note.map_or(Some(60), |_| None);
            }
        }
    }

    fn description(&self) -> String {
        format!("sequence · {} steps", self.notes.len())
    }
}

pub struct RandomWalk {
    note: u8,
    start: u8,
    root: u8,
    scale: Scale,
    rng: ChaCha8Rng,
}

impl RandomWalk {
    pub fn new(start: u8, root: u8, scale: Scale, seed: u64) -> Self {
        let start = quantize(start.min(127), root, scale);
        Self {
            note: start,
            start,
            root: root % 12,
            scale,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}

impl Generator for RandomWalk {
    fn next(&mut self, _: &GeneratorContext) -> GeneratedStep {
        let step = self.rng.gen_range(-2..=2);
        self.note = (i16::from(self.note) + step).clamp(0, 127) as u8;
        self.note = quantize(self.note, self.root, self.scale);
        GeneratedStep {
            note: Some(self.note),
        }
    }

    fn controls(&self) -> Vec<ControlValue> {
        vec![
            ControlValue {
                key: GeneratorControl::WalkStart,
                label: "Start".into(),
                value: note_name(self.start),
            },
            ControlValue {
                key: GeneratorControl::WalkRoot,
                label: "Root".into(),
                value: pitch_class_name(self.root).into(),
            },
            ControlValue {
                key: GeneratorControl::WalkScale,
                label: "Scale".into(),
                value: self.scale.name().into(),
            },
        ]
    }

    fn adjust(&mut self, control: GeneratorControl, direction: i8) {
        match control {
            GeneratorControl::WalkStart => {
                self.start = (i16::from(self.start) + i16::from(direction)).clamp(0, 127) as u8;
                self.start = quantize(self.start, self.root, self.scale);
                self.note = self.start;
            }
            GeneratorControl::WalkRoot => {
                self.root = (i16::from(self.root) + i16::from(direction)).rem_euclid(12) as u8;
                self.start = quantize(self.start, self.root, self.scale);
                self.note = quantize(self.note, self.root, self.scale);
            }
            GeneratorControl::WalkScale => {
                let scales = Scale::ALL;
                let current = scales
                    .iter()
                    .position(|&scale| scale == self.scale)
                    .unwrap_or(0);
                let next = (current as isize + isize::from(direction))
                    .rem_euclid(scales.len() as isize) as usize;
                self.scale = scales[next];
                self.start = quantize(self.start, self.root, self.scale);
                self.note = quantize(self.note, self.root, self.scale);
            }
            _ => {}
        }
    }

    fn description(&self) -> String {
        format!(
            "random walk · {} {}",
            pitch_class_name(self.root),
            self.scale.name()
        )
    }
}

pub struct Euclidean {
    pattern: Vec<bool>,
    pulses: usize,
    steps: usize,
    note: u8,
}

impl Euclidean {
    pub fn new(pulses: usize, steps: usize, note: u8) -> Self {
        let steps = steps.clamp(1, 32);
        let pulses = pulses.clamp(1, steps);
        Self {
            pattern: euclid(pulses, steps),
            pulses,
            steps,
            note: note.min(127),
        }
    }

    fn rebuild(&mut self) {
        self.pattern = euclid(self.pulses, self.steps);
    }
}

impl Generator for Euclidean {
    fn next(&mut self, context: &GeneratorContext) -> GeneratedStep {
        let note = if self.pattern[context.step % self.pattern.len()] {
            Some(self.note)
        } else {
            None
        };
        GeneratedStep { note }
    }

    fn controls(&self) -> Vec<ControlValue> {
        vec![
            ControlValue {
                key: GeneratorControl::EuclideanPulses,
                label: "Hits".into(),
                value: self.pulses.to_string(),
            },
            ControlValue {
                key: GeneratorControl::EuclideanSteps,
                label: "Steps".into(),
                value: self.steps.to_string(),
            },
            ControlValue {
                key: GeneratorControl::Note,
                label: "Note".into(),
                value: note_name(self.note),
            },
        ]
    }

    fn adjust(&mut self, control: GeneratorControl, direction: i8) {
        match control {
            GeneratorControl::EuclideanPulses => {
                self.pulses = (self.pulses as isize + isize::from(direction))
                    .clamp(1, self.steps as isize) as usize;
                self.rebuild();
            }
            GeneratorControl::EuclideanSteps => {
                self.steps = (self.steps as isize + isize::from(direction)).clamp(1, 32) as usize;
                self.pulses = self.pulses.min(self.steps);
                self.rebuild();
            }
            GeneratorControl::Note => {
                self.note = (i16::from(self.note) + i16::from(direction)).clamp(0, 127) as u8;
            }
            _ => {}
        }
    }

    fn description(&self) -> String {
        format!(
            "euclid({},{}) · {}",
            self.pulses,
            self.steps,
            note_name(self.note)
        )
    }
}

pub fn euclid(pulses: usize, steps: usize) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }
    let pulses = pulses.min(steps);
    (0..steps)
        .map(|index| ((index * pulses) % steps) < pulses)
        .collect()
}

pub fn passes_probability(rng: &mut ChaCha8Rng, probability: f64) -> bool {
    probability >= 1.0 || (probability > 0.0 && rng.gen::<f64>() < probability)
}

pub fn track_rng(seed: u64, track_id: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed ^ track_id.wrapping_mul(0x9E3779B97F4A7C15))
}

fn note_name(note: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    format!(
        "{}{}",
        NAMES[usize::from(note % 12)],
        i16::from(note) / 12 - 1
    )
}

fn pitch_class_name(pitch_class: u8) -> &'static str {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    NAMES[usize::from(pitch_class % 12)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euclidean_five_of_eight_has_five_hits() {
        let pattern = euclid(5, 8);
        assert_eq!(pattern.iter().filter(|&&active| active).count(), 5);
        assert_eq!(
            pattern,
            vec![true, false, true, false, true, true, false, true]
        );
    }

    #[test]
    fn sequence_wraps_and_keeps_rests() {
        let mut sequence = Sequence::new(vec![Some(60), None, Some(64)]);
        let context = GeneratorContext { step: 0 };
        assert_eq!(sequence.next(&context).note, Some(60));
        assert_eq!(sequence.next(&context).note, None);
        assert_eq!(sequence.next(&context).note, Some(64));
        assert_eq!(sequence.next(&context).note, Some(60));
    }

    #[test]
    fn euclidean_edits_keep_hits_inside_the_pattern() {
        let mut rhythm = Euclidean::new(5, 8, 36);
        rhythm.adjust(GeneratorControl::EuclideanSteps, -1);
        let controls = rhythm.controls();
        assert!(controls.iter().any(|control| control.value == "7"));
        assert!(controls.iter().any(|control| control.value == "5"));
    }

    #[test]
    fn sequence_note_can_be_edited_and_toggled_to_a_rest() {
        let mut sequence = Sequence::new(vec![Some(60)]);
        sequence.adjust(GeneratorControl::SequenceNote(0), 1);
        assert_eq!(sequence.controls()[1].value, "C#4");
        sequence.toggle_rest(GeneratorControl::SequenceNote(0));
        assert_eq!(sequence.controls()[1].value, "REST");
    }

    #[test]
    fn seeded_walk_and_probability_are_repeatable() {
        fn walk(seed: u64) -> Vec<Option<u8>> {
            let mut generator = RandomWalk::new(50, 2, Scale::Dorian, seed);
            (0..32)
                .map(|step| generator.next(&GeneratorContext { step }).note)
                .collect()
        }
        assert_eq!(walk(666), walk(666));
        assert_ne!(walk(666), walk(667));
        let mut first_rng = track_rng(666, 1);
        let mut second_rng = track_rng(666, 1);
        let first: Vec<_> = (0..100)
            .map(|_| passes_probability(&mut first_rng, 0.5))
            .collect();
        let second: Vec<_> = (0..100)
            .map(|_| passes_probability(&mut second_rng, 0.5))
            .collect();
        assert_eq!(first, second);
        assert!(first.iter().any(|value| *value) && first.iter().any(|value| !*value));
    }
}
