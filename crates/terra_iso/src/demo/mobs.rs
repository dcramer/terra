//! The biome's roster as mobs (iso-forest-demo; the developer, 11-08: "the monsters are
//! kinda getting stuck"): each foe of its file's `foes` is a body set down off the hero in
//! play, steered to an attack slot about him (steer.rs) and animated by what it does: its
//! idle standing, its walk at its pace while it moves (the clip's speed scaled to the ground
//! it covers, as action games match a stride to a mover's speed), one swing of its attack
//! when it stands in its slot, a stagger after each blow he lands. The still's staged cast
//! stays out of play (wood/fight.rs).
//!
//! Steering runs in PreUpdate, after strike.rs's `settle` (First) takes the last flinch off
//! and before its `show` (Update) lays the next on, so a struck mob flinches on its turn.
//! Before its first blow a mob plays its own graph of idle, walk and attack; after it,
//! strike.rs owns the graph and plays each by name (`Struck::gait`).

use super::steer::{self, Field, Idles, Mob, Rules, STILL, Stalls};
use crate::Args;
use crate::kit::biome::{Biome, Layout};
use crate::nav::Nav;
use crate::play::footprint::{self, Footprint};
use crate::play::strike::Struck;
use crate::play::{Fallen, Ground, Hero};
use crate::ruin::effects::Stage;
use crate::ruin::fight::{Pose, Posed};
use bevy::prelude::*;
use std::time::Duration;

/// A blow holds its mob where it stands this long (s): the stagger.
const STAGGER: f32 = 0.6;
/// Its walk clip's speed is its ground speed over the clip's, kept within these.
const PACE: (f32, f32) = (0.5, 2.5);
/// It swings once it stands within its reach of him (steer.rs `Mob::reach`); under
/// `steer::STILL` it stands, and turns to face him rather than its way.
/// Between one mob's swings (s), and each next mob's a little longer, so they take turns.
const COOLDOWN: (f32, f32) = (1.5, 0.4);
/// How fast it turns to its way (1/s, of what is left).
const TURN: f32 = 8.0;
const BLEND: Duration = Duration::from_millis(200);
/// A stall is warned of once it passes this (s), and again each time it grows by `GROWS`;
/// an awake mob standing idle out of its reach (steer.rs `Idles`) past `IDLE` fails the run.
pub const STALLED: f32 = 1.0;
pub const IDLE: f32 = 1.5;
const GROWS: f32 = 0.5;
/// A pack wakes once the hero comes this near one of it (m): about the camera's half height
/// on the ground, so it is in frame as it starts toward him.
pub const WAKE: f32 = 10.0;

pub fn plugin(app: &mut App) {
    app.add_systems(PostStartup, spawn)
        .add_systems(PreUpdate, steer)
        .add_systems(Update, (rig, gait).chain());
}

/// One of the roster: its place among the horde's mobs and its walk clip's ground speed.
#[derive(Component)]
pub struct Foe {
    pub(super) i: usize,
    walk: f32,
}

/// Its gaits on its own graph (until struck, strike.rs's after), what plays, and its swing.
#[derive(Component)]
pub(super) struct Gait {
    player: Entity,
    nodes: [AnimationNodeIndex; 3],
    /// Its attack clip's length (s).
    swing: f32,
    playing: &'static str,
    /// Whether the graph playing is strike.rs's.
    struck: bool,
    swing_until: f32,
    next_swing: f32,
}

impl Gait {
    /// The swing under way at `now`, if any: when it began and its length (s).
    pub(super) fn swinging(&self, now: f32) -> Option<(f32, f32)> {
        (now < self.swing_until).then_some((self.swing_until - self.swing, self.swing))
    }
}

/// The mobs' steering, one a foe, and their stalls over the run.
#[derive(Resource, Default)]
pub struct Horde {
    pub mobs: Vec<Mob>,
    pub stalls: Stalls,
    pub idles: Idles,
    warned: f32,
    idle_warned: f32,
    /// The nav grid with the footprints blocked, and how many circles it was built with.
    paths: Option<(usize, Nav)>,
    blows: Vec<u32>,
    /// Each mob's pack (its roster line's `pack`), and whether it has woken.
    pub packs: Vec<u32>,
    pub awake: Vec<bool>,
}

