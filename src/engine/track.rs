use rand_chacha::ChaCha8Rng;

use crate::generators::{passes_probability, track_rng, Generator, GeneratorContext};

pub struct Track {
    pub name: String,
    pub channel: u8,
    pub enabled: bool,
    pub muted: bool,
    pub velocity: u8,
    pub duration: u32,
    pub generator: Box<dyn Generator>,
    pub probability: f64,
    pub step: usize,
    rng: ChaCha8Rng,
}

impl Track {
    pub fn new(
        id: u64,
        seed: u64,
        name: &str,
        channel: u8,
        velocity: u8,
        generator: Box<dyn Generator>,
    ) -> Self {
        Self {
            name: name.into(),
            channel,
            enabled: true,
            muted: false,
            velocity,
            duration: 5,
            generator,
            probability: 1.0,
            step: 0,
            rng: track_rng(seed, id),
        }
    }

    pub fn next(&mut self) -> Option<u8> {
        let generated = self.generator.next(&GeneratorContext { step: self.step });
        self.step = self.step.wrapping_add(1);
        generated
            .note
            .filter(|_| passes_probability(&mut self.rng, self.probability))
    }
}
