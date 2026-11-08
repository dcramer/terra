//! The hero's blows landing in play, sold as the staged hit sells its one (ruin/effects/
//! hit.rs): each struck body lit white then red (the hit flash, on its own copies of its
//! materials: a glTF's are shared by every copy of the model, so the staged ogre's would
//! light every ogre), thrown back a few degrees and recovering (the flinch), its hit clip
//! played and then its own again; the `BLOWS`th blow plays its death, held. The blow's
//! sparks, flash, lights and dust are the biome's presets (kit/effects.rs, `Cues::blow`),
//! moved to where the blade meets the body (`Cues::contact`). Nothing here runs without
//! `--play`, so the staged scenes are as before.

use super::knock::{Knock, Weight};
use super::{Fallen, Hero};
use crate::kit::effects::{Contact, Cues, Landed};
use crate::ruin::effects::hit::{FLINCH, flash, flinch};
use crate::ruin::effects::slash::Blade;
use crate::ruin::fight::Pose;
use bevy::prelude::*;
use std::time::Duration;

/// The blows a body takes before it falls: two hits, then its death.
pub const BLOWS: u32 = 3;
/// How far in from a body's middle the blade bites it (m), and the bite's height when the
/// blade's path is unknown (no sword baked).
const RADIUS: f32 = 0.4;
const HEIGHT: f32 = 1.3;
/// Into a hit or death, and back to its own clip after the hit.
const INTO: Duration = Duration::from_millis(60);
const BACK: Duration = Duration::from_millis(250);

/// A blow landing on a body this frame: where the hero stood, and the attack clip's time
/// of the blow (control in play.rs inserts it).
#[derive(Component)]
pub struct Blow {
    pub from: Vec2,
    pub t: f32,
    /// 0 a light blow, 1 a heavy (its feel, play/feel.rs).
    pub weight: f32,
}

/// A body the hero has struck: the blows it took, the last one's time, the flinch's tilt
/// laid on its facing this frame, the way it was struck, its own materials and its clips on
/// its own graph.
#[derive(Component)]
pub struct Struck {
    pub blows: u32,
    at: f32,
    tilt: Quat,
    away: Vec3,
    skin: Vec<Handle<StandardMaterial>>,
    player: Entity,
    own: Option<AnimationNodeIndex>,
    hit: Option<AnimationNodeIndex>,
    death: Option<AnimationNodeIndex>,
    /// Its gaits on the same graph, by name (`GAITS`): a steered mob still changes gait once
    /// struck (demo/mobs.rs, `Struck::gait`).
    gaits: Vec<(&'static str, AnimationNodeIndex)>,
    /// A heavy or killing blow's slide and turn (knock.rs).
    pub(super) knock: Option<Knock>,
}

/// The clips a struck body keeps beside its hit and death, so it can still stand, walk and
/// swing on its own graph.
pub const GAITS: [&str; 3] = ["idle", "walk", "attack"];

impl Struck {
    /// Whether a blow's slide moves it at `now`: a steered mob is left where it slides
    /// (demo/mobs.rs).
    pub fn sliding(&self, now: f32) -> bool {
        self.knock.is_some_and(|k| k.sliding(now))
    }

    /// The player its clips are on.
    pub fn player(&self) -> Entity {
        self.player
    }

    /// Its gait `name`'s node on its graph, if it has the clip.
    pub fn node(&self, name: &str) -> Option<AnimationNodeIndex> {
        self.gaits.iter().find(|g| g.0 == name).map(|g| g.1)
    }

