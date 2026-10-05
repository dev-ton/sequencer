use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::music::scale::{quantize, Scale};

pub struct GeneratedStep {
    pub note: Option<u8>,
}
pub struct GeneratorContext {
    pub step: usize,
}

pub trait Generator: Send {
    fn next(&mut self, context: &GeneratorContext) -> GeneratedStep;
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
}

pub struct RandomWalk {
    note: i16,
    root: u8,
    scale: Scale,
    rng: ChaCha8Rng,
}
impl RandomWalk {
    pub fn new(start: u8, root: u8, scale: Scale, seed: u64) -> Self {
        Self {
            note: start as i16,
            root,
            scale,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }
}
impl Generator for RandomWalk {
    fn next(&mut self, _: &GeneratorContext) -> GeneratedStep {
        self.note = (self.note + self.rng.gen_range(-2..=2)).clamp(0, 127);
        let note = quantize(self.note as u8, self.root, self.scale);
        self.note = note as i16;
        GeneratedStep { note: Some(note) }
    }
}

pub fn euclid(pulses: usize, steps: usize) -> Vec<bool> {
    if steps == 0 {
        return vec![];
    }
    let pulses = pulses.min(steps);
    (0..steps)
        .map(|i| ((i * pulses) % steps) < pulses)
        .collect()
}

pub struct Euclidean {
    pattern: Vec<bool>,
    note: u8,
}
impl Euclidean {
    pub fn new(pulses: usize, steps: usize, note: u8) -> Self {
        Self {
            pattern: euclid(pulses, steps),
            note,
        }
    }
}
impl Generator for Euclidean {
    fn next(&mut self, context: &GeneratorContext) -> GeneratedStep {
        let note = if self
            .pattern
            .get(context.step % self.pattern.len().max(1))
            .copied()
            .unwrap_or(false)
        {
            Some(self.note)
        } else {
            None
        };
        GeneratedStep { note }
    }
}

pub fn passes_probability(rng: &mut ChaCha8Rng, probability: f64) -> bool {
    probability >= 1.0 || (probability > 0.0 && rng.gen::<f64>() < probability)
}

pub fn track_rng(seed: u64, track_id: u64) -> ChaCha8Rng {
    ChaCha8Rng::seed_from_u64(seed ^ track_id.wrapping_mul(0x9E3779B97F4A7C15))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn euclidean_five_of_eight_has_five_hits() {
        let pattern = euclid(5, 8);
        assert_eq!(pattern.iter().filter(|&&x| x).count(), 5);
        assert_eq!(
            pattern,
            vec![true, false, true, false, true, true, false, true]
        );
    }
    #[test]
    fn sequence_wraps_and_keeps_rests() {
        let mut seq = Sequence::new(vec![Some(60), None, Some(64)]);
        let ctx = GeneratorContext { step: 0 };
        assert_eq!(seq.next(&ctx).note, Some(60));
        assert_eq!(seq.next(&ctx).note, None);
        assert_eq!(seq.next(&ctx).note, Some(64));
        assert_eq!(seq.next(&ctx).note, Some(60));
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
        let mut a = track_rng(666, 1);
        let mut b = track_rng(666, 1);
        let result_a: Vec<_> = (0..100).map(|_| passes_probability(&mut a, 0.5)).collect();
        let result_b: Vec<_> = (0..100).map(|_| passes_probability(&mut b, 0.5)).collect();
        assert_eq!(result_a, result_b);
        assert!(result_a.iter().any(|x| *x) && result_a.iter().any(|x| !x));
    }
}