/// Wakes every pack one of whose mobs is `near` (the hero within `WAKE` of it, or it struck):
/// all of that pack, for good.
pub(super) fn wake(awake: &mut [bool], packs: &[u32], near: &[bool]) {
    for (i, _) in near.iter().enumerate().filter(|(_, n)| **n) {
        for (j, p) in packs.iter().enumerate() {
            awake[j] |= *p == packs[i];
        }
    }
}

/// The roster set down in play off the stage's hero, each on open ground facing him.
fn spawn(
    mut commands: Commands,
    args: Res<Args>,
    (biome, stage, nav, ground): (
        Option<Res<Biome>>,
        Option<Res<Stage>>,
        Option<Res<Nav>>,
        Option<Res<Ground>>,
    ),
    asset: Res<AssetServer>,
) {
    let (Some(biome), Some(stage)) = (biome, stage) else {
        return;
    };
    if !args.play || args.layout == Some(Layout::Street) || biome.foes.is_empty() {
        return;
    }
    let mut horde = Horde::default();
    for (i, foe) in biome.foes.iter().enumerate() {
        let mut p = stage.knight + Vec2::new(foe.at.0, foe.at.1);
        if let Some(q) = nav.as_deref().and_then(|n| n.nearest_open(p)) {
            p = q;
        }
        let floor = ground.as_deref().map_or(stage.floor, |g| (g.floor)(p));
        let d = stage.knight - p;
        let body = crate::scene::model(
            &mut commands,
            &asset,
            &foe.model,
            p.extend(floor).xzy(),
            libm::atan2f(d.x, d.y),
            1.0,
        );
        commands.entity(body).insert((
            crate::rim::Fighter::default(),
            crate::foliage::Seen,
            Pose {
                gltf: asset.load(foe.model.clone()),
                clip: "idle",
                at: 0.0,
                once: false,
            },
            Foe { i, walk: foe.walk },
        ));
        if let Some(tone) = foe.tone {
            // Its own value and rim over the biome's for its model (ruin/fight.rs `cast`).
            commands
                .entity(body)
                .insert(crate::ruin::fight::Toned(tone));
        }
        // Its body the hero and the others walk round (footprint.rs takes none for it then).
        commands
            .entity(body)
            .insert(footprint::Footprint(vec![(Vec2::ZERO, foe.radius)]));
        horde.mobs.push(Mob::sized(p, foe.radius));
        horde.blows.push(0);
        horde.packs.push(foe.pack);
        horde.awake.push(false);
    }
    info!("{} mobs set down about the hero", horde.mobs.len());
    commands.insert_resource(horde);
}

