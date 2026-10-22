//! Building as it happens, to measure (D185): with `--going-up`, on the yard, a stone
//! house's plan (a room and a hipped roof) is laid beside the start, the camera held on it,
//! and from the script's second second a step is built every tenth of a second, as holding
//! the hammer builds on the yard, until it is finished. `cargo xtask bench going-up` times
//! the frames while its sections are laid, merged and put on screen again and again, so a
//! hitch from building shows as it would to a player.

use crate::act::Building;
use bevy::prelude::*;
use terra_bases::plans;
use terra_build::roofer::Style;
use terra_walker::{LocalPlayer, Player};
use terra_world::shell::{Options, ScriptFrame};
use terra_world::start::Start;

/// The room's corner tiles, in the base's own cubes, and where the camera stands and looks,
/// in metres in its own space: across the room from a front corner, as the demo's picture
/// of a stone house going up is.
const ROOM: ([i32; 3], [i32; 3]) = ([4, 0, -16], [13, 0, -9]);
const EYE_M: Vec3 = Vec3::new(13.5, 4.5, -1.5);
const AT_M: Vec3 = Vec3::new(4.5, 1.2, -6.0);
const INSIDE_EYE_M: Vec3 = Vec3::new(6.2, 2.2, -5.0);
const INSIDE_AT_M: Vec3 = Vec3::new(2.6, 1.0, -7.6);
/// The frame building starts on, and how many frames apart its steps are: a tenth of a
/// second at sixty frames a second.
const FIRST_STEP: u32 = 60;
const EVERY: u32 = 6;

pub fn plugin(app: &mut App) {
    app.init_resource::<View>().add_systems(
        Update,
        (build, hold_view)
            .chain()
            .run_if(|options: Res<Options>| options.going_up),
    );
}

/// Where the camera is held, from the start, once the plan is laid.
#[derive(Resource, Default)]
struct View(Option<(Vec3, Vec3)>);

/// How far it has got: the plan laid, and the steps built.
#[derive(Default)]
struct Progress {
    laid: bool,
    steps: u32,
    done: bool,
}

fn build(
    mut progress: Local<Progress>,
    frame: Res<ScriptFrame>,
    start: Option<Res<Start>>,
    mut view: ResMut<View>,
    mut building: Building,
    players: Query<Entity, (With<Player>, With<LocalPlayer>)>,
) {
    if progress.done || frame.0 == 0 {
        return;
    }
    let (Ok(player), Some(start)) = (players.single(), start) else {
        return;
    };
    let (min, max) = (IVec3::from_array(ROOM.0), IVec3::from_array(ROOM.1));
    if !progress.laid {
        progress.laid = true;
        let Some(stone) = building.rules.footing else {
            warn!("Going up: there is no footing kind to build in.");
            progress.done = true;
            return;
        };
        let room = {
            let ground_cm = |column: [i32; 2]| building.ground_cm(column);
            let footings = plans::Footings::of(Some(stone), &ground_cm, &building.rules.footed);
            plans::room(&building.site.base, min, max, stone, stone, footings)
        };
        let laid = building.lay(Some(player), "lay a room", room, Some((min, max)));
        let roof_kind = building
            .site
            .build
            .kits
            .as_ref()
            .map_or(terra_build::Kind::FIRST_ROOF, |kits| kits.roof_over(stone));
        let roof = plans::roof(
            &building.site.base,
            &plans::rectangle(min, max),
            min.y,
            Style::Hip,
            Some(plans::ridge_along_x(min, max)),
            roof_kind,
        );
        let roofed = building.lay(Some(player), "lay a roof", roof, None);
        if let Err(why) = laid.as_ref().and(roofed.as_ref()) {
            warn!("Going up: the plan was refused: {}", why.message());
            progress.done = true;
            return;
        }
        let frame_of = building.site.frame;
        let from_start = |at: Vec3| start.local(frame_of.transform_point(at));
        view.0 = Some((from_start(EYE_M), from_start(AT_M)));
        // Inside, at eye height, looking into the far corner: where its light is seen to
        // change as its roof closes it in (`cargo xtask artifacts --frames`).
        let (eye, at) = (from_start(INSIDE_EYE_M), from_start(INSIDE_AT_M));
        info!(
            "Going up: a stone house's plan laid; inside it: --vantage {:.2},{:.2},{:.2},{:.2},{:.2},{:.2}",
            eye.x, eye.y, eye.z, at.x, at.y, at.z
        );
        return;
    }
    if frame.0 < FIRST_STEP || !(frame.0 - FIRST_STEP).is_multiple_of(EVERY) || !frame.is_changed()
    {
        return;
    }
    let near = (min.as_vec3() + max.as_vec3() + 1.0) * 0.5 * terra_build::CELL_M;
    let Some(work) = building.next_work(Some(player), None, near) else {
        info!("Going up: finished in {} steps.", progress.steps);
        progress.done = true;
        return;
    };
    if let Err(why) = &work.check {
        warn!("Going up: {} was refused: {}", work.what, why.message());
        progress.done = true;
        return;
    }
    if building.act(Some(player), &work.what, &work.ops).is_ok() {
        progress.steps += 1;
    }
}

/// Holds the camera on the house going up.
fn hold_view(view: Res<View>, mut options: ResMut<Options>) {
    if view.is_changed()
        && let Some(view) = view.0
    {
        options.vantage = Some(view);
    }
}
