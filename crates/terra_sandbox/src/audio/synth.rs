//! Sounds made in code rather than recorded: the air, birds, water, fire, the hum of
//! pooled magic, a few short ones, and music (`music.rs`). They are generated as they
//! play, from a seed, so a loop never repeats and a place can sound like itself.
//!
//! Each is a `Synth` asset; Bevy plays it like a recorded clip. Everything here runs on
//! the sound thread, so it is plain arithmetic with no allocation once started.

use super::music::{self, Phrase};
use bevy::{
    audio::{ChannelCount, Decodable, SampleRate, Source},
    prelude::*,
    reflect::TypePath,
};
use std::f32::consts::{PI, TAU};
use std::time::Duration;

/// Samples a second, for everything made here.
pub const RATE: u32 = 44_100;
const DT: f32 = 1.0 / RATE as f32;

/// What to generate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sound {
    /// Moving air, in gusts. Never ends.
    Wind,
    /// Small birds calling now and then, left and right. Never ends.
    Birds,
    /// Water lapping at a shore. Never ends.
    Water,
    /// A fire: crackles over a low roar. Never ends.
    Fire,
    /// Something wrong where magic pools: low beating tones and a whisper. Never ends.
    Hum,
    /// Air cut by a swing.
    Whoosh,
    /// A spider's warning.
    Hiss,
    /// A startled deer.
    Snort,
    /// A foot in shallow water.
    Splash,
    /// A phrase of music.
    Music(Phrase),
}

impl Sound {
    /// Whether it plays on until stopped.
    pub fn endless(self) -> bool {
        matches!(
            self,
            Sound::Wind | Sound::Birds | Sound::Water | Sound::Fire | Sound::Hum
        )
    }
}

/// A sound to generate, and the seed that makes this one differ from the last.
#[derive(Asset, TypePath, Clone, Copy, Debug)]
pub struct Synth {
    pub sound: Sound,
    pub seed: u64,
}

impl Decodable for Synth {
    type Decoder = Generated;

    fn decoder(&self) -> Generated {
        let mut dice = Dice::new(self.seed);
        let (samples, channels, length): (Box<dyn Iterator<Item = f32> + Send>, u16, _) =
            match self.sound {
                Sound::Wind => (Box::new(wind(dice)), 2, None),
                Sound::Birds => (Box::new(birds(dice)), 2, None),
                Sound::Water => (Box::new(water(dice)), 2, None),
                Sound::Fire => (Box::new(fire(dice)), 1, None),
                Sound::Hum => (Box::new(hum(dice)), 1, None),
                Sound::Whoosh => one_shot(0.3, whoosh(&mut dice)),
                Sound::Hiss => one_shot(0.55, hiss(&mut dice)),
                Sound::Snort => one_shot(0.4, snort(&mut dice)),
                Sound::Splash => one_shot(0.32, splash(&mut dice)),
                Sound::Music(phrase) => (
                    Box::new(music::play(phrase, dice)),
                    2,
                    Some(phrase.length_s()),
                ),
            };
        Generated {
            samples,
            channels,
            length,
        }
    }
}

fn one_shot(
    seconds: f32,
    shape: impl FnMut(f32) -> f32 + Send + 'static,
) -> (Box<dyn Iterator<Item = f32> + Send>, u16, Option<f32>) {
    let mut shape = shape;
    let count = (seconds * RATE as f32) as usize;
    let samples = (0..count).map(move |i| shape(i as f32 * DT));
    (Box::new(samples), 1, Some(seconds))
}

/// A generated sound as Bevy plays it.
pub struct Generated {
    samples: Box<dyn Iterator<Item = f32> + Send>,
    channels: u16,
    length: Option<f32>,
}

impl Iterator for Generated {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        self.samples.next()
    }
}

impl Source for Generated {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::new(self.channels).unwrap_or(ChannelCount::MIN)
    }

    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap_or(SampleRate::MIN)
    }

    fn total_duration(&self) -> Option<Duration> {
        self.length.map(Duration::from_secs_f32)
    }
}