/// Each tick: every mob not fallen steered to its slot, held by a blow's stagger or its own
/// swing, set on the ground and turned to its way (or to him, standing).
#[allow(clippy::type_complexity)]
fn steer(
    time: Res<Time>,
    (ground, nav, horde): (Option<Res<Ground>>, Option<Res<Nav>>, Option<ResMut<Horde>>),
    hero: Query<&Transform, (With<Hero>, Without<Foe>)>,
    mut bodies: Query<
        (
            &Foe,
            &mut Transform,
            Option<&Struck>,
            Has<Fallen>,
            Option<&Gait>,
        ),
        Without<Hero>,
    >,
    footprints: Query<(&Footprint, &GlobalTransform), (Without<Hero>, Without<Foe>)>,
) {
    let (Some(ground), Some(mut horde), Ok(hero)) = (ground, horde, hero.single()) else {
        return;
    };
    let (now, dt) = (time.elapsed_secs(), time.delta_secs());
    if dt <= 0.0 {
        return;
    }
    let him = hero.translation.xz();
    let circles = footprint::world(&footprints);
    let horde = &mut *horde;
    if let Some(n) = nav.as_deref()
        && horde.paths.as_ref().is_none_or(|p| p.0 != circles.len())
    {
        horde.paths = Some((circles.len(), steer::with_circles(n, &circles)));
    }
    let mut near = vec![false; horde.mobs.len()];
    for (foe, _, struck, fallen, _) in &bodies {
        near[foe.i] = !fallen && (horde.mobs[foe.i].at.distance(him) <= WAKE || struck.is_some());
    }
    wake(&mut horde.awake, &horde.packs, &near);
    let mut active = vec![false; horde.mobs.len()];
    for (foe, _, struck, fallen, gait) in &bodies {
        let m = &mut horde.mobs[foe.i];
        active[foe.i] = !fallen && horde.awake[foe.i];
        let blows = struck.map_or(0, |s| s.blows);
        if blows > horde.blows[foe.i] {
            horde.blows[foe.i] = blows;
            m.hold = m.hold.max(now + STAGGER);
        }
        if let Some(g) = gait {
            m.hold = m.hold.max(g.swing_until);
        }
    }
    let open = |p: Vec2| (ground.open)(p);
    let field = Field {
        nav: horde.paths.as_ref().map(|p| &p.1),
        circles: &circles,
        open: &open,
    };
    steer::step(&mut horde.mobs, &active, him, &field, Rules::ALL, now, dt);
    horde.stalls.watch(&horde.mobs, &active, him, now);
    let (worst, who) = horde.stalls.worst;
    if worst > STALLED.max(horde.warned + GROWS) {
        horde.warned = worst;
        warn!("mob {who} stalled {worst:.1} s");
    }
    horde.idles.watch(&horde.mobs, &active, him, now);
    let (worst, who, off) = horde.idles.worst;
    if worst > STALLED.max(horde.idle_warned + GROWS) {
        horde.idle_warned = worst;
        let m = &horde.mobs[who];
        warn!(
            "mob {who} idle {worst:.1} s {off:.1} m off him (reach {:.1}), slot {:?}",
            m.reach(),
            m.slot
        );
    }
    let turn = 1.0 - libm::expf(-TURN * dt);
    for (foe, mut tr, _, fallen, _) in &mut bodies {
        if fallen {
            continue;
        }
        let m = &horde.mobs[foe.i];
        tr.translation = m.at.extend((ground.floor)(m.at)).xzy();
        // A pack asleep stands as it was set down.
        if !horde.awake[foe.i] {
            continue;
        }
        let way = if m.vel.length() > STILL {
            m.vel
        } else {
            him - m.at
        };
        if way.length_squared() > 1e-6 {
            let face = Quat::from_rotation_y(libm::atan2f(way.x, way.y));
            tr.rotation = tr.rotation.slerp(face, turn);
        }
    }
}

/// Once its model is in and posed: its idle, walk and attack on its own graph, its idle
/// playing.
fn rig(
    mut commands: Commands,
    (gltfs, clips, mut graphs): (
        Res<Assets<Gltf>>,
        Res<Assets<AnimationClip>>,
        ResMut<Assets<AnimationGraph>>,
    ),
    bodies: Query<(Entity, &Pose, &Foe), (With<Posed>, Without<Gait>, Without<Struck>)>,
    children: Query<&Children>,
    mut players: Query<&mut AnimationPlayer>,
) {
    for (body, pose, foe) in &bodies {
        let Some(gltf) = gltfs.get(&pose.gltf) else {
            continue;
        };
        let Some(player) = children
            .iter_descendants(body)
            .find(|e| players.contains(*e))
        else {
            continue;
        };
        let clip = |n: &str| gltf.named_animations.get(n).cloned();
        let (Some(idle), Some(walk), Some(attack)) = (clip("idle"), clip("walk"), clip("attack"))
        else {
            let have: Vec<_> = gltf.named_animations.keys().collect();
            warn!(
                "mob {} {:?} lacks idle, walk or attack (it has {have:?}): it stands, unsteered",
                foe.i,
                pose.gltf.path()
            );
            commands.entity(body).remove::<Foe>();
            continue;
        };
        let swing = clips.get(&attack).map_or(1.0, |c| c.duration());
        let (graph, n) = AnimationGraph::from_clips([idle, walk, attack]);
        let mut p = players.get_mut(player).unwrap();
        p.stop_all();
        let mut tx = AnimationTransitions::new();
        tx.play(&mut p, n[0], Duration::ZERO).repeat();
        commands
            .entity(player)
            .insert((AnimationGraphHandle(graphs.add(graph)), tx));
        commands.entity(body).insert(Gait {
            player,
            nodes: [n[0], n[1], n[2]],
            swing,
            playing: "idle",
            struck: false,
            swing_until: f32::NEG_INFINITY,
            next_swing: 0.0,
        });
    }
}

