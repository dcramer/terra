//! A fire light's shadows eased in and out as they pass from one light to another, split from
//! `light` for its size (D396). Bevy's point lights cast their shadows whole or not at all
//! (bevy_light 0.20's `PointLight` has no strength), so a light part-way is lit as Godot's
//! `Light3D.shadow_opacity` lights one: two lights at one place, one shadowed with the share
//! `s` of its lumens and one unshadowed (a twin) with the rest, `L * (s * shadow + 1 - s)`, a
//! shadow of strength `s`. The light that is to have the shadows waits for the one losing
//! them to let go, so no more lights cast them at once than `shadowed` allows and the cost
//! holds. Cut at once, the shadow hopped from torch to torch as the camera walked a crypt:
//! the walls behind each torch's bracket and the fog round it stepped (light round 4).

use super::{Lit, Shown};
use bevy::{light::VolumetricLight, prelude::*};

/// The fire lights and their twins, as `light` sets them.
pub(crate) type Lights<'w, 's> = Query<
    'w,
    's,
    (
        &'static mut PointLight,
        &'static mut Transform,
        &'static mut Visibility,
        Has<VolumetricLight>,
    ),
>;

/// How long a light's shadows take to come in, and to go, seconds.
const FADE_S: f32 = 0.5;
/// How far a light entity's light may move from one frame to the next and still be the same
/// light (as `choose` keeps a light on its entity); farther, its shadows start again from none.
const SAME_M: f32 = 4.0;
/// How many lights may be part-way at once: as many as cast shadows in films (`shadowed`).
pub(super) const TWINS: usize = 4;

/// Each light entity's shadow strength, 0 to 1, whether its light wants shadows (`choose`),
/// where its light was, and the twins that light the unshadowed part of those part-way.
#[derive(Default)]
pub(crate) struct ShadowFade {
    pub strength: Vec<f32>,
    pub wanted: Vec<bool>,
    at: Vec<Vec3>,
    pub twins: Vec<Entity>,
}

impl ShadowFade {
    /// One frame of `dt` seconds: a light that no longer wants shadows lets go of them over
    /// `FADE_S`; one that wants them takes them over `FADE_S` once fewer than `shadows`
    /// lights hold any, the nearest the eye first.
    pub fn ease(&mut self, shown: &[Option<Shown>], eye: Option<Vec3>, shadows: usize, dt: f32) {
        let n = shown.len();
        self.strength.resize(n, 0.0);
        self.wanted.resize(n, false);
        self.at.resize(n, Vec3::ZERO);
        let step = dt / FADE_S;
        for (i, shown) in shown.iter().enumerate() {
            let Some(shown) = shown else {
                self.strength[i] = 0.0;
                self.wanted[i] = false;
                continue;
            };
            if self.at[i].distance(shown.lit.at) > SAME_M {
                self.strength[i] = 0.0;
            }
            self.at[i] = shown.lit.at;
            self.wanted[i] = shown.shadows;
            let s = &mut self.strength[i];
            *s = match (shown.shadows, *s > 0.0) {
                (true, true) => (*s + step).min(1.0),
                (true, false) => 0.0,
                (false, _) => (*s - step).max(0.0),
            };
        }
        let mut holding = self.strength.iter().filter(|s| **s > 0.0).count();
        let far = |i: &usize| eye.map_or(0.0, |eye| eye.distance(self.at[*i]));
        let mut waiting: Vec<usize> = (0..n)
            .filter(|&i| self.wanted[i] && self.strength[i] <= 0.0)
            .collect();
        waiting.sort_by(|a, b| far(a).total_cmp(&far(b)));
        for i in waiting {
            if holding < shadows {
                holding += 1;
                self.strength[i] = step.clamp(f32::MIN_POSITIVE, 1.0);
            }
        }
    }

    /// The twins, spawned the first frame and kept: hidden, they cost nothing.
    pub fn spawn_twins(&mut self, commands: &mut Commands) {
        while self.twins.len() < TWINS {
            let twin = commands.spawn((
                Name::new("Fire: light's unshadowed twin"),
                PointLight {
                    intensity: 0.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                Transform::IDENTITY,
                Visibility::Hidden,
            ));
            self.twins.push(twin.id());
        }
    }

    /// Each light part-way's twin, at its place with the lumens its shadowed part leaves.
    pub fn show_twins(&self, parts: &[(Lit, f32)], lights: &mut Lights) {
        for (k, &entity) in self.twins.iter().enumerate() {
            let Ok((mut light, mut transform, mut visibility, _)) = lights.get_mut(entity) else {
                continue;
            };
            match parts.get(k) {
                Some((lit, lumens)) => {
                    light.intensity = *lumens;
                    light.range = lit.range;
                    light.color = lit.color;
                    light.radius = lit.radius;
                    transform.translation = lit.at;
                    *visibility = Visibility::Inherited;
                }
                None if *visibility != Visibility::Hidden => {
                    light.intensity = 0.0;
                    *visibility = Visibility::Hidden;
                }
                None => {}
            }
        }
    }
}

/// A light's lumens split by its shadow strength: its own entity's (shadowed if the strength
/// is over 0, whole and unshadowed if not) and its twin's.
pub(super) fn parts(lumens: f32, strength: f32) -> (f32, f32) {
    if strength <= 0.0 {
        (lumens, 0.0)
    } else {
        (lumens * strength, lumens * (1.0 - strength))
    }
}

/// A kept fire's light as it shines unshadowed: where it hangs without its sway, how far its
/// flicker has it from its mean now (its lumens over its mean's), and which kept fire's it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Still {
    pub at: Vec3,
    pub shines: f32,
    pub fire: usize,
}

