//! The load's floor decided on a thread of its own, behind the loading screen, and put down
//! once it comes back (portal-freeze j1; the developers: "the game freezes when you walk
//! through the portal"). Deciding where every rock, log, stump and plant goes is 153,000
//! tries by one run of dice, 0.4 to 1.2 s of the frame the world was entered in, the loading
//! screen's ring standing still meanwhile; it reads only the ground and the floor's map
//! (`floor::decide_load`), so, as shipped games generate behind their loading screen and
//! only spawn on the game thread (D623), `plant` makes the floor's kit, starts the deciding
//! and ends, and `put_down` spawns what was decided from the frame it comes back, a slice a
//! frame (`A_FRAME`), the loading screen waiting until all is down (`waited`). With no
//! loading screen up (a layer's test, a scripted run without one) it is decided at once and
//! all put down in the same frame, as before.

use super::floor::{self, Floor};
use super::shade::NearShadows;
use super::{Foliage, Forest, LushCover, Path};
use bevy::platform::collections::HashMap;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use std::sync::Arc;
use std::thread::JoinHandle;
use terra_gen::Seed;
use terra_world::cover::CoverRules;
use terra_world::flora::KeptClear;
use terra_world::flora_kinds::Flora;
use terra_world::grown::Cards;
use terra_world::loading::{Progress, Screen};
use terra_world::planet::Planet;
use terra_world::sky_occlusion::SkyOccluders;

/// What the loading screen says while the floor is decided.
const LAYING: &str = "Laying the forest floor";

/// What the deciding reads, its own to take to a thread: the ground, the places kept clear
/// as the world was entered, the crowns and the way to the sun, and the floor's map.
pub(super) struct Deciding {
    pub planet: Planet,
    pub kept_clear: Option<KeptClear>,
    pub occluders: SkyOccluders,
    pub sun_way: Option<Vec3>,
    pub map: Floor<Path>,
    pub kit: Arc<floor::Kit>,
    pub seed: Seed,
    pub start: Vec3,
    pub east: Vec3,
    pub south: Vec3,
}

/// The floor decided: what to spawn, the map as it left it, and the crowns given back.
type Decided = (floor::Put, Floor<Path>, SkyOccluders);

impl Deciding {
    fn decide(mut self) -> Decided {
        let ground = floor::Ground {
            planet: &self.planet,
            kept_clear: self.kept_clear.as_ref(),
            start: self.start,
            east: self.east,
            south: self.south,
            sun: self.sun_way.map(|way| (&self.occluders, way)),
        };
        let put = floor::decide_load(
            &ground,
            &self.kit,
            &mut self.map,
            &floor::Area::YARD,
            self.seed,
        );
        (put, self.map, self.occluders)
    }
}

/// What the floor is put down with once decided: the forest's materials, what making the
/// kit filed (`shrinking`, `shade`), and what the forest says of itself once it stands.
pub(super) struct Laying {
    pub kit: Arc<floor::Kit>,
    pub cards: Cards,
    pub bark: Handle<Foliage>,
    pub pine_bark: Handle<Foliage>,
    pub spruce_bark: Handle<Foliage>,
    pub atlas: Handle<Foliage>,
    pub thicket: Handle<Foliage>,
    pub seed: Seed,
    pub shrinking: HashMap<(Option<AssetId<Foliage>>, u32), Handle<Foliage>>,
    pub shade: NearShadows,
    pub start: Vec3,
    pub east: Vec3,
    pub south: Vec3,
    pub forest: Forest,
}

/// The floor being decided, or decided and waiting to be put down. The world's: left with
/// it, its thread ending on its own.
#[derive(Resource)]
pub(super) struct Pending {
    thread: Option<JoinHandle<Decided>>,
    decided: Option<Decided>,
    laying: Option<Laying>,
    began: Instant,
    frames: u32,
    /// How many things it puts down a frame: `A_FRAME` behind the loading screen, all of
    /// them with none.
    a_frame: usize,
    /// How many things the floor has, once decided, and how many are still to put down.
    things: usize,
    left: usize,
}

/// The things of the floor put down in a frame of the loading screen: all 36,000 at once
/// were one frame of 243 to 306 ms, the ring standing still (their commands 75 to 116 ms,
/// then the bounds and the detail of each); at this many a frame it is some twenty frames
/// of about 15 ms more each (portal-freeze j3).
const A_FRAME: usize = 2000;