/// A small, fast random source for the sound thread.
#[derive(Clone, Copy)]
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Self {
        Dice(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// From 0 up to 1.
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    /// White noise, from -1 to 1.
    pub fn noise(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// A one-pole low-pass filter.
#[derive(Clone, Copy, Default)]
pub struct Smooth {
    pub out: f32,
}

impl Smooth {
    /// Moves toward `input`, with `cutoff` in hertz.
    pub fn low(&mut self, input: f32, cutoff: f32) -> f32 {
        let a = 1.0 - libm::expf(-TAU * cutoff * DT);
        self.out += a * (input - self.out);
        self.out
    }

    /// What `low` leaves out: the part of `input` above `cutoff`.
    pub fn high(&mut self, input: f32, cutoff: f32) -> f32 {
        input - self.low(input, cutoff)
    }
}

/// A band-pass filter (Chamberlin's state-variable filter).
#[derive(Clone, Copy, Default)]
pub struct Band {
    low: f32,
    band: f32,
}

impl Band {
    /// The part of `input` near `centre` hertz; a larger `width` lets more through.
    pub fn pass(&mut self, input: f32, centre: f32, width: f32) -> f32 {
        let f = 2.0 * libm::sinf(PI * centre.min(RATE as f32 / 6.0) * DT);
        let high = input - self.low - width * self.band;
        self.band += f * high;
        self.low += f * self.band;
        self.band
    }
}

/// Pink-ish noise: white noise with the highs rolled off, like air rather than hiss.
#[derive(Clone, Copy, Default)]
struct Pink {
    b: [f32; 3],
}

impl Pink {
    fn next(&mut self, white: f32) -> f32 {
        self.b[0] = 0.99765 * self.b[0] + white * 0.099_046;
        self.b[1] = 0.963 * self.b[1] + white * 0.296_516_4;
        self.b[2] = 0.57 * self.b[2] + white * 1.052_691_3;
        (self.b[0] + self.b[1] + self.b[2] + white * 0.1848) * 0.2
    }
}

/// A slow wander between `low` and `high`, picking a new aim every so often.
#[derive(Clone, Copy)]
struct Drift {
    aim: f32,
    now: Smooth,
    left: f32,
    every: (f32, f32),
    span: (f32, f32),
}

impl Drift {
    fn new(span: (f32, f32), every: (f32, f32)) -> Self {
        Drift {
            aim: span.0,
            now: Smooth { out: span.0 },
            left: 0.0,
            every,
            span,
        }
    }

    fn next(&mut self, dice: &mut Dice, speed_hz: f32) -> f32 {
        self.left -= DT;
        if self.left <= 0.0 {
            self.left = dice.range(self.every.0, self.every.1);
            self.aim = dice.range(self.span.0, self.span.1);
        }
        self.now.low(self.aim, speed_hz)
    }
}

/// A rise and fall over `0..1`, smooth at both ends.
pub fn hump(t: f32) -> f32 {
    if (0.0..=1.0).contains(&t) {
        let s = libm::sinf(PI * t);
        s * s
    } else {
        0.0
    }
}

/// Keeps a sum of voices from clipping, gently.
pub fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

fn wind(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    let mut gust = Drift::new((0.25, 1.0), (1.0, 4.0));
    let mut tone = [Smooth::default(); 2];
    let mut pink = [Pink::default(); 2];
    let mut channel = 0;
    let mut level = 0.0;
    let mut cutoff = 0.0;
    std::iter::from_fn(move || {
        if channel == 0 {
            level = gust.next(&mut dice, 0.4);
            cutoff = 120.0 + 700.0 * level * level;
        }
        let air = pink[channel].next(dice.noise());
        let out = tone[channel].low(air, cutoff) * level * 1.6;
        channel = 1 - channel;
        Some(soft(out))
    })
}

fn birds(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    // One call at a time: a few notes, each a quick sweep, from somewhere left or right.
    let mut quiet = dice.range(0.5, 3.0);
    let mut notes: Vec<(f32, f32, f32, f32)> = Vec::with_capacity(12);
    let mut t = 0.0;
    let mut phase = 0.0;
    let mut pan = 0.0;
    let mut loud = 0.0;
    let mut frame = [0.0f32; 2];
    let mut channel = 0;
    std::iter::from_fn(move || {
        if channel == 0 {
            let mut out = 0.0;
            if notes.is_empty() {
                quiet -= DT;
                if quiet <= 0.0 {
                    call(&mut dice, &mut notes);
                    t = 0.0;
                    pan = dice.range(-0.8, 0.8);
                    loud = dice.range(0.06, 0.2);
                    quiet = dice.range(0.4, 4.5);
                }
            } else {
                t += DT;
                // (start, length, from hertz, to hertz)
                if let Some(&(start, length, from, to)) = notes
                    .iter()
                    .find(|(start, length, ..)| t >= *start && t < start + length)
                {
                    let through = (t - start) / length;
                    phase = (phase + (from + (to - from) * through) * DT) % 1.0;
                    out = libm::sinf(TAU * phase) * hump(through) * loud;
                }
                if notes.iter().all(|(start, length, ..)| t > start + length) {
                    notes.clear();
                }
            }
            frame = [out * (1.0 - pan) * 0.7, out * (1.0 + pan) * 0.7];
        }
        let sample = frame[channel];
        channel = 1 - channel;
        Some(sample)
    })
}

/// Lays out the notes of one bird's call.
fn call(dice: &mut Dice, notes: &mut Vec<(f32, f32, f32, f32)>) {
    let mut at = 0.0;
    match dice.next() % 3 {
        // Rising tweets.
        0 => {
            for _ in 0..(2 + dice.next() % 4) {
                let low = dice.range(2400.0, 3200.0);
                notes.push((at, 0.07, low, low + dice.range(900.0, 1600.0)));
                at += dice.range(0.1, 0.16);
            }
        }
        // A falling trill.
        1 => {
            let mut high = dice.range(4200.0, 5200.0);
            for _ in 0..(5 + dice.next() % 5) {
                notes.push((at, 0.035, high, high - 500.0));
                high -= 120.0;
                at += 0.05;
            }
        }
        // Two notes, high then low.
        _ => {
            let high = dice.range(3000.0, 3600.0);
            notes.push((0.0, 0.13, high, high + 80.0));
            notes.push((0.18, 0.2, high * 0.78, high * 0.76));
        }
    }
}

fn water(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    let mut swell = Drift::new((0.2, 1.0), (1.2, 3.5));
    let mut body = [Smooth::default(); 2];
    let mut pink = [Pink::default(); 2];
    let mut blip = Band::default();
    let mut blip_left = 0.0;
    let mut blip_at = 1000.0;
    let mut blip_for = 0.0;
    let mut channel = 0;
    let mut level = 0.0;
    std::iter::from_fn(move || {
        if channel == 0 {
            level = swell.next(&mut dice, 0.8);
            blip_left -= DT;
            if blip_left <= 0.0 {
                blip_left = dice.range(0.15, 1.2);
                blip_at = dice.range(700.0, 1700.0);
                blip_for = 0.05;
            }
            blip_for -= DT;
        }
        let rush = body[channel].low(pink[channel].next(dice.noise()), 280.0 + 500.0 * level);
        let mut out = rush * level * 1.4;
        if blip_for > 0.0 {
            out += blip.pass(dice.noise(), blip_at, 0.15) * 0.12 * (blip_for / 0.05);
        }
        channel = 1 - channel;
        Some(soft(out))
    })
}

fn fire(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    let mut roar = Smooth::default();
    let mut flicker = Drift::new((0.4, 1.0), (0.1, 0.5));
    let mut crack = 0.0f32;
    let mut crack_tone = Smooth::default();
    std::iter::from_fn(move || {
        // About a dozen crackles a second, of every size.
        if dice.unit() < 12.0 * DT {
            let size = dice.unit();
            crack += size * size * 0.9;
        }
        crack *= 0.996;
        let pop = crack_tone.high(dice.noise() * crack, 1500.0);
        let bed = roar.low(dice.noise(), 260.0) * 0.5 * flicker.next(&mut dice, 3.0);
        Some(soft(pop + bed))
    })
}

fn hum(mut dice: Dice) -> impl Iterator<Item = f32> + Send {
    let mut t = 0.0f32;
    let mut whisper = Band::default();
    let mut breath = Drift::new((0.0, 1.0), (1.5, 4.0));
    std::iter::from_fn(move || {
        t += DT;
        let tone = |hz: f32| libm::sinf(TAU * ((hz * t) % 1.0));
        let swell = 0.7 + 0.3 * libm::sinf(TAU * 0.13 * t);
        let low = (tone(55.0) + 0.7 * tone(57.3) + 0.35 * tone(110.4)) * 0.22 * swell;
        let air = whisper.pass(dice.noise(), 620.0, 0.3) * 0.08 * breath.next(&mut dice, 0.5);
        Some(soft(low + air))
    })
}

fn whoosh(dice: &mut Dice) -> impl FnMut(f32) -> f32 + Send + 'static {
    let mut dice = *dice;
    let mut band = Band::default();
    move |t| {
        let through = t / 0.3;
        let centre = 350.0 + 2200.0 * through;
        band.pass(dice.noise(), centre, 0.6) * hump(libm::powf(through, 0.7)) * 0.9
    }
}

fn hiss(dice: &mut Dice) -> impl FnMut(f32) -> f32 + Send + 'static {
    let mut dice = *dice;
    let mut tone = Smooth::default();
    move |t| {
        let through = t / 0.55;
        let shake = 0.6 + 0.4 * libm::sinf(TAU * 19.0 * t);
        tone.high(dice.noise(), 2800.0) * hump(through) * shake * 0.7
    }
}

fn snort(dice: &mut Dice) -> impl FnMut(f32) -> f32 + Send + 'static {
    let mut dice = *dice;
    let mut tone = Smooth::default();
    move |t| {
        let puff = |from: f32| hump((t - from) / 0.13);
        tone.low(dice.noise(), 900.0) * (puff(0.0) + 0.8 * puff(0.2)) * 1.4
    }
}

