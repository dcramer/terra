//! A sound bed for a film (D357): the game's own sounds, made in code as they play
//! (`terra_world::audio::synth`), written to a WAV file rather than played, layered at the
//! times a film's places come on screen, with the game's music phrases over them. Scripted
//! runs are silent and step their clock, so a film cannot record what it would have played;
//! the sounds are the same generators, from seeds, so a bed is the same every time.
//!
//!   terra_sandbox --sound-bed shots/bed.wav --seconds 80 wind:0.4@0-20 birds:0.6@0-20 calm:0.8@1
//!
//! A layer is a sound (wind, birds, water, fire, hum, murmur) or a mood of the music (calm,
//! village, danger), its gain, and when it plays, in seconds: `@from-to` for a sound, which
//! fades in and out over a second and a half at each end; `@from` for a phrase of music,
//! which plays to its end. The mix is scaled so its peak is a decibel under full scale.

use bevy::audio::{Decodable, Source};
use std::io::Write;
use terra_game::play::music::Mood;
use terra_world::audio::synth::{RATE, Sound, Synth};

/// How long a sound takes to come in and to go.
const FADE_S: f32 = 1.5;

/// One layer of the bed.
#[derive(Clone, Debug, PartialEq)]
struct Layer {
    sound: Sound,
    gain: f32,
    from_s: f32,
    to_s: Option<f32>,
}

fn parse_layer(text: &str) -> Result<Layer, String> {
    let problem = || {
        format!(
            "a layer is a sound or mood, a gain and a time, like wind:0.4@0-20 or calm:0.8@2, not `{text}`"
        )
    };
    let (what, when) = text.split_once('@').unwrap_or((text, "0"));
    let (name, gain) = what.split_once(':').unwrap_or((what, "1"));
    let gain: f32 = gain.parse().map_err(|_| problem())?;
    let (from, to) = match when.split_once('-') {
        Some((from, to)) => (from, Some(to)),
        None => (when, None),
    };
    let from_s: f32 = from.parse().map_err(|_| problem())?;
    let to_s = to
        .map(|to| to.parse::<f32>().map_err(|_| problem()))
        .transpose()?;
    let sound = match name {
        "wind" => Sound::Wind,
        "birds" => Sound::Birds,
        "water" => Sound::Water,
        "fire" => Sound::Fire,
        "hum" => Sound::Hum,
        "murmur" => Sound::Murmur,
        "calm" => Sound::Music(Mood::Calm.phrase()),
        "village" => Sound::Music(Mood::Village.phrase()),
        "danger" => Sound::Music(Mood::Danger.phrase()),
        _ => return Err(problem()),
    };
    if !matches!(sound, Sound::Music(_)) && to_s.is_none_or(|to| to <= from_s) {
        return Err(format!("{text}: a sound needs when it ends, like @0-20"));
    }
    Ok(Layer {
        sound,
        gain,
        from_s,
        to_s,
    })
}

/// Mixes the layers into `seconds` of stereo, left and right interleaved.
fn mix(layers: &[Layer], seconds: f32) -> Vec<f32> {
    let frames = (seconds * RATE as f32) as usize;
    let mut out = vec![0.0f32; frames * 2];
    for (index, layer) in layers.iter().enumerate() {
        let seed = 0x5EED_0000 + index as u64 * 7919;
        let decoder = Synth {
            sound: layer.sound,
            seed,
        }
        .decoder();
        let channels = decoder.channels().get() as usize;
        let first = (layer.from_s * RATE as f32) as usize;
        let last = layer
            .to_s
            .map_or(frames, |to| ((to * RATE as f32) as usize).min(frames));
        let fades = layer.to_s.is_some();
        let length = last.saturating_sub(first);
        let fade = (FADE_S * RATE as f32) as usize;
        let mut samples = decoder;
        for i in 0..length {
            let (Some(left), right) = (
                samples.next(),
                (channels > 1).then(|| samples.next()).flatten(),
            ) else {
                break;
            };
            let right = right.unwrap_or(left);
            let envelope = if fades {
                let edge = i.min(length - 1 - i) as f32 / fade.max(1) as f32;
                edge.min(1.0)
            } else {
                1.0
            };
            let at = (first + i) * 2;
            out[at] += left * layer.gain * envelope;
            out[at + 1] += right * layer.gain * envelope;
        }
    }
    let peak = out.iter().fold(0.0f32, |most, s| most.max(s.abs()));
    if peak > 0.0 {
        // A decibel under full scale.
        let scale = 0.891 / peak;
        out.iter_mut().for_each(|s| *s *= scale);
    }
    out
}

/// Stereo samples as a 16-bit WAV file.
fn wav(samples: &[f32]) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Writes a bed from the command line after `--sound-bed`: the file, `--seconds`, then the
/// layers. Returns a problem in words.
pub fn run(args: &[String]) -> Result<String, String> {
    let mut args = args.iter();
    let path = args
        .next()
        .ok_or("--sound-bed wants a file, like shots/bed.wav")?;
    let mut seconds = None;
    let mut layers = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--seconds" {
            seconds = args.next().and_then(|s| s.parse::<f32>().ok());
            continue;
        }
        layers.push(parse_layer(arg)?);
    }
    let seconds = seconds
        .filter(|s| *s > 0.0)
        .ok_or("--sound-bed wants --seconds, how long the bed is")?;
    let samples = mix(&layers, seconds);
    let mut file = std::fs::File::create(path).map_err(|e| format!("{path}: {e}"))?;
    file.write_all(&wav(&samples))
        .map_err(|e| format!("{path}: {e}"))?;
    Ok(format!(
        "bed: {path}, {seconds:.1} s, {} layers",
        layers.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layers_are_read_with_their_gain_and_time() {
        let wind = parse_layer("wind:0.4@2-20").unwrap();
        assert_eq!(wind.sound, Sound::Wind);
        assert_eq!((wind.gain, wind.from_s, wind.to_s), (0.4, 2.0, Some(20.0)));
        assert!(matches!(
            parse_layer("calm:0.8@1").unwrap().sound,
            Sound::Music(_)
        ));
        assert!(parse_layer("wind:0.4@2").is_err(), "a sound needs an end");
        assert!(parse_layer("thunder:1@0-2").is_err());
    }

    /// A sound plays only within its time, coming in and going out gently, and the mix
    /// peaks a decibel under full scale.
    #[test]
    fn a_layer_sounds_only_in_its_time_and_fades() {
        let samples = mix(&[parse_layer("wind:1@1-3").unwrap()], 4.0);
        let at = |s: f32| (s * RATE as f32) as usize * 2;
        let loudest = |from: f32, to: f32| {
            samples[at(from)..at(to)]
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()))
        };
        assert_eq!(loudest(0.0, 0.99), 0.0, "sounding before its time");
        assert_eq!(loudest(3.01, 4.0), 0.0, "sounding after its time");
        assert!(loudest(1.9, 2.1) > 0.05, "silent in its time");
        // Coming in: its first tenth of a second far quieter than its middle.
        assert!(loudest(1.0, 1.1) < loudest(1.9, 2.1) * 0.3);
        let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 0.891).abs() < 1e-3, "{peak}");
    }

    #[test]
    fn a_wav_says_how_long_it_is() {
        let bytes = wav(&[0.0; 8]);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 16);
        assert_eq!(bytes.len(), 44 + 16);
    }
}