/// Starts deciding the floor: on a thread of its own while the loading screen is up, at
/// once otherwise.
pub(super) fn begin(deciding: Deciding, laying: Laying, behind_the_screen: bool) -> Pending {
    let mut pending = Pending {
        thread: None,
        decided: None,
        laying: Some(laying),
        began: Instant::now(),
        frames: 0,
        a_frame: if behind_the_screen {
            A_FRAME
        } else {
            usize::MAX
        },
        things: 0,
        left: 0,
    };
    if behind_the_screen {
        let thread = std::thread::Builder::new()
            .name("terra-floor".into())
            .spawn(move || deciding.decide())
            .unwrap_or_else(|why| {
                panic!(
                    "the forest floor's thread could not be started ({why}); the machine is \
                     out of threads or memory"
                )
            });
        pending.thread = Some(thread);
    } else {
        pending.decided = Some(deciding.decide());
    }
    pending
}

/// Whether the loading screen is up, so the floor is decided behind it.
pub(super) fn behind_the_screen(screen: Option<&State<Screen>>) -> bool {
    screen.is_some_and(|screen| *screen.get() == Screen::Loading)
}

/// The loading screen waits for the floor.
pub(super) fn waited(pending: Option<ResMut<Pending>>, progress: Option<ResMut<Progress>>) {
    let (Some(mut pending), Some(mut progress)) = (pending, progress) else {
        return;
    };
    pending.frames += 1;
    // Still being decided, it is one thing coming; then what is left to put down.
    let done = pending.things - pending.left;
    progress.report(LAYING, pending.left.max(1), done);
}

/// Puts the floor down once it is decided, and says the forest stands: what `plant` did at
/// its end when it decided the floor itself.
#[allow(clippy::too_many_arguments)]
pub(super) fn put_down(
    mut commands: Commands,
    mut pending: ResMut<Pending>,
    (planet, flora, kept_clear): (Res<Planet>, Res<Flora>, Option<Res<KeptClear>>),
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut plain: ResMut<Assets<StandardMaterial>>,
    mut mossy: ResMut<Assets<floor::Mossy>>,
    mut mossy_scans: ResMut<floor::MossyScans>,
    mut variants: ResMut<floor::Variants>,
    mut foliage: ResMut<Assets<Foliage>>,
) {
    if pending.decided.is_none() {
        if !pending.thread.as_ref().is_some_and(JoinHandle::is_finished) {
            return;
        }
        let Some(thread) = pending.thread.take() else {
            return;
        };
        // A panic deciding it goes on on the main thread, as it did there.
        pending.decided = Some(
            thread
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic)),
        );
        info!(
            "lush: the floor decided in {:.2} s on its own thread, {} frames of the loading \
             screen drawn meanwhile.",
            pending.began.elapsed().as_secs_f32(),
            pending.frames
        );
    }
    let pending = &mut *pending;
    let (Some((put, ..)), Some(laying)) = (&mut pending.decided, &mut pending.laying) else {
        return;
    };
    // All of them, the first time it is put down from.
    pending.things = pending.things.max(put.left());
    let seed = laying.seed;
    let (start, east, south) = (laying.start, laying.east, laying.south);
    // What the laying files as it goes is the lay's for the slice, and kept for the next.
    let mut lay = floor::Lay {
        commands: &mut commands,
        planet: &planet,
        kept_clear: kept_clear.as_deref(),
        start,
        east,
        south,
        assets: &assets,
        meshes: &mut meshes,
        plain: &mut plain,
        mossy: &mut mossy,
        mossy_scans: &mut mossy_scans,
        flora: &flora,
        cards: &laying.cards,
        variants: &mut variants,
        bark: laying.bark.clone(),
        pine_bark: laying.pine_bark.clone(),
        spruce_bark: laying.spruce_bark.clone(),
        atlas: laying.atlas.clone(),
        thicket: laying.thicket.clone(),
        seed,
        foliage: &mut foliage,
        shrinking: std::mem::take(&mut laying.shrinking),
        shade: std::mem::replace(&mut laying.shade, NearShadows::new(start, east, south)),
        // Asked while it was decided; putting down asks nothing of the sun.
        sun: None,
    };
    let down = floor::put_down(&mut lay, &laying.kit, put, pending.a_frame);
    (laying.shrinking, laying.shade) = (lay.shrinking, lay.shade);
    pending.left = put.left();
    if !down {
        return;
    }
    let (Some((_, map, occluders)), Some(laying)) = (pending.decided.take(), pending.laying.take())
    else {
        return;
    };
    commands.insert_resource(occluders);
    commands.insert_resource(CoverRules {
        source: Arc::new(LushCover(Arc::new(map))),
        seed: seed.derive("grass"),
    });
    commands.insert_resource(laying.forest);
    commands.remove_resource::<Pending>();
}
