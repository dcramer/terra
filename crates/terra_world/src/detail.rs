//! How far detail reaches, as one scale over every distance at which something changes its
//! level of detail or comes into view: each object's own distances, as authored, times
//! [`DetailScale`]. The Witcher 3 does the same (`FoliageDistanceScale` over each asset's own
//! distances); it doubles as a quality setting.
//!
//! It also makes popping checkable without a person (D69, D225). A scripted run given
//! `--detail-sweep` holds the camera, the clock and the wind still and sweeps the scale over
//! frames, so every crown, trunk, shadow and ring of grass crosses its switch in front of a
//! still camera, and anything that changes between two frames is a level of detail changing
//! (`tools/sandbox/lodcheck.ts`).

use crate::shell::{Options, ScriptFrame};
use bevy::{camera::visibility::VisibilityRange, prelude::*};

/// Every distance at which detail changes is its own, as authored, times this.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct DetailScale(pub f32);

impl Default for DetailScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// An object's distances as authored, before the scale.
#[derive(Component, Clone)]
struct Authored(VisibilityRange);

pub fn plugin(app: &mut App) {
    app.init_resource::<DetailScale>().add_systems(
        PostUpdate,
        (sweep, remember, rescale)
            .chain()
            .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
    );
}

/// Keeps what each newly ranged object was given, and scales it at once.
fn remember(
    mut commands: Commands,
    scale: Res<DetailScale>,
    mut new: Query<(Entity, &mut VisibilityRange), Without<Authored>>,
) {
    for (entity, mut range) in &mut new {
        commands.entity(entity).insert(Authored(range.clone()));
        if scale.0 != 1.0 {
            *range = scaled(&range, scale.0);
        }
    }
}

/// When the scale changes, every object\'s distances follow it.
fn rescale(scale: Res<DetailScale>, mut ranged: Query<(&Authored, &mut VisibilityRange)>) {
    if !scale.is_changed() {
        return;
    }
    for (authored, mut range) in &mut ranged {
        *range = scaled(&authored.0, scale.0);
    }
}

fn scaled(range: &VisibilityRange, by: f32) -> VisibilityRange {
    VisibilityRange {
        start_margin: range.start_margin.start * by..range.start_margin.end * by,
        end_margin: range.end_margin.start * by..range.end_margin.end * by,
        use_aabb: range.use_aabb,
    }
}

/// Over a scripted run\'s sweep, sets the scale for its frame, the clock held still so
/// nothing but detail changes between frames.
fn sweep(
    options: Res<Options>,
    frame: Res<ScriptFrame>,
    mut scale: ResMut<DetailScale>,
    mut time: ResMut<Time<Virtual>>,
) {
    let Some(sweep) = options.detail_sweep else {
        return;
    };
    if let Some(now) = sweep.at(frame.0) {
        if !time.is_paused() {
            time.pause();
        }
        if scale.0 != now {
            scale.0 = now;
        }
    }
}

/// A sweep of the detail scale over frames of a scripted run: from one scale to another,
/// between two frames, counted as `--press` counts them; logarithmic, so each frame moves
/// the switches by the same share of their distance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sweep {
    pub from: f32,
    pub to: f32,
    pub first: u32,
    pub last: u32,
}

impl Sweep {
    pub fn parse(text: &str) -> Result<Self, String> {
        let problem =
            || format!("--detail-sweep wants scales and frames, like 0.5-2@300-600, not `{text}`");
        let (scales, frames) = text.split_once('@').ok_or_else(problem)?;
        let pair = |part: &str| -> Result<(String, String), String> {
            part.split_once('-')
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .ok_or_else(problem)
        };
        let (from, to) = pair(scales)?;
        let (first, last) = pair(frames)?;
        let sweep = Self {
            from: from.trim().parse().map_err(|_| problem())?,
            to: to.trim().parse().map_err(|_| problem())?,
            first: first.trim().parse().map_err(|_| problem())?,
            last: last.trim().parse().map_err(|_| problem())?,
        };
        if sweep.from <= 0.0 || sweep.to <= 0.0 || sweep.last <= sweep.first {
            return Err(problem());
        }
        Ok(sweep)
    }

    /// The scale on a frame: none before the sweep, the end\'s after it.
    pub fn at(&self, frame: u32) -> Option<f32> {
        if frame < self.first {
            return None;
        }
        let t = ((frame - self.first) as f32 / (self.last - self.first) as f32).min(1.0);
        Some(libm::expf(
            libm::logf(self.from) * (1.0 - t) + libm::logf(self.to) * t,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sweep_runs_from_one_scale_to_the_other_evenly_by_ratio() {
        let sweep = Sweep::parse("0.5-2@100-200").unwrap();
        assert_eq!(sweep.at(99), None);
        assert!((sweep.at(100).unwrap() - 0.5).abs() < 1e-5);
        assert!(
            (sweep.at(150).unwrap() - 1.0).abs() < 1e-4,
            "halfway by ratio is 1"
        );
        assert!((sweep.at(200).unwrap() - 2.0).abs() < 1e-4);
        assert!((sweep.at(900).unwrap() - 2.0).abs() < 1e-4);
        assert!(Sweep::parse("2@1-2").is_err() && Sweep::parse("1-2@5-5").is_err());
    }

    #[test]
    fn distances_follow_the_scale_from_what_was_authored() {
        let mut app = App::new();
        app.insert_resource(DetailScale(2.0))
            .add_systems(Update, (remember, rescale).chain());
        let thing = app
            .world_mut()
            .spawn(VisibilityRange {
                start_margin: 10.0..12.0,
                end_margin: 40.0..50.0,
                use_aabb: false,
            })
            .id();
        app.update();
        let range = app.world().get::<VisibilityRange>(thing).unwrap().clone();
        assert_eq!(range.end_margin, 80.0..100.0);
        app.world_mut().resource_mut::<DetailScale>().0 = 0.5;
        app.update();
        let range = app.world().get::<VisibilityRange>(thing).unwrap().clone();
        assert_eq!(
            range.start_margin,
            5.0..6.0,
            "from what was authored, not compounded"
        );
    }
}
