//! The dusk sky's rule (`assets/shaders/dusk.wesl`) on the CPU, line for line, so it can be
//! tested without a GPU: the shader draws it, this checks what it draws. The two share their
//! numbers in order, which a test holds.

use bevy::math::Vec3;
use terra_gen::smoothstep;

fn afterglow(horizon: Vec3, glow: Vec3) -> Vec3 {
    let hue = mix(
        glow / glow.x.max(glow.y).max(glow.z.max(1.0e-4)),
        Vec3::new(1.0, 0.46, 0.13),
        0.6,
    );
    hue * horizon.dot(Vec3::new(0.2126, 0.7152, 0.0722)) * 1.6
}

pub(super) fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

pub(super) fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub(crate) fn dusk_sky(
    direction: Vec3,
    up: Vec3,
    sun: Vec3,
    horizon: Vec3,
    zenith: Vec3,
    glow: Vec3,
) -> Vec3 {
    let lift = direction.dot(up);
    let flat_sun = (sun - up * sun.dot(up) + up * 1e-4).normalize();
    let flat_way = (direction - up * lift + up * 1e-4).normalize();
    let bearing = flat_way.dot(flat_sun).max(0.0);
    let toward = bearing;
    let band = 1.0 - smoothstep(0.0, mixf(0.06, 0.3, toward), lift);
    let pale =
        Vec3::splat(horizon.dot(Vec3::new(0.2126, 0.7152, 0.0722))) * Vec3::new(0.86, 0.97, 1.15);
    let warm = mix(horizon, afterglow(horizon, glow), toward);
    let mut low = mix(pale, warm, band * mixf(0.15, 1.0, toward));
    let away = (-flat_way.dot(flat_sun)).max(0.0);
    let belt = smoothstep(0.04, 0.14, lift) * (1.0 - smoothstep(0.16, 0.34, lift));
    let venus =
        Vec3::splat(horizon.dot(Vec3::new(0.2126, 0.7152, 0.0722))) * Vec3::new(1.3, 0.8, 0.92);
    let earth = (1.0 - smoothstep(0.02, 0.1, lift)) * away;
    let shadowed = (zenith + Vec3::splat(1.0e-4)).normalize() * pale.length() * 0.7;
    low = mix(low, venus, belt * away);
    low = mix(low, shadowed, earth * 0.5);
    let deep = zenith * Vec3::new(0.8, 1.0, 1.0);
    let mut colour = mix(
        low,
        deep,
        libm::powf(smoothstep(-0.02, 0.55, lift), 0.5) * (1.0 - belt * away * 0.8),
    );
    colour += glow
        * libm::powf(direction.dot(sun).max(0.0), 5.0)
        * 0.35
        * (1.0 - smoothstep(0.0, 0.5, lift));
    colour
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A linear colour's hue as shown (sRGB), degrees: 0 red, 30 orange, 60 yellow, 300 mauve.
    fn hue(linear: Vec3) -> f32 {
        let s =
            bevy::color::Srgba::from(bevy::color::LinearRgba::rgb(linear.x, linear.y, linear.z));
        let c = Vec3::new(s.red, s.green, s.blue);
        let (max, min) = (c.max_element(), c.min_element());
        let d = (max - min).max(1e-6);
        let h = if max == c.x {
            ((c.y - c.z) / d).rem_euclid(6.0)
        } else if max == c.y {
            (c.z - c.x) / d + 2.0
        } else {
            (c.x - c.y) / d + 4.0
        };
        h * 60.0
    }

    /// The sky seen so many degrees up, toward a bearing so many degrees round from the sun's,
    /// with the sun so many degrees under the horizon, in the daylight's colours for it.
    fn seen(sun_under: f32, bearing_deg: f32, up_deg: f32) -> Vec3 {
        let (horizon, zenith, glow) = crate::daylight::dusk_colours(-sun_under);
        let (h, b, u) = (
            (-sun_under).to_radians(),
            bearing_deg.to_radians(),
            up_deg.to_radians(),
        );
        let sun = Vec3::new(0.0, libm::sinf(h), -libm::cosf(h));
        let way = Vec3::new(
            libm::sinf(b) * libm::cosf(u),
            libm::sinf(u),
            -libm::cosf(b) * libm::cosf(u),
        );
        dusk_sky(way, Vec3::Y, sun, horizon, zenith, glow)
    }

    #[test]
    fn after_sunset_the_sky_low_toward_the_sun_glows_orange_and_opposite_it_is_cool_under_a_pink_belt()
     {
        for under in [1.0, 2.0, 4.0] {
            let toward = seen(under, 0.0, 3.0);
            let away = seen(under, 180.0, 3.0);
            let belt = seen(under, 180.0, 10.0);
            // Toward the set sun, low: orange, red well over blue, green over blue.
            assert!(
                (10.0..50.0).contains(&hue(toward)) && toward.x > 2.0 * toward.z,
                "{under}: toward the sun {toward} hue {}",
                hue(toward)
            );
            // Warmer than anything opposite it.
            let warmth = |c: Vec3| c.x / c.z;
            assert!(
                warmth(toward) > 2.0 * warmth(away) && warmth(toward) > 1.5 * warmth(belt),
                "{under}: toward {toward} away {away} belt {belt}"
            );
            // Opposite: the earth's shadow blue-grey on the horizon, the Belt of Venus over it
            // pinker than it, its red over its green.
            assert!(away.z > away.x, "{under}: earth's shadow {away}");
            assert!(
                warmth(belt) > warmth(away) && belt.x > belt.y,
                "{under}: belt {belt} over the shadow {away}"
            );
            // And reads as a band of its own: rose (sRGB hue past 300), brighter than the
            // shadow under it and redder than the sky over it. Half taken by the zenith's blue
            // it was blue-violet, hue 251 (lake round 21).
            let over = seen(under, 180.0, 25.0);
            let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
            assert!(hue(belt) > 300.0, "{under}: belt {belt} hue {}", hue(belt));
            assert!(
                luma(belt) > luma(away),
                "{under}: belt {belt} under the shadow {away}"
            );
            assert!(
                warmth(belt) > 1.2 * warmth(over),
                "{under}: belt {belt} under {over}"
            );
            // The arch spans the western sky: 45 degrees off the sun, still warm.
            let aside = seen(under, 45.0, 3.0);
            assert!(aside.x > 1.5 * aside.z, "{under}: 45 degrees off {aside}");
        }
    }

    /// Through the civil twilight, the sun down to 6 degrees under, the sky stays as bright
    /// against the land it lights as at sunset, overhead, across and low opposite the sun, as
    /// a dusk photograph's does: the land is lit by that sky, so the two fade together. Its
    /// blue hour six times darker against the land than its sunset, the settlement film's
    /// sky 3 degrees under went to night over a lit meadow (settlement r31).
    #[test]
    fn through_civil_twilight_the_sky_fades_with_the_land_it_lights() {
        let luma = |c: Vec3| c.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        let against_land = |under: f32, bearing: f32, up: f32| {
            let (sky, land) = crate::daylight::sky_and_land(-under);
            luma(seen(under, bearing, up)) * sky / land
        };
        for (bearing, up) in [(180.0, 70.0), (90.0, 20.0), (180.0, 8.0), (150.0, 35.0)] {
            let sunset = against_land(0.0, bearing, up);
            for under in [1.0, 2.0, 3.0, 4.0, 5.0, 6.0] {
                let now = against_land(under, bearing, up);
                assert!(
                    now > 0.75 * sunset,
                    "{under} under, {bearing} round, {up} up: {now} against {sunset} at sunset"
                );
            }
        }
    }

    /// The numbers of a function's body in a source, in order.
    pub(in crate::sky) fn numbers(source: &str, function: &str) -> Vec<f32> {
        let start = source.find(&format!("fn {function}(")).unwrap();
        let body = &source[start..];
        let end = body.find("\n}").unwrap();
        let mut found = Vec::new();
        for line in body[..end].lines() {
            let code = line.split("//").next().unwrap();
            let mut chars = code.char_indices().peekable();
            while let Some((i, c)) = chars.next() {
                let before = code[..i].chars().last();
                if !c.is_ascii_digit() || before.is_some_and(|b| b.is_alphanumeric() || b == '_') {
                    continue;
                }
                let mut j = i + 1;
                while let Some(&(k, d)) = chars.peek() {
                    let exponent = d == '-' && code[..k].ends_with('e');
                    if d.is_ascii_digit() || d == '.' || d == 'e' || exponent {
                        j = k + d.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let text = &code[i..j];
                if text.contains('.') || text.contains('e') {
                    found.push(text.parse().unwrap());
                }
            }
        }
        found
    }

    #[test]
    fn the_shader_draws_the_rule_tested_here() {
        let shader =
            std::fs::read_to_string(crate::data::assets().join("shaders/dusk.wesl")).unwrap();
        let here = include_str!("dusk.rs");
        for function in ["afterglow", "dusk_sky"] {
            assert_eq!(
                numbers(&shader, function),
                numbers(here, function),
                "{function}"
            );
        }
    }
}