/// What a mob plays: its attack once while it swings, its walk at its pace while it moves,
/// a swing begun when it stands in its slot off its cooldown, else its idle.
#[allow(clippy::type_complexity)]
pub(super) fn gait(
    time: Res<Time>,
    horde: Option<Res<Horde>>,
    hero: Query<&Transform, With<Hero>>,
    mut bodies: Query<(&Foe, &mut Pose, &mut Gait, Option<&mut Struck>), Without<Fallen>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let (Some(horde), Ok(hero)) = (horde, hero.single()) else {
        return;
    };
    let now = time.elapsed_secs();
    let him = hero.translation.xz();
    for (foe, mut pose, mut g, struck) in &mut bodies {
        let m = &horde.mobs[foe.i];
        let speed = m.vel.length();
        let want = if now < g.swing_until {
            "attack"
        } else if speed > STILL {
            "walk"
        } else if m.at.distance(him) <= m.reach()
            && m.wants(him, now).is_none()
            && now >= g.next_swing
        {
            g.swing_until = now + g.swing;
            g.next_swing = g.swing_until + COOLDOWN.0 + COOLDOWN.1 * foe.i as f32;
            "attack"
        } else {
            "idle"
        };
        let pace = (speed / foe.walk).clamp(PACE.0, PACE.1);
        let Ok((mut p, mut tx)) = players.get_mut(match &struck {
            Some(s) => s.player(),
            None => g.player,
        }) else {
            continue;
        };
        // Its first blow started strike.rs's graph on its own clip: play its gait afresh on it.
        let fresh = struck.is_some() && !g.struck;
        let walk = match struck {
            Some(mut s) => {
                if want != g.playing || fresh {
                    s.gait(want, want != "attack", BLEND, &mut p, &mut tx);
                }
                g.struck = true;
                s.node("walk")
            }
            None => {
                if want != g.playing {
                    let n = g.nodes[["idle", "walk", "attack"]
                        .iter()
                        .position(|w| *w == want)
                        .unwrap()];
                    let a = tx.play(&mut p, n, BLEND);
                    if want != "attack" {
                        a.repeat();
                    }
                }
                Some(g.nodes[1])
            }
        };
        if let Some(a) = walk.and_then(|w| p.animation_mut(w)) {
            a.set_speed(pace);
        }
        if want != g.playing {
            g.playing = want;
            pose.clip = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One of a pack near him wakes all of it and no other pack, and a pack once woken stays
    /// awake when he walks on; with no near mob nothing wakes.
    #[test]
    fn a_pack_wakes_whole_and_alone() {
        let packs = [0, 0, 1, 1, 1, 2];
        let mut awake = [false; 6];
        wake(&mut awake, &packs, &[false; 6]);
        assert_eq!(awake, [false; 6]);
        wake(
            &mut awake,
            &packs,
            &[false, false, false, true, false, false],
        );
        assert_eq!(awake, [false, false, true, true, true, false]);
        wake(
            &mut awake,
            &packs,
            &[true, false, false, false, false, false],
        );
        assert_eq!(awake, [true, true, true, true, true, false]);
    }
}
