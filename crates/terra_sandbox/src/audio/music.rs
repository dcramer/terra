//! Music made in code: one phrase at a time, from a seed and a style, so it never
//! repeats exactly and can follow the place. A phrase is a few plucked notes, a held
//! chord under them that swells and fades, and sometimes a low drone, in an echo.
//!
//! Which style plays, and how long the silence between phrases is, belongs to the game
//! (the play experiment's `music.rs`). This only turns a phrase into sound.

use super::synth::{Dice, RATE, Smooth, hump, soft};
use std::f32::consts::TAU;

const DT: f32 = 1.0 / RATE as f32;
/// How long the echo rings on after the last note.
const TAIL_S: f32 = 4.0;
/// The most plucked notes sounding at once.
const STRINGS: usize = 6;
/// The longest string, for the lowest note: about 55 Hz.
const LONGEST: usize = (RATE / 55) as usize + 2;

/// What a phrase is made of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phrase {
    /// The note everything is built on, in hertz.
    pub root_hz: f32,
    /// The notes the melody may use, in semitones above the root.
    pub scale: &'static [i32],
    /// How long the notes go on for, in seconds, before the echo dies away.
    pub seconds: f32,
    /// How many plucked notes, at most and at least.
    pub notes: (u32, u32),
    /// Octaves above the root that the melody keeps within.
    pub register: (i32, i32),
    /// How loud the held chord is, and the low drone, from 0.
    pub pad: f32,
    pub drone: f32,
}

impl Phrase {
    /// From the first note to silence.
    pub fn length_s(&self) -> f32 {
        self.seconds + TAIL_S
    }
}

fn hertz(root_hz: f32, semitones: i32) -> f32 {
    root_hz * libm::exp2f(semitones as f32 / 12.0)
}

/// A plucked string (Karplus and Strong): a burst of noise going round a loop that
/// softens it each time, which rings like a harp.
struct Pluck {
    ring: Vec<f32>,
    length: usize,
    at: usize,
    loud: f32,
}

impl Pluck {
    fn new() -> Self {
        Pluck {
            ring: vec![0.0; LONGEST],
            length: 1,
            at: 0,
            loud: 0.0,
        }
    }

    fn pluck(&mut self, hz: f32, loud: f32, dice: &mut Dice) {
        self.length = ((RATE as f32 / hz) as usize).clamp(2, LONGEST);
        // Softened noise, so the attack is a pluck rather than a buzz.
        let mut soft_noise = Smooth::default();
        for sample in &mut self.ring[..self.length] {
            *sample = soft_noise.low(dice.noise(), 2500.0);
        }
        self.at = 0;
        self.loud = loud;
    }

    fn next(&mut self) -> f32 {
        if self.loud <= 0.0 {
            return 0.0;
        }
        let next = (self.at + 1) % self.length;
        let out = self.ring[self.at];
        self.ring[self.at] = 0.4985 * (out + self.ring[next]);
        self.at = next;
        out * self.loud
    }
}

/// An echo for each side, so notes hang in the air.
struct Echo {
    line: Vec<f32>,
    at: usize,
    tone: Smooth,
}

impl Echo {
    fn new(seconds: f32) -> Self {
        Echo {
            line: vec![0.0; (seconds * RATE as f32) as usize],
            at: 0,
            tone: Smooth::default(),
        }
    }

    fn next(&mut self, input: f32) -> f32 {
        let out = self.line[self.at];
        self.line[self.at] = input + self.tone.low(out, 2200.0) * 0.42;
        self.at = (self.at + 1) % self.line.len();
        out
    }
}

