use rand_chacha::ChaCha8Rng;

use crate::generators::{
    passes_probability, track_rng, Generator, GeneratorContext, GeneratorControl,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackControl {
    Channel,
    Probability,
    Velocity,
    GateSteps,
    Generator(GeneratorControl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlValue {
    pub key: TrackControl,
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrackSnapshot {
    pub name: String,
    pub channel: u8,
    pub muted: bool,
    pub enabled: bool,
    pub step: usize,
    pub description: String,
    pub controls: Vec<ControlValue>,
}

#[derive(Clone, Copy, Debug)]
pub struct TrackOptions {
    pub velocity: u8,
    pub gate_steps: u8,
    pub probability: f64,
}

pub struct Track {
    pub name: String,
    pub channel: u8,
    pub enabled: bool,
    pub muted: bool,
    pub velocity: u8,
    pub gate_steps: u8,
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
        options: TrackOptions,
        generator: Box<dyn Generator>,
    ) -> Self {
        Self {
            name: name.into(),
            channel: channel.min(15),
            enabled: true,
            muted: false,
            velocity: options.velocity.clamp(1, 127),
            gate_steps: options.gate_steps.clamp(1, 16),
            generator,
            probability: options.probability.clamp(0.0, 1.0),
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

    pub fn controls(&self) -> Vec<ControlValue> {
        let mut controls = vec![
            ControlValue {
                key: TrackControl::Channel,
                label: "Channel".into(),
                value: (self.channel + 1).to_string(),
            },
            ControlValue {
                key: TrackControl::Probability,
                label: "Chance".into(),
                value: format!("{}%", (self.probability * 100.0).round() as u8),
            },
            ControlValue {
                key: TrackControl::Velocity,
                label: "Velocity".into(),
                value: self.velocity.to_string(),
            },
            ControlValue {
                key: TrackControl::GateSteps,
                label: "Gate (16ths)".into(),
                value: self.gate_steps.to_string(),
            },
        ];
        controls.extend(
            self.generator
                .controls()
                .into_iter()
                .map(|control| ControlValue {
                    key: TrackControl::Generator(control.key),
                    label: control.label,
                    value: control.value,
                }),
        );
        controls
    }

    pub fn adjust(&mut self, control: TrackControl, direction: i8) {
        match control {
            TrackControl::Channel => {
                self.channel =
                    (i16::from(self.channel) + i16::from(direction)).rem_euclid(16) as u8;
            }
            TrackControl::Probability => {
                let current = (self.probability * 20.0).round() as i16;
                let next = (current + i16::from(direction)).clamp(0, 20);
                self.probability = f64::from(next) / 20.0;
            }
            TrackControl::Velocity => {
                self.velocity =
                    (i16::from(self.velocity) + i16::from(direction)).clamp(1, 127) as u8;
            }
            TrackControl::GateSteps => {
                self.gate_steps =
                    (i16::from(self.gate_steps) + i16::from(direction)).clamp(1, 16) as u8;
            }
            TrackControl::Generator(generator_control) => {
                self.generator.adjust(generator_control, direction);
            }
        }
    }

    pub fn toggle_rest(&mut self, control: TrackControl) {
        if let TrackControl::Generator(generator_control) = control {
            self.generator.toggle_rest(generator_control);
        }
    }

    pub fn snapshot(&self) -> TrackSnapshot {
        TrackSnapshot {
            name: self.name.clone(),
            channel: self.channel,
            muted: self.muted,
            enabled: self.enabled,
            step: self.step,
            description: self.generator.description(),
            controls: self.controls(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::generators::Sequence;

    use super::*;

    #[test]
    fn common_controls_adjust_within_valid_ranges() {
        let mut track = Track::new(
            1,
            666,
            "TEST",
            0,
            TrackOptions {
                velocity: 1,
                gate_steps: 1,
                probability: 0.0,
            },
            Box::new(Sequence::new(vec![Some(60)])),
        );
        track.adjust(TrackControl::Channel, -1);
        track.adjust(TrackControl::Probability, -1);
        track.adjust(TrackControl::Velocity, -1);
        track.adjust(TrackControl::GateSteps, -1);

        assert_eq!(track.channel, 15);
        assert_eq!(track.probability, 0.0);
        assert_eq!(track.velocity, 1);
        assert_eq!(track.gate_steps, 1);
    }
}