    /// Plays its gait `name` as its own clip from now on (what a hit gives way to, `recover`),
    /// looped or once, blended in over `blend`; unless it is fallen or mid-hit, when the
    /// reaction plays on and only its own clip changes. Whether it has the clip.
    pub fn gait(
        &mut self,
        name: &str,
        looped: bool,
        blend: Duration,
        p: &mut AnimationPlayer,
        tx: &mut AnimationTransitions,
    ) -> bool {
        let Some(n) = self.node(name) else {
            return false;
        };
        self.own = Some(n);
        let reacting = tx
            .get_main_animation()
            .is_some_and(|m| Some(m) == self.death || Some(m) == self.hit);
        if !reacting {
            let a = tx.play(p, n, blend);
            if looped {
                a.repeat();
            }
        }
        true
    }
}

/// What a body does on its `n`th blow (from 1).
#[derive(Debug, PartialEq)]
pub enum React {
    Hit,
    Death,
    /// No clip for it: the flash and flinch alone.
    Flinch,
}

pub fn react(n: u32, hit: bool, death: bool) -> React {
    if n >= BLOWS && death {
        React::Death
    } else if hit {
        React::Hit
    } else {
        React::Flinch
    }
}

/// Where a blow from `hero` bites the body standing at `foe` (ground points): on its near
/// side, `RADIUS` in from its middle, at the height where the blade's edge (`edge`, its
/// inner and outer points in the world) passes nearest the body's axis.
pub fn bite(hero: Vec2, foe: Vec2, edge: Option<(Vec3, Vec3)>, floor: f32) -> Vec3 {
    let d = foe - hero;
    let toward = d.normalize_or(Vec2::X);
    let at = foe - toward * RADIUS.min(d.length() / 2.0);
    let y = edge.map_or(floor + HEIGHT, |(a, b)| {
        let ab = (b - a).xz();
        let k = ((foe - a.xz()).dot(ab) / ab.length_squared().max(1e-6)).clamp(0.0, 1.0);
        (a.y + (b.y - a.y) * k).clamp(floor + 0.4, floor + 2.2)
    });
    Vec3::new(at.x, y, at.y)
}

pub fn plugin(app: &mut App) {
    app.add_systems(First, settle).add_systems(
        Update,
        (land, super::knock::slide, recover, show)
            .chain()
            .after(super::control),
    );
}

fn player_of(
    body: Entity,
    children: &Query<&Children>,
    players: &Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) -> Option<Entity> {
    children
        .iter_descendants(body)
        .find(|e| players.contains(*e))
}

/// Each blow: the body's first takes its materials and clips over; then its reaction, and
/// the blow's effects cued at the bite.
#[allow(clippy::too_many_arguments)]
fn land(
    mut commands: Commands,
    time: Res<Time>,
    (gltfs, mut graphs, mut materials): (
        Res<Assets<Gltf>>,
        ResMut<Assets<AnimationGraph>>,
        ResMut<Assets<StandardMaterial>>,
    ),
    mut blown: Query<(
        Entity,
        &Blow,
        &Pose,
        &Transform,
        Option<&Weight>,
        Option<&mut Struck>,
    )>,
    children: Query<&Children>,
    skins: Query<&MeshMaterial3d<StandardMaterial>>,
    mut players: Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
    (blade, hero, cues): (
        Option<Res<Blade>>,
        Query<(Entity, &GlobalTransform), With<Hero>>,
        Option<ResMut<Cues>>,
    ),
) {
    let now = time.elapsed_secs();
    let mut contact = None;
    let mut landed = Landed {
        at: now,
        weight: 0.0,
        parried: false,
        bodies: hero.iter().map(|(e, _)| e).collect(),
    };
    for (body, blow, pose, tr, weight, struck) in &mut blown {
        landed.bodies.push(body);
        landed.weight = landed.weight.max(blow.weight);
        // A struck body stops walking in (walk.rs).
        commands
            .entity(body)
            .remove::<(Blow, crate::ruin::walk::WalkIn)>();
        let foe = tr.translation.xz();
        let away = (foe - blow.from).normalize_or(Vec2::X);
        let edge = blade
            .as_ref()
            .and_then(|b| b.edge(blow.t))
            .zip(hero.single().ok())
            .map(|((i, o), (_, g))| (g.transform_point(i), g.transform_point(o)));
        contact = Some(Contact {
            point: bite(blow.from, foe, edge, tr.translation.y),
            floor: tr.translation.y,
            toward: away,
        });
        // The first blow takes the body over; its transitions are inserted with it.
        let mut fresh = None;
        let first = struck.is_none();
        let s = match struck {
            Some(s) => s.into_inner(),
            None => {
                let Some(n) = take_over(
                    body,
                    pose,
                    (&gltfs, &mut graphs, &mut materials),
                    &children,
                    &skins,
                    &mut commands,
                    &players,
                ) else {
                    continue;
                };
                fresh.insert(n)
            }
        };
        s.blows += 1;
        s.at = now;
        s.away = away.extend(0.0).xzy();
        let Ok((mut p, tx)) = players.get_mut(s.player) else {
            continue;
        };
        // The first blow starts the body afresh on its own graph (take_over), whatever played
        // on the one before: its node indices mean nothing on this one.
        let fell = match tx {
            Some(mut tx) if !first => start(s, &mut p, &mut tx),
            _ => {
                let mut new = AnimationTransitions::new();
                let fell = begin(s, &mut p, &mut new);
                commands.entity(s.player).insert(new);
                fell
            }
        };
        debug!(
            "blow {} lands on {body} at {:.2}{}",
            s.blows,
            foe,
            if fell { ", it falls" } else { "" }
        );
        // A heavy blow slides it back, a killing one throws it away from the blow (knock.rs);
        // a body with no weight in the data is a man's.
        let w = weight.copied().unwrap_or(Weight {
            kg: super::knock::MASS,
            falls: None,
        });
        if fell {
            s.knock = Some(Knock::death(blow.from, foe, w, now, tr.rotation));
        } else if blow.weight >= 0.5 {
            s.knock = Some(Knock::heavy(blow.from, foe, w, now));
        }
        if fell {
            commands.entity(body).insert(Fallen);
        }
        if let Some(n) = fresh {
            commands.entity(body).insert(n);
        }
    }
    if let (Some(c), Some(mut cues)) = (contact, cues) {
        cues.blow = now;
        cues.contact = Some(c);
        cues.landed = Some(landed);
    }
}

/// A body's first blow on its own graph: its own clip looping, the reaction over it. A body
/// with no hit clip (the treants, the ogre) flinches over its own clip; stopping it alone
/// left the body in its bind pose till it died (iso-forest-demo j2).
fn begin(s: &Struck, p: &mut AnimationPlayer, tx: &mut AnimationTransitions) -> bool {
    p.stop_all();
    if let Some(own) = s.own {
        tx.play(p, own, Duration::ZERO).repeat();
    }
    start(s, p, tx)
}

/// Plays a struck body's reaction to its latest blow; whether it fell.
fn start(s: &Struck, p: &mut AnimationPlayer, tx: &mut AnimationTransitions) -> bool {
    match react(s.blows, s.hit.is_some(), s.death.is_some()) {
        React::Death => {
            tx.play(p, s.death.unwrap(), INTO);
            true
        }
        React::Hit => {
            tx.play(p, s.hit.unwrap(), INTO);
            false
        }
        React::Flinch => false,
    }
}

/// A body's first blow: its clips (its own, hit, death) on a graph of its own, and its
/// materials copied, each lit by its own albedo when it flashes (as hit.rs's ogre).
#[allow(clippy::too_many_arguments)]
fn take_over(
    body: Entity,
    pose: &Pose,
    (gltfs, graphs, materials): (
        &Assets<Gltf>,
        &mut Assets<AnimationGraph>,
        &mut Assets<StandardMaterial>,
    ),
    children: &Query<&Children>,
    skins: &Query<&MeshMaterial3d<StandardMaterial>>,
    commands: &mut Commands,
    players: &Query<(&mut AnimationPlayer, Option<&mut AnimationTransitions>)>,
) -> Option<Struck> {
    let gltf = gltfs.get(&pose.gltf)?;
    let player = player_of(body, children, players)?;
    let (graph, nodes) = graph_of(|n| gltf.named_animations.get(n).cloned(), pose.clip);
    commands
        .entity(player)
        .insert(AnimationGraphHandle(graphs.add(graph)));
    let mut copies: Vec<(AssetId<StandardMaterial>, Handle<StandardMaterial>)> = Vec::new();
    for e in children.iter_descendants(body) {
        let Ok(m) = skins.get(e) else { continue };
        let copy = match copies.iter().find(|c| c.0 == m.0.id()) {
            Some(c) => c.1.clone(),
            None => {
                let Some(mut mat) = materials.get(&m.0).cloned() else {
                    continue;
                };
                if mat.emissive_texture.is_none() {
                    mat.emissive_texture = mat.base_color_texture.clone();
                }
                mat.emissive = LinearRgba::BLACK;
                let h = materials.add(mat);
                copies.push((m.0.id(), h.clone()));
                h
            }
        };
        commands.entity(e).insert(MeshMaterial3d(copy));
    }
    Some(Struck {
        blows: 0,
        at: -10.0,
        tilt: Quat::IDENTITY,
        away: Vec3::X,
        skin: copies.into_iter().map(|c| c.1).collect(),
        player,
        own: nodes.own,
        hit: nodes.hit,
        death: nodes.death,
        gaits: nodes.gaits,
        knock: None,
    })
}

/// A struck body's graph's nodes: its own clip, hit, death and gaits.
struct Nodes {
    own: Option<AnimationNodeIndex>,
    hit: Option<AnimationNodeIndex>,
    death: Option<AnimationNodeIndex>,
    gaits: Vec<(&'static str, AnimationNodeIndex)>,
}

/// A struck body's graph from its clips by name (`clip`): its own clip, its hit and death,
/// and its gaits (`GAITS`), each once, its own one of them when it is a gait. Keeping only
/// own, hit and death left a steered mob walking in place once struck (iso-forest-demo j3).
fn graph_of(
    clip: impl Fn(&str) -> Option<Handle<AnimationClip>>,
    own: &str,
) -> (AnimationGraph, Nodes) {
    let mut graph = AnimationGraph::new();
    let root = graph.root;
    let mut gaits = Vec::new();
    for g in GAITS {
        if let Some(c) = clip(g) {
            gaits.push((g, graph.add_clip(c, 1.0, root)));
        }
    }
    let mut node = |n: &str| match gaits.iter().find(|g| g.0 == n) {
        Some(g) => Some(g.1),
        None => clip(n).map(|c| graph.add_clip(c, 1.0, root)),
    };
    let (own, hit, death) = (node(own), node("hit"), node("death"));
    let nodes = Nodes {
        own,
        hit,
        death,
        gaits,
    };
    (graph, nodes)
}

/// A body back on its own clip once its hit has played.
fn recover(
    bodies: Query<&Struck, Without<Fallen>>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    for s in &bodies {
        let (Some(hit), Some(own)) = (s.hit, s.own) else {
            continue;
        };
        let Ok((mut p, mut tx)) = players.get_mut(s.player) else {
            continue;
        };
        if tx.get_main_animation() == Some(hit) && p.animation(hit).is_none_or(|a| a.is_finished())
        {
            tx.play(&mut p, own, BACK).repeat();
        }
    }
}

/// The flash and flinch since each body's last blow, the flinch's tilt laid on the body's
/// facing as it is this frame (a mob turns as it walks, demo/mobs.rs), and taken off again
/// before the next (`settle`). Writing it over the facing at the first blow froze a mob's
/// turn till it died (iso-forest-demo j2).
fn show(
    time: Res<Time>,
    mut bodies: Query<(&mut Struck, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now = time.elapsed_secs();
    for (mut s, mut tr) in &mut bodies {
        let a = now - s.at;
        let f = flinch(a);
        let tilt = Quat::from_axis_angle(Vec3::Y.cross(s.away).normalize_or(Vec3::X), FLINCH * f);
        tr.rotation = tilt * tr.rotation;
        s.tilt = tilt;
        let e = flash(a);
        for h in &s.skin {
            if materials.get(h).is_some_and(|m| m.emissive != e)
                && let Some(mut m) = materials.get_mut(h)
            {
                m.emissive = e;
            }
        }
    }
}

/// The last frame's flinch taken off each struck body's facing, before anything turns it.
fn settle(mut bodies: Query<(&mut Struck, &mut Transform)>) {
    for (mut s, mut tr) in &mut bodies {
        tr.rotation = s.tilt.inverse() * tr.rotation;
        s.tilt = Quat::IDENTITY;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::animation::graph::AnimationNodeType;

    /// Two hits, then the death on the third blow; a body without a hit clip only flinches
    /// until it dies, and one without a death never falls.
    #[test]
    fn three_blows_fell_a_body() {
        let seq: Vec<_> = (1..=BLOWS).map(|n| react(n, true, true)).collect();
        assert_eq!(seq, [React::Hit, React::Hit, React::Death]);
        assert_eq!(react(1, false, true), React::Flinch);
        assert_eq!(react(BLOWS, false, true), React::Death);
        assert_eq!(react(BLOWS + 2, true, false), React::Hit);
    }

    fn struck(own: Option<usize>, hit: Option<usize>, death: Option<usize>) -> Struck {
        let node = |i: Option<usize>| i.map(AnimationNodeIndex::new);
        Struck {
            blows: 1,
            at: 0.0,
            tilt: Quat::IDENTITY,
            away: Vec3::X,
            skin: Vec::new(),
            player: Entity::PLACEHOLDER,
            own: node(own),
            hit: node(hit),
            death: node(death),
            gaits: Vec::new(),
            knock: None,
        }
    }

    /// A struck body keeps its idle, walk and attack on its own graph beside its hit and
    /// death, each the clip of that name, its own clip one of them, and plays one by name as
    /// its own from then on: a steered mob walks, stops and swings after its first blow. A
    /// hit plays on through a change of gait. The planted fault (only own, hit and death on
    /// the graph, as before) has no walk to play: the mob walks in place.
    #[test]
    fn a_struck_body_keeps_its_gaits() {
        let mut clips = Assets::<AnimationClip>::default();
        let names = ["idle", "walk", "attack", "hit", "death"];
        let handles: Vec<_> = names
            .iter()
            .map(|_| clips.add(AnimationClip::default()))
            .collect();
        let clip = |n: &str| {
            names
                .iter()
                .position(|m| *m == n)
                .map(|i| handles[i].clone())
        };
        let (graph, nodes) = graph_of(clip, "idle");
        let holds = |n: AnimationNodeIndex, name: &str| {
            matches!(&graph.get(n).unwrap().node_type,
                AnimationNodeType::Clip(h) if Some(h.clone()) == clip(name))
        };
        let mut s = struck(None, None, None);
        (s.own, s.hit, s.death, s.gaits) = (nodes.own, nodes.hit, nodes.death, nodes.gaits);
        for g in GAITS {
            assert!(holds(s.node(g).unwrap(), g), "{g}");
        }
        assert_eq!(s.own, s.node("idle"), "its own clip is its idle, once");
        assert!(holds(s.hit.unwrap(), "hit") && holds(s.death.unwrap(), "death"));
        let (mut p, mut tx) = (AnimationPlayer::default(), AnimationTransitions::new());
        // Back on its own idle after its hit (`recover`).
        tx.play(&mut p, s.own.unwrap(), Duration::ZERO).repeat();
        assert!(s.gait("walk", true, Duration::ZERO, &mut p, &mut tx));
        let walk = s.node("walk").unwrap();
        assert!(p.is_playing_animation(walk) && tx.get_main_animation() == Some(walk));
        assert_eq!(s.own, Some(walk), "a hit gives way to its walk now");
        tx.play(&mut p, s.hit.unwrap(), Duration::ZERO);
        assert!(s.gait("idle", true, Duration::ZERO, &mut p, &mut tx));
        assert_eq!(tx.get_main_animation(), s.hit, "the hit plays on");
        assert_eq!(s.own, s.node("idle"));
        let mut fault = struck(Some(1), Some(2), Some(3));
        let mut p = AnimationPlayer::default();
        assert!(!fault.gait("walk", true, Duration::ZERO, &mut p, &mut tx));
        assert_eq!(p.playing_animations().count(), 0, "the fault: no walk");
    }

    /// A body with no hit clip (a treant) keeps playing its own clip through its first
    /// blow's flinch, not its bind pose; one with a hit clip blends into it from its own.
    /// The planted fault, as before the fix: its player stopped and only the reaction
    /// started, so a flinch leaves nothing playing.
    #[test]
    fn a_first_blow_keeps_a_clip_playing() {
        let own = AnimationNodeIndex::new(1);
        let s = struck(Some(1), None, Some(2));
        let mut p = AnimationPlayer::default();
        assert!(!begin(&s, &mut p, &mut AnimationTransitions::new()));
        assert!(p.is_playing_animation(own), "the flinch over its own clip");
        let mut fault = AnimationPlayer::default();
        fault.stop_all();
        start(&s, &mut fault, &mut AnimationTransitions::new());
        assert_eq!(
            fault.playing_animations().count(),
            0,
            "the fault: bind pose"
        );
        let s = struck(Some(1), Some(2), Some(3));
        let mut p = AnimationPlayer::default();
        let mut tx = AnimationTransitions::new();
        begin(&s, &mut p, &mut tx);
        assert_eq!(tx.get_main_animation(), Some(AnimationNodeIndex::new(2)));
        assert!(p.is_playing_animation(own), "fading out under the hit");
    }

    /// The flinch rides on the body's facing as it turns: a mob turning a tenth of a turn a
    /// frame through its flinch faces each frame's way tilted by the flinch, and a body that
    /// never turns (the staged cast) keeps its own facing under the one tilt, the tilt never
    /// compounding. The planted fault (the tilt laid over the facing at the first blow, as
    /// before) leaves the turning mob facing its first way.
    #[test]
    fn the_flinch_rides_on_the_body_s_turn() {
        fn turn(mut bodies: Query<&mut Transform, With<Struck>>, mut n: Local<f32>) {
            *n += 1.0;
            for mut tr in &mut bodies {
                tr.rotation = Quat::from_rotation_y(0.6 * *n);
            }
        }
        let base = Quat::from_rotation_y(0.4);
        let run = |turning: bool, frames: u32| {
            let mut app = App::new();
            app.init_resource::<Time>()
                .init_resource::<Assets<StandardMaterial>>()
                .add_systems(First, settle)
                .add_systems(Update, show);
            if turning {
                app.add_systems(Update, turn.before(show));
            }
            // A tenth of a second into its flinch, held there: time does not run here.
            let mut s = struck(None, None, None);
            s.at = -0.1;
            s.away = Vec3::Z;
            let body = app
                .world_mut()
                .spawn((s, Transform::from_rotation(base)))
                .id();
            for _ in 0..frames {
                app.update();
            }
            app.world().get::<Transform>(body).unwrap().rotation
        };
        let tilt = Quat::from_axis_angle(Vec3::Y.cross(Vec3::Z), FLINCH * flinch(0.1));
        assert!(tilt.angle_between(Quat::IDENTITY) > 0.05, "a flinch to see");
        let near = |a: Quat, b: Quat| a.angle_between(b) < 1e-3;
        let turned = run(true, 3);
        assert!(near(turned, tilt * Quat::from_rotation_y(1.8)), "{turned}");
        assert!(!near(turned, tilt * base), "the fault: its turn frozen");
        let still = run(false, 3);
        assert!(near(still, tilt * base), "{still}");
    }

    /// The bite is on the body's near side, toward the hero, at the height where the blade
    /// passes its axis, kept on the body (between the knees and over the head).
    #[test]
    fn the_bite_is_where_the_blade_meets_the_body() {
        let (hero, foe) = (Vec2::ZERO, Vec2::new(0.0, 2.0));
        // The blade level across the body's axis at 1.1 m, its outer half past it.
        let edge = (Vec3::new(-1.0, 1.1, 2.0), Vec3::new(1.0, 1.1, 2.0));
        let b = bite(hero, foe, Some(edge), 0.0);
        assert!(b.distance(Vec3::new(0.0, 1.1, 2.0 - RADIUS)) < 1e-5, "{b}");
        // Sloping from 2 m at the hilt to 0 m at the tip, halfway across at the axis.
        let edge = (Vec3::new(-1.0, 2.0, 2.0), Vec3::new(1.0, 0.0, 2.0));
        assert!((bite(hero, foe, Some(edge), 0.5).y - 1.0).abs() < 1e-5);
        // Over the head and unknown.
        let high = (Vec3::new(-1.0, 5.0, 2.0), Vec3::new(1.0, 5.0, 2.0));
        assert!((bite(hero, foe, Some(high), 0.0).y - 2.2).abs() < 1e-5);
        assert!((bite(hero, foe, None, 0.3).y - (0.3 + HEIGHT)).abs() < 1e-5);
        // A body pressed against him: the bite stays between them.
        let b = bite(hero, Vec2::new(0.0, 0.3), None, 0.0);
        assert!(b.z > 0.0 && b.z < 0.3, "{b}");
    }
}
