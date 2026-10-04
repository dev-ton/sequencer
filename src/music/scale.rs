#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scale {
    Chromatic,
    Major,
    NaturalMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
}

impl Scale {
    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Self::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
        }
    }
}

pub fn quantize(note: u8, root: u8, scale: Scale) -> u8 {
    let root = root % 12;
    let pitch = note.min(127);
    let mut best = pitch;
    let mut distance = u8::MAX;
    for candidate in 0..=127 {
        if scale.intervals().contains(&((candidate + 12 - root) % 12)) {
            let d = candidate.abs_diff(pitch);
            if d < distance || (d == distance && candidate < best) {
                best = candidate;
                distance = d;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quantizes_to_nearest_d_dorian_pitch() {
        assert_eq!(quantize(54, 2, Scale::Dorian), 53);
        assert_eq!(quantize(56, 2, Scale::Dorian), 57);
        assert_eq!(quantize(60, 2, Scale::Dorian), 60);
    }
}