fn splash(dice: &mut Dice) -> impl FnMut(f32) -> f32 + Send + 'static {
    let mut dice = *dice;
    let mut band = Band::default();
    let bubble = dice.range(500.0, 800.0);
    move |t| {
        let fall = libm::expf(-t * 14.0);
        let rush = band.pass(dice.noise(), 1600.0, 0.8) * fall * (t * 200.0).min(1.0);
        let pop = if (0.08..0.12).contains(&t) {
            libm::sinf(TAU * (bubble + 4000.0 * (t - 0.08)) * t) * 0.15
        } else {
            0.0
        };
        (rush + pop) * 0.8
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The first so many seconds of a sound.
    pub fn render(sound: Sound, seed: u64, seconds: f32) -> (Vec<f32>, u16) {
        let decoder = Synth { sound, seed }.decoder();
        let channels = decoder.channels().get();
        let count = (seconds * RATE as f32) as usize * channels as usize;
        (decoder.take(count).collect(), channels)
    }

    pub fn loudness(samples: &[f32]) -> f32 {
        libm::sqrtf(samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32)
    }

    #[test]
    fn every_sound_is_finite_in_range_and_not_silent() {
        for sound in [
            Sound::Wind,
            Sound::Birds,
            Sound::Water,
            Sound::Fire,
            Sound::Hum,
            Sound::Whoosh,
            Sound::Hiss,
            Sound::Snort,
            Sound::Splash,
        ] {
            let (samples, _) = render(sound, 7, 6.0);
            assert!(
                samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                "{sound:?} went out of range"
            );
            assert!(loudness(&samples) > 0.003, "{sound:?} is silent");
        }
    }

    #[test]
    fn short_sounds_end_and_long_ones_do_not() {
        let (splash, _) = render(Sound::Splash, 1, 5.0);
        assert_eq!(splash.len(), (0.32 * RATE as f32) as usize);
        assert!(!Sound::Splash.endless());
        let (wind, channels) = render(Sound::Wind, 1, 5.0);
        assert_eq!(wind.len(), 5 * RATE as usize * channels as usize);
        assert!(Sound::Wind.endless());
    }

    #[test]
    fn the_same_seed_makes_the_same_sound_and_another_does_not() {
        let (a, _) = render(Sound::Birds, 3, 8.0);
        let (b, _) = render(Sound::Birds, 3, 8.0);
        let (c, _) = render(Sound::Birds, 4, 8.0);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