/// The samples of a phrase, left and right in turn.
pub fn play(phrase: Phrase, mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    // Lay the notes out before anything sounds: when, how high, how hard.
    let count =
        phrase.notes.0 + (dice.next() % (phrase.notes.1 - phrase.notes.0 + 1) as u64) as u32;
    let degrees = phrase.scale.len() as i32;
    let lowest = phrase.register.0 * degrees;
    let highest = phrase.register.1 * degrees;
    let mut step = (lowest + highest) / 2;
    let mut notes: Vec<(usize, f32, f32)> = (0..count)
        .map(|_| {
            // Mostly small steps, now and then a leap.
            let leap = match dice.next() % 6 {
                0 => 3,
                1 => -3,
                2 | 3 => 1,
                _ => -1,
            };
            step = (step + leap).clamp(lowest, highest);
            let octave = step.div_euclid(degrees);
            let semitones = 12 * octave + phrase.scale[step.rem_euclid(degrees) as usize];
            let start = dice.range(0.08, 0.82) * phrase.seconds;
            (
                (start * RATE as f32) as usize,
                hertz(phrase.root_hz, semitones),
                dice.range(0.35, 0.8),
            )
        })
        .collect();
    notes.sort_by_key(|note| note.0);

    let chord = [0, 7, 14].map(|semitones| hertz(phrase.root_hz, semitones));
    let last = (phrase.length_s() * RATE as f32) as usize;
    let mut strings: Vec<Pluck> = (0..STRINGS).map(|_| Pluck::new()).collect();
    let mut next_string = 0;
    let mut next_note = 0;
    let mut echoes = [Echo::new(0.33), Echo::new(0.47)];
    let mut sample = 0usize;
    let mut frame = [0.0f32; 2];
    let mut channel = 0;
    std::iter::from_fn(move || {
        // A frame is left and right together: the phrase ends only between frames.
        if channel == 0 && sample >= last {
            return None;
        }
        if channel == 0 {
            while next_note < notes.len() && notes[next_note].0 <= sample {
                let (_, hz, loud) = notes[next_note];
                strings[next_string].pluck(hz, loud, &mut dice);
                next_string = (next_string + 1) % STRINGS;
                next_note += 1;
            }
            let t = sample as f32 * DT;
            let tone = |hz: f32| libm::sinf(TAU * ((hz * t) % 1.0));
            let through = t / phrase.seconds;
            let mut held = 0.0;
            if phrase.pad > 0.0 {
                // Each chord tone twice, a hair apart, so it shimmers.
                for hz in chord {
                    held += tone(hz) + tone(hz * 1.003);
                }
                held *= phrase.pad * 0.05 * hump(through.min(1.0));
            }
            if phrase.drone > 0.0 {
                let low = phrase.root_hz * 0.5;
                let throb = 0.75 + 0.25 * libm::sinf(TAU * 0.4 * t);
                let rub = hump(through.min(1.0) * 1.6 - 0.3) * 0.5;
                held +=
                    (tone(low) + 0.6 * tone(low * 1.5) + rub * tone(low * libm::exp2f(1.0 / 12.0)))
                        * phrase.drone
                        * 0.12
                        * throb
                        * hump((through * 0.9 + 0.05).min(1.0));
            }
            let plucked: f32 = strings.iter_mut().map(Pluck::next).sum::<f32>() * 0.5;
            let dry = plucked + held;
            let wet = [echoes[0].next(dry), echoes[1].next(dry)];
            frame = [
                soft(dry + wet[0] * 0.5) * 0.8,
                soft(dry + wet[1] * 0.5) * 0.8,
            ];
            sample += 1;
        }
        let out = frame[channel];
        channel = 1 - channel;
        Some(out)
    })
}

#[cfg(test)]
mod tests {
    use super::super::synth::{Sound, tests::loudness, tests::render};
    use super::*;

    const PENTATONIC: &[i32] = &[0, 2, 4, 7, 9];

    fn phrase() -> Phrase {
        Phrase {
            root_hz: 146.8,
            scale: PENTATONIC,
            seconds: 6.0,
            notes: (4, 6),
            register: (1, 3),
            pad: 1.0,
            drone: 1.0,
        }
    }

    #[test]
    fn a_phrase_plays_for_its_length_and_ends() {
        let (samples, channels) = render(Sound::Music(phrase()), 9, 60.0);
        let expected = (phrase().length_s() * RATE as f32) as usize * channels as usize;
        assert_eq!(samples.len(), expected);
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(loudness(&samples) > 0.005);
    }

    #[test]
    fn pitches_are_whole_semitones_from_the_root() {
        for semitones in [0, 2, 4, 7, 9, 12, 14] {
            let hz = hertz(110.0, semitones);
            let back = libm::roundf(12.0 * libm::log2f(hz / 110.0)) as i32;
            assert_eq!(back, semitones);
        }
    }
}