/// A light as its entity shows it, its fade and shadow share in its lumens, and its twin's,
/// the unshadowed rest of one part-way (`parts`).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Shone {
    pub own: Lit,
    pub twin: Option<Lit>,
}

/// So much of a light's flicker and sway, from none (its mean, where it hangs) to all.
fn flickering(lit: &Lit, flickers: f32, lumens: f32) -> Lit {
    let mut out = lit.clone();
    out.lumens = lumens;
    if let Some(still) = &lit.still {
        let mean = lumens / still.shines;
        out.lumens = mean + (lumens - mean) * flickers;
        out.at = still.at + (lit.at - still.at) * flickers;
    }
    out
}

/// What each light shows with its shadows so strong (`ShadowFade::strength`). A kept fire's
/// light flickers and sways only as far as its fire's shadows are cast, its top light (never
/// shadowed) as far as its key's: the unshadowed rest shines steady at its mean, where it
/// hangs. Unshadowed, a torch's light passes through the rock round it, and its flicker
/// pulsed the walls of the corridors beside it: the crypt tour's whole-picture brightness
/// jumped over 5% a frame 32 times, 13 with it steady (dungeon light round 5); a wall torch lights only the dungeon's walls, as Bevy 0.20 masks no light
/// by room. A blaze's light flickers whole.
pub(super) fn shone(shown: &[Option<Shown>], strength: &[f32]) -> Vec<Option<Shone>> {
    let s = |i: usize| strength.get(i).copied().unwrap_or(0.0);
    let keys: Vec<(usize, f32)> = shown
        .iter()
        .enumerate()
        .filter_map(|(i, shown)| {
            let lit = &shown.as_ref()?.lit;
            lit.casts.then_some((lit.still?.fire, s(i)))
        })
        .collect();
    let key = |fire: usize| keys.iter().find(|k| k.0 == fire).map_or(0.0, |k| k.1);
    shown
        .iter()
        .enumerate()
        .map(|(i, shown)| {
            let Shown { lit, fade, .. } = shown.as_ref()?;
            let lumens = lit.lumens * fade;
            if !lit.casts {
                let flickers = lit.still.map_or(0.0, |still| key(still.fire));
                return Some(Shone {
                    own: flickering(lit, flickers, lumens),
                    twin: None,
                });
            }
            let (own, twin) = parts(lumens, s(i));
            Some(if s(i) > 0.0 {
                Shone {
                    own: flickering(lit, 1.0, own),
                    twin: (twin > 0.0).then(|| flickering(lit, 0.0, twin)),
                }
            } else {
                Shone {
                    own: flickering(lit, 0.0, own),
                    twin: None,
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn torch(x: f32, shadows: bool) -> Option<Shown> {
        let lit = Lit {
            at: Vec3::new(x, 1.5, 0.0),
            lumens: 1.0e6,
            color: Color::WHITE,
            range: 20.0,
            radius: 0.1,
            glow: 1.0,
            kept: true,
            night_stops: 2.5,
            dusk_gain: 1.0,
            casts: true,
            still: None,
        };
        Some(Shown {
            lit,
            fade: 1.0,
            shadows,
            scatters: false,
        })
    }

    /// Frames at 60 a second until `until` holds, at most `most`, each frame's strengths
    /// kept in `seen`; how many it took.
    fn run(
        fade: &mut ShadowFade,
        shown: &[Option<Shown>],
        until: impl Fn(&ShadowFade) -> bool,
        seen: &mut Vec<Vec<f32>>,
    ) -> usize {
        let most = 600;
        for frame in 1..=most {
            fade.ease(shown, Some(Vec3::ZERO), 1, 1.0 / 60.0);
            seen.push(fade.strength.clone());
            if until(fade) {
                return frame;
            }
        }
        most
    }

    #[test]
    fn a_shadow_passes_from_one_torch_to_the_next_eased_and_never_cast_by_two() {
        let mut fade = ShadowFade::default();
        let mut seen = Vec::new();
        let first = [torch(4.0, true), torch(12.0, false)];
        run(&mut fade, &first, |f| f.strength[0] >= 1.0, &mut seen);
        let next = [torch(4.0, false), torch(12.0, true)];
        let frames = run(&mut fade, &next, |f| f.strength[1] >= 1.0, &mut seen);
        // A step a frame no bigger than a tenth of the shadow, cast by one light at a time,
        // and the hand-over done in well under two seconds.
        for pair in seen.windows(2) {
            for (a, b) in pair[0].iter().zip(&pair[1]) {
                assert!((a - b).abs() <= 0.1, "a shadow stepped from {a} to {b}");
            }
        }
        assert!(
            seen.iter()
                .all(|s| s.iter().filter(|s| **s > 0.0).count() <= 1)
        );
        assert!(
            frames > 6 && frames < 120,
            "the hand-over took {frames} frames"
        );
    }

    #[test]
    fn a_torch_turned_back_to_mid_hand_over_takes_its_shadow_back_without_a_blink() {
        let mut fade = ShadowFade::default();
        let mut seen = Vec::new();
        let first = [torch(4.0, true), torch(12.0, false)];
        run(&mut fade, &first, |f| f.strength[0] >= 1.0, &mut seen);
        let next = [torch(4.0, false), torch(12.0, true)];
        run(&mut fade, &next, |f| f.strength[0] < 0.5, &mut seen);
        let low = fade.strength[0];
        seen.clear();
        run(&mut fade, &first, |f| f.strength[0] >= 1.0, &mut seen);
        assert!(seen.iter().all(|s| s[0] >= low && s[1] == 0.0));
    }

    #[test]
    fn a_light_part_way_and_its_twin_light_as_much_as_the_light_whole() {
        for strength in [0.0, 0.01, 0.3, 0.5, 0.99, 1.0] {
            let (shadowed, twin) = parts(1000.0, strength);
            assert!((shadowed + twin - 1000.0).abs() < 1e-3);
        }
        assert_eq!(parts(1000.0, 0.0), (1000.0, 0.0));
    }

    /// Three torches down a corridor along x, the eye by the first, its key's shadows so
    /// strong: over ten seconds, each torch's light summed (key, top and twin) and how far its
    /// lumens and its lights' places stray from their first.
    fn strays(strength: f32) -> Vec<(f32, f32)> {
        use crate::fire::{Blaze, Blazes};
        let fire = |i: usize| Blaze {
            at: Vec3::new(i as f32 * 3.0, 2001.5, 0.0),
            tall: 0.45,
            heat: 0.6,
            seed: 0.1 + 0.3 * i as f32,
            wide: 0.12,
        };
        let blazes = Blazes {
            kept: (0..3).map(fire).collect(),
            lights: vec![super::super::TORCH_LIGHT; 3],
            ..default()
        };
        let eye = Some(Vec3::new(-1.0, 2001.6, 0.0));
        let off = super::super::Was {
            at: Vec3::ZERO,
            on: false,
            near: false,
        };
        let was = vec![off; super::super::LIGHTS];
        let mut first: Vec<Option<(f32, Vec<Vec3>)>> = vec![None; 3];
        let mut out = vec![(0.0f32, 0.0f32); 3];
        for step in 0..2000 {
            let t = step as f32 * 0.005;
            let lit = super::super::lit(&blazes, t, crate::fire::BreathTier::Cards);
            let shown = super::super::choose(lit, eye, &was, 1);
            let strengths: Vec<f32> = shown
                .iter()
                .map(|s| {
                    s.as_ref()
                        .map_or(0.0, |s| if s.shadows { strength } else { 0.0 })
                })
                .collect();
            let mut torches = vec![(0.0f32, Vec::new()); 3];
            for shone in shone(&shown, &strengths).into_iter().flatten() {
                for light in std::iter::once(shone.own).chain(shone.twin) {
                    let torch = &mut torches[(light.at.x / 3.0).round() as usize];
                    torch.0 += light.lumens;
                    torch.1.push(light.at);
                }
            }
            for (i, (lumens, at)) in torches.into_iter().enumerate() {
                let (was_lumens, was_at) = first[i].get_or_insert_with(|| (lumens, at.clone()));
                out[i].0 = out[i].0.max((lumens / *was_lumens - 1.0).abs());
                for (a, b) in at.iter().zip(was_at.iter()) {
                    out[i].1 = out[i].1.max(a.distance(*b));
                }
            }
        }
        out
    }

    #[test]
    fn a_torch_flickers_its_light_only_as_far_as_its_shadows_are_cast() {
        // The shadowed torch flickers and sways; the others, whose light passes through the
        // rock unshadowed, shine steady where they hang.
        let whole = strays(1.0);
        assert!(whole[0].0 > 0.05 && whole[0].1 > 1e-3, "{whole:?}");
        for steady in &whole[1..] {
            assert!(steady.0 < 1e-5 && steady.1 < 1e-5, "{whole:?}");
        }
        // Part-way through a hand-over, it flickers part-way, its twin steady.
        let half = strays(0.5);
        let share = half[0].0 / whole[0].0;
        assert!((0.3..0.7).contains(&share), "{share}: {half:?}");
        // Its shadows gone, it shines as steady as the others.
        let none = strays(0.0);
        assert!(none.iter().all(|s| s.0 < 1e-5 && s.1 < 1e-5), "{none:?}");
    }
}
