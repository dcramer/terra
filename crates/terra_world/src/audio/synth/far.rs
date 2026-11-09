//! The far realm's air (docs/realm-two.md, D642): where the first realm has birdsong, its cap
//! forest rings: soft glassy notes from all round, now and then, each a struck tone that
//! rings on and dies away, over a low breath of two tones beating slowly. A bed, generated
//! as the others are.

use super::{DT, Dice, soft};
use std::f32::consts::TAU;

/// The notes it rings, hertz: a scale of five with no semitone, so any two sound well
/// together however they fall.
const NOTES: [f32; 8] = [293.7, 329.6, 392.0, 440.0, 493.9, 587.3, 659.3, 784.0];
/// How many notes ring at once at most.
const VOICES: usize = 6;

/// One note ringing: its pitch, how far through its turn it is, how long it has rung, how
/// fast it dies, how loud it was struck and where it is, left to right.
#[derive(Clone, Copy, Default)]
struct Note {
    hz: f32,
    phase: f32,
    rung_s: f32,
    dies: f32,
    loud: f32,
    pan: f32,
}

impl Note {
    /// Its sound now: its tone, a bell's higher partial that dies sooner, a soft onset.
    fn sound(&mut self) -> f32 {
        if self.loud <= 0.0 {
            return 0.0;
        }
        self.rung_s += DT;
        self.phase = (self.phase + self.hz * DT) % 1.0;
        let onset = (self.rung_s / 0.02).min(1.0);
        let body = libm::expf(-self.rung_s * self.dies);
        let glass = libm::expf(-self.rung_s * self.dies * 3.0);
        if body < 0.001 {
            self.loud = 0.0;
        }
        let tone = libm::sinf(TAU * self.phase);
        let over = libm::sinf(TAU * ((self.phase * 2.76) % 1.0));
        (tone * body + over * glass * 0.35) * onset * self.loud
    }
}

/// The far realm's air, in stereo.
pub(super) fn chimes(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    let mut notes = [Note::default(); VOICES];
    let mut quiet = dice.range(0.5, 2.0);
    let mut t = 0.0f32;
    let mut frame = [0.0f32; 2];
    let mut channel = 0;
    std::iter::from_fn(move || {
        if channel == 0 {
            t += DT;
            quiet -= DT;
            if quiet <= 0.0 {
                quiet = dice.range(0.8, 4.5);
                if let Some(free) = notes.iter_mut().find(|note| note.loud <= 0.0) {
                    let octave = if dice.range(0.0, 1.0) < 0.3 { 2.0 } else { 1.0 };
                    *free = Note {
                        hz: NOTES
                            [(dice.range(0.0, NOTES.len() as f32) as usize).min(NOTES.len() - 1)]
                            * octave,
                        phase: 0.0,
                        rung_s: 0.0,
                        dies: dice.range(0.5, 1.1),
                        loud: dice.range(0.05, 0.14),
                        pan: dice.range(-0.85, 0.85),
                    };
                }
            }
            // The breath under them: two low tones a fraction apart, swelling slowly.
            let swell = 0.6 + 0.4 * libm::sinf(TAU * 0.07 * t);
            let low = (libm::sinf(TAU * ((98.0 * t) % 1.0)) + libm::sinf(TAU * ((98.7 * t) % 1.0)))
                * 0.022
                * swell;
            let (mut left, mut right) = (low, low);
            for note in &mut notes {
                let out = note.sound();
                left += out * (1.0 - note.pan) * 0.7;
                right += out * (1.0 + note.pan) * 0.7;
            }
            frame = [soft(left), soft(right)];
        }
        let sample = frame[channel];
        channel = 1 - channel;
        Some(sample)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// It rings: quiet between notes and never loud, both sides heard, and no two seeds the
    /// same; over a minute several notes are struck and each dies away.
    #[test]
    fn the_far_realms_air_rings_softly_from_both_sides() {
        let minute: Vec<f32> = chimes(Dice::new(3))
            .take(2 * 60 * super::super::RATE as usize)
            .collect();
        let peak = minute
            .iter()
            .fold(0.0f32, |most, sample| most.max(sample.abs()));
        assert!(peak > 0.03 && peak < 0.5, "its loudest {peak}");
        let side = |which: usize| {
            minute
                .iter()
                .skip(which)
                .step_by(2)
                .map(|s| s * s)
                .sum::<f32>()
        };
        let (left, right) = (side(0), side(1));
        assert!(
            left > 0.0 && right > 0.0 && (left / right - 1.0).abs() < 0.8,
            "{left} and {right}"
        );
        // Loudness by second: some seconds ring and some are near silent.
        let second = 2 * super::super::RATE as usize;
        let loud: Vec<f32> = minute
            .chunks(second)
            .map(|part| part.iter().fold(0.0f32, |most, s| most.max(s.abs())))
            .collect();
        assert!(
            loud.iter().filter(|most| **most > 0.06).count() >= 8,
            "{loud:?}"
        );
        assert!(loud.iter().any(|most| *most < 0.06), "{loud:?}");
        let other: Vec<f32> = chimes(Dice::new(4)).take(second * 10).collect();
        assert_ne!(other[..], minute[..second * 10]);
    }
}
