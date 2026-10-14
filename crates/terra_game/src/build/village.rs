//! A village to measure (docs/building-plan.md, checking it; D185): with `--village`, on
//! the yard, fifty buildings north of the start, built, of every kind, roof and plan the
//! kit sheet has (logs, stone, hewn timber, and stone with logs; gable, hip and lean-to;
//! a rectangle, an L, a T and a U), turned every way, ten to a row. `cargo xtask bench
//! village` looks at it from its corner, and `village-bare` at the yard from the same place,
//! so what fifty buildings cost is the difference.
//!
//! Laying it also times operations on each house, as a player's are applied and checked,
//! and says how long they took.

use super::{Build, BuildLook, Built, Site, frame_at, lifted, read_kits};
use crate::planet::Planet;
use crate::shell::Options;
use crate::start::FLOOR_CLEARANCE_M;
use bevy::prelude::*;
use std::sync::Arc;
use std::time::Instant;
use terra_build::kit::{Pattern, Profile};
use terra_build::ops::{Context, Op};
use terra_build::{Base, Rules};

/// How many buildings to a row, and how far apart, in metres.
const PER_ROW: usize = 10;
const APART_M: f32 = 15.0;
/// How far north of the start the first row is, and west the first in a row.
const NORTH_M: f32 = 20.0;
const WEST_M: f32 = 68.0;

/// Lays the village once the base is there.
pub(super) fn lay(
    mut done: Local<bool>,
    options: Res<Options>,
    site: Option<Res<Site>>,
    look: Option<Res<BuildLook>>,
    planet: Res<Planet>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    rules: Option<Res<super::act::BuildRules>>,
) {
    if *done || !options.village {
        return;
    }
    let (Some(site), Some(look)) = (site, look) else {
        return;
    };
    *done = true;
    let Some(kits) = read_kits().map(Arc::new) else {
        return;
    };
    let find = |pattern: Pattern, profile: Profile| {
        kits.0
            .iter()
            .find(|kit| {
                kit.pattern == pattern
                    && (pattern == Pattern::Masonry || kit.timber.profile == profile)
            })
            .map(|kit| kit.kind())
    };
    let (Some(logs), Some(stone), Some(hewn)) = (
        find(Pattern::Logs, Profile::Round),
        kits.footing().map(|kit| kit.kind()),
        find(Pattern::Logs, Profile::Hewn),
    ) else {
        warn!("The village needs a log, a stone and a hewn kit.");
        return;
    };
    let rules = rules.map(|rules| rules.rules.clone()).unwrap_or_default();
    let mut took: Vec<f32> = Vec::new();
    for (index, (name, ops)) in terra_build::bench_village(logs, stone, hewn)
        .into_iter()
        .enumerate()
    {
        let (row, column) = (index / PER_ROW, index % PER_ROW);
        let east = -WEST_M + APART_M * column as f32;
        let north = -NORTH_M - APART_M * row as f32;
        let point = site.frame.transform_point(Vec3::new(east, 0.0, north));
        let ground = point - point.normalize() * planet.distance(point);
        let frame = frame_at(ground + ground.normalize() * FLOOR_CLEARANCE_M, &site.frame);
        let mut base = Base::default();
        for &op in &ops {
            base.apply(op);
        }
        // Turned every way, a quarter at a time, about its middle.
        let cubes: Vec<IVec3> = base.cubes().map(|(at, _)| at).collect();
        let low = cubes.iter().copied().reduce(IVec3::min).unwrap_or_default();
        let high = cubes.iter().copied().reduce(IVec3::max).unwrap_or_default();
        let middle = ((low + high + IVec3::ONE).as_vec3() * 0.5 * terra_build::CELL_M).with_y(0.0);
        let turn = Transform::from_rotation(Quat::from_rotation_y(
            std::f32::consts::FRAC_PI_2 * (index % 4) as f32,
        ));
        let frame = frame
            * Transform::from_translation(middle)
            * turn
            * Transform::from_translation(-middle);
        let frame = lifted(frame, &base, &planet);
        let label = format!("Village: {name}");
        let mut build = Build::new(&mut commands, &look, frame, &label);
        build.kits = Some(kits.clone());
        build.replace_with(&ops, &mut commands, &planet, &mut meshes);
        took.extend(time_operations(&build.base, &rules));
        commands.spawn((Name::new(label), Built(build)));
    }
    took.sort_by(f32::total_cmp);
    let at = |share: f32| took[((took.len() - 1) as f32 * share) as usize];
    if !took.is_empty() {
        info!(
            "Village: {} operations applied and checked in a median {:.3} ms, the slowest {:.3} ms.",
            took.len(),
            at(0.5),
            at(1.0)
        );
    }
}

/// How long operations on a house take to apply and check, as a player's do: a window
/// cut into a wall, a step of it taken out, and both undone, each in milliseconds.
fn time_operations(base: &Base, rules: &Rules) -> Vec<f32> {
    let cx = Context::free(rules);
    let mut base = base.clone();
    // A wall's cube a metre up, along its first row.
    let Some(cube) = base
        .cubes()
        .map(|(at, _)| at)
        .find(|&at| at.y == 2 && base.in_wall(at))
    else {
        return Vec::new();
    };
    let cubes = vec![cube.to_array()];
    let mut took = Vec::new();
    let mut undo = Vec::new();
    for op in [
        Op::Carve {
            cubes: cubes.clone(),
        },
        Op::Empty { cubes },
    ] {
        let started = Instant::now();
        let done = base.act(&op, &cx);
        took.push(started.elapsed().as_secs_f32() * 1000.0);
        if let Ok(done) = done {
            undo.push(done.undo);
        }
    }
    for ops in undo.into_iter().rev() {
        let started = Instant::now();
        let _ = base.act_all(&ops, &cx);
        took.push(started.elapsed().as_secs_f32() * 1000.0);
    }
    took
}
