//! Feet kept where they land (`terra_motion::feet`, D455), for every body with legs, whatever
//! it does or holds: after the clip has posed the skeleton and before the pose is carried to
//! the joints' world places, each leg the clip has on the ground is bent so its foot stays
//! where it was locked while the body moves over it (a stride, a move's drive or warp, a turn
//! to the foe), and steps when it has gone too far. A stride's clip plays in place at its pace
//! (`pace.rs`), so its feet are read against the ground it stands on (`Kept::ground`, which the
//! figure sets); a clip that stands is read in the body's frame. When a foot is locked is its
//! clips' own data, the lock curves baked into their file (`markers`, `terra_motion::feet::
//! Markers`), read at the time each clip playing on the legs is at and summed by its weight, as
//! ALS sums its FootLock curves. A body whose feet cannot be kept (no legs, no animation player,
//! no clip with markers) says so: in the fight log (`Kept::gated`) and once in the log. Each
//! foot is then put on the walk's own ground under it (`terra_motion::placement`, asked
//! through `underfoot`): probed once a lock, a swinging foot once a frame, and the pelvis
//! lowered for the lower foot. Only the pelvis's place and the thigh, calf and foot's turns are
//! written (the pelvis's drop taken out again next frame where nothing rewrote it), so nothing
//! else that writes the joints after the animation (the body's proportions, its lift) is
//! disturbed.

use std::collections::HashMap;

use bevy::{
    animation::{AnimatedBy, AnimationTargetId},
    gltf::{Gltf, GltfNode},
    prelude::*,
    transform::TransformSystems,
};
use terra_motion::feet::{self, Foot, Markers, Pose, Posed};
use terra_motion::placement::{self, Placing};

use crate::stepping::Stepping;
use crate::underfoot::Underfoot;
use crate::{Downed, Motion, character};

/// Every clip's foot-down markers, by the clip loaded: whoever loads a body's clips registers
/// them (`Baked::register`), so a foot is kept by the clip it plays whichever file it is from.
#[derive(Resource, Default)]
pub struct Baked(HashMap<AssetId<AnimationClip>, Markers>);

impl Baked {
    /// The markers a body file carries (`markers`), for each of its clips as loaded.
    pub fn register(&mut self, gltf: &Gltf, nodes: &Assets<GltfNode>) {
        let markers = markers(gltf, nodes);
        for (name, clip) in &gltf.named_animations {
            match markers.get(name.as_ref()) {
                Some(each) => {
                    self.0.insert(clip.id(), each.clone());
                }
                None => {
                    warn!("The clip {name} has no foot-down markers (tools/characters/markers.ts).")
                }
            }
        }
    }
}

/// The foot-down markers kept in a body file, by clip (`tools/characters/markers.ts`):
/// `{"markers": {"Jog_Fwd_Loop": {...}}}` in its glTF extras, beside the paces.
pub fn markers(gltf: &Gltf, nodes: &Assets<GltfNode>) -> HashMap<String, Markers> {
    gltf.nodes
        .iter()
        .filter_map(|node| nodes.get(node)?.extras.as_ref())
        .find_map(|extras| {
            let value: serde_json::Value = serde_json::from_str(&extras.value).ok()?;
            serde_json::from_value(value.get("markers")?.clone()).ok()
        })
        .unwrap_or_default()
}

/// Each foot's lock curve over the clips playing on `animator` (`Markers::lock` at the time
/// each is at), summed by their weights, as ALS sums its FootLock curves; a clip masked off
/// that leg (`leg`, its foot's target) or without markers is left out. Across a cross-fade only
/// the clips coming in are read (their weight not falling since `was`, which is kept for next
/// frame): a foot stands as the pose it goes to has it, as motion matching labels contact by
/// the pose it blends to (inertialization) and not by the mix, which in a fade from a stride
/// or a fall into a stance reads half-locked and so neither locks nor lets go. Why there is
/// none, if there is none.
fn locks(
    animator: &AnimationPlayer,
    graph: &AnimationGraph,
    baked: &Baked,
    leg: [Option<AnimationTargetId>; 2],
    was: &mut Vec<(AnimationNodeIndex, f32)>,
) -> Result<[f32; 2], &'static str> {
    // [all, coming in] for each foot: the curve summed by weight, and the weight.
    let (mut lock, mut whole, mut playing) = ([[0.0; 2]; 2], [[0.0; 2]; 2], false);
    let mut now = Vec::with_capacity(was.len());
    for (node, active) in animator.playing_animations() {
        let Some(each) = graph.get(*node) else {
            continue;
        };
        let AnimationNodeType::Clip(clip) = &each.node_type else {
            continue;
        };
        playing = true;
        let Some(markers) = baked.0.get(&clip.id()) else {
            continue;
        };
        let weight = active.weight() * each.weight;
        if weight <= 0.0 {
            continue;
        }
        now.push((*node, weight));
        let fading = was
            .iter()
            .any(|&(seen, last)| seen == *node && weight < last - 1e-4);
        for side in 0..2 {
            let masked = leg[side]
                .and_then(|target| graph.mask_groups.get(&target))
                .is_some_and(|groups| groups & each.mask != 0);
            if masked {
                continue;
            }
            let curve = markers.lock(side, active.seek_time());
            for (k, counts) in [true, !fading].into_iter().enumerate() {
                if counts {
                    whole[k][side] += weight;
                    lock[k][side] += weight * curve;
                }
            }
        }
    }
    *was = now;
    if whole[0].iter().all(|w| *w <= 0.0) {
        return Err(if playing {
            "no markers"
        } else {
            "nothing playing"
        });
    }
    Ok([0, 1].map(|side| {
        let k = if whole[1][side] > 0.0 { 1 } else { 0 };
        if whole[k][side] > 0.0 {
            lock[k][side] / whole[k][side]
        } else {
            0.0
        }
    }))
}

/// A body whose feet cannot be kept this long says so in the log.
const GATED_LOUD_S: f32 = 1.0;

/// Each foot's ball of the foot, left then right.
const BALLS: [&str; 2] = ["ball_l", "ball_r"];

/// Each leg's hip, knee and ankle, left then right.
const LEGS: [[&str; 3]; 2] = [
    ["thigh_l", "calf_l", "foot_l"],
    ["thigh_r", "calf_r", "foot_r"],
];

/// A body's two feet as kept, and its legs' joints once found.
#[derive(Component, Default)]
pub struct Kept {
    feet: [Foot; 2],
    strained: [bool; 2],
    /// Each clip foot's place in the body's own frame last frame, for how fast it swings.
    was: [Option<Vec3>; 2],
    legs: Option<[[Entity; 3]; 2]>,
    /// Each foot's ball of the foot, if it has one: a locked foot keeps it still.
    balls: [Option<Entity>; 2],
    /// The animation player posing its skeleton, once found.
    animator: Option<Entity>,
    /// How fast the ground the clip's feet stand on passes under the body, in the world, m/s:
    /// the body's velocity over the ground while a clip that travels plays at its pace, zero
    /// for a clip that stands. Whoever plays the clip sets it (`figure`).
    pub ground: Vec3,
    /// How much longer than its clip's the stride playing is shown, less 1 (`pace::gait`'s
    /// warp): each foot's travel along the way is stretched so (`stride`).
    warp_by: f32,
    /// How far each planted leg asks the pelvis down to reach its warped foot, m (`warped`).
    reach_need: [f32; 2],
    /// A layer above lets the feet go this frame (a fighter down, a roll): they go as the clip
    /// has them. Set each frame by whoever knows.
    pub let_go: bool,
    /// A layer above plays its own clip set this frame (a fighter's move set): its feet are
    /// read in the body's own frame whatever it plays, its walk too, as a stance's are (D455).
    /// Else a stride's feet are read on the ground it passes. Set each frame by whoever knows,
    /// like `let_go`: both last the frame.
    pub stance: bool,
    /// Off the ground last frame, so the frame it comes down is known.
    was_off: bool,
    /// Each foot's heel (its ankle) and ball over the body's ground along its up, as the clip
    /// posed them before the lock and the bend: the clip's own sole over its flat ground, which
    /// the walk record holds against the world's (`feet.ground`).
    clip_soles: [[f32; 2]; 2],
    /// Each leg's shortfall from the goal it is bent to, m, and the shortfall the lock's goal
    /// alone leaves, before the foot is placed on the ground: what placement adds
    /// (`pelvis.drop`) is their difference.
    shorts: [[f32; 2]; 2],
    /// Each foot as placed last frame, heel and ball, over its ground past where its clip had
    /// it (`placement::Grounded::over`): a lock starts only once it is down (`feet::PLANT_M`).
    over: [[f32; 2]; 2],
    /// Each foot put on the ground under it and the pelvis lowered for it
    /// (`terra_motion::placement`).
    placing: Placing,
    /// The pelvis as lowered last frame: the joint, its translation as written, and what was
    /// added to it.
    pelvis: Option<(Entity, Vec3, Vec3)>,
    /// Each foot's lock curve over the clips playing this frame.
    curve: [f32; 2],
    /// Each clip node's weight last frame, to tell the clips coming in from those going.
    weights: Vec<(AnimationNodeIndex, f32)>,
    /// Why the feet were not kept this frame (no legs, no animator, no markers), if not.
    gated: Option<&'static str>,
    /// How long they have been, and whether that was logged.
    gated_s: f32,
    told: bool,
}

impl Kept {
    /// The clip playing travels `pace` metres a second (scaled to the body) at `rate`, `way`,
    /// its feet's travel along the way stretched `warp` times (`pace::gait`, stride warping):
    /// the ground its planted feet stand on passes under the body that fast (none for a clip
    /// that stands, of no pace).
    pub fn stride(&mut self, way: Vec3, pace: f32, rate: f32, warp: f32) {
        self.ground = way.normalize_or_zero() * pace * rate.abs() * warp;
        self.warp_by = if pace > 0.0 { warp - 1.0 } else { 0.0 };
    }

    /// The clip's foot `ankle` and ball `ball` as a warped stride shows them (Unreal's Stride
    /// Warping, its documentation): their place along the way the body goes, from the hip
    /// `hip`, stretched by the warp; and how far the pelvis must come down for the leg to
    /// reach it no farther than the clip's leg reaches its own, asked only of a foot its clip
    /// has down (`curve` over 0), as a swinging leg is bent.
    fn warped(
        &self,
        ankle: Vec3,
        ball: Vec3,
        hip: Vec3,
        up: Vec3,
        curve: f32,
    ) -> (Vec3, Vec3, f32) {
        let way = self.ground.reject_from(up).normalize_or_zero();
        if self.warp_by == 0.0 || way == Vec3::ZERO {
            return (ankle, ball, 0.0);
        }
        let by = way * (ankle - hip).dot(way) * self.warp_by;
        let need = if curve > 0.0 {
            let reach = (hip - ankle).length();
            let from = hip - (ankle + by);
            let (high, across) = (from.dot(up), from.reject_from(up).length_squared());
            (high - (reach * reach - across).max(0.0).sqrt()).max(0.0)
        } else {
            0.0
        };
        (ankle + by, ball + by, need)
    }

    /// How each foot is kept, left then right, as the fight log writes it.
    pub fn states(&self) -> [&'static str; 2] {
        self.feet.map(|foot| foot.state())
    }

    /// Each foot's lock curve over the clips playing, and its lock weight, as the fight log
    /// writes them.
    pub fn locks(&self) -> [[f32; 2]; 2] {
        [0, 1].map(|side| [self.curve[side], self.feet[side].alpha()])
    }

    /// Why each foot was left free while its curve had it locked (`Foot::why`).
    pub fn whys(&self) -> [&'static str; 2] {
        self.feet.map(|foot| foot.why())
    }

    /// Why the feet were not kept at all this frame, if not.
    pub fn gated(&self) -> Option<&'static str> {
        self.gated
    }

    /// Each foot's heel and ball over the body's ground as its clip posed them (`clip_soles`).
    pub fn clip_soles(&self) -> [[f32; 2]; 2] {
        self.clip_soles
    }

    /// Each leg's shortfall from its goal and the lock's goal's alone, m (`shorts`).
    pub fn shorts(&self) -> [[f32; 2]; 2] {
        self.shorts
    }

    /// Each foot's lock curve this frame over the clips `player` plays (`locks`), kept for the
    /// record; none, and why noted (`gate`), when the feet cannot be kept.
    fn curve_of(
        &mut self,
        body: Entity,
        player: Option<(&AnimationPlayer, &AnimationGraphHandle)>,
        graphs: &Assets<AnimationGraph>,
        baked: &Baked,
        leg: [Option<AnimationTargetId>; 2],
        dt: f32,
    ) -> Option<[f32; 2]> {
        let curve = match player {
            None => Err("no animator"),
            Some((animator, graph)) => match graphs.get(graph) {
                None => Err("no graph"),
                Some(graph) => locks(animator, graph, baked, leg, &mut self.weights),
            },
        };
        let curve = match curve {
            Ok(curve) => {
                self.gate(body, None, dt);
                curve
            }
            // A body showing its rest pose, no clip yet, stands on both feet: the clip's
            // first pose is stepped to, not slid to.
            Err(why @ "nothing playing") => {
                self.gate(body, Some(why), dt);
                [1.0; 2]
            }
            Err(why) => {
                self.gate(body, Some(why), dt);
                return None;
            }
        };
        self.curve = curve;
        Some(curve)
    }

    /// Notes why the feet are not kept this frame, and says so once it has lasted.
    fn gate(&mut self, body: Entity, why: Option<&'static str>, dt: f32) {
        self.gated = why;
        let Some(why) = why else {
            self.gated_s = 0.0;
            return;
        };
        self.gated_s += dt;
        if self.gated_s >= GATED_LOUD_S && !self.told {
            self.told = true;
            warn!("Feet: {body}'s feet are not kept ({why}); they go as the clip has them.");
        }
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<Baked>().add_systems(
        PostUpdate,
        keep.after(bevy::app::AnimationSystems)
            .after(crate::Fitted)
            .before(TransformSystems::Propagate),
    );
}

/// An entity's place in the world from its own and its ancestors' transforms as they are now,
/// before this frame's propagation.
fn world(
    entity: Entity,
    transforms: &Query<&mut Transform>,
    parents: &Query<&ChildOf>,
) -> Option<GlobalTransform> {
    let mut place = GlobalTransform::from(*transforms.get(entity).ok()?);
    for each in parents.iter_ancestors(entity) {
        place = *transforms.get(each).ok()? * place;
    }
    Some(place)
}

/// A leg's joints in the world as they are posed now: the hips it hangs from, the hip, knee
/// and ankle.
struct Leg {
    hips: GlobalTransform,
    thigh: GlobalTransform,
    calf: GlobalTransform,
    foot: GlobalTransform,
}

impl Leg {
    fn of(
        [thigh, calf, foot]: [Entity; 3],
        transforms: &Query<&mut Transform>,
        parents: &Query<&ChildOf>,
    ) -> Option<Leg> {
        let hips = world(parents.get(thigh).ok()?.parent(), transforms, parents)?;
        let at_thigh = hips * *transforms.get(thigh).ok()?;
        let at_calf = at_thigh * *transforms.get(calf).ok()?;
        let at_foot = at_calf * *transforms.get(foot).ok()?;
        Some(Leg {
            hips,
            thigh: at_thigh,
            calf: at_calf,
            foot: at_foot,
        })
    }

    /// The knee and ankle bent to reach `goal` (D455's two-bone solve), the knee on the side
    /// the clip bends it (`ahead` if straight).
    fn solve(&self, goal: Vec3, ahead: Vec3) -> (Vec3, Vec3) {
        let (hip, knee, ankle) = (
            self.thigh.translation(),
            self.calf.translation(),
            self.foot.translation(),
        );
        let (upper, lower) = ((knee - hip).length(), (ankle - knee).length());
        let pole = knee + (knee - (hip + ankle) * 0.5).normalize_or(ahead) * 0.5;
        feet::two_bone(hip, upper, lower, goal, pole)
    }

    /// Bends the leg `joints` so its ankle goes to `goal`: how far short of it the ankle stays,
    /// m.
    fn bend(
        &self,
        [thigh, calf, foot]: [Entity; 3],
        goal: Pose,
        ahead: Vec3,
        transforms: &mut Query<&mut Transform>,
    ) -> f32 {
        let (knee, ankle) = (self.calf.translation(), self.foot.translation());
        let hip = self.thigh.translation();
        let (to_knee, to_ankle) = self.solve(goal.at, ahead);
        let turn_thigh =
            Quat::from_rotation_arc((knee - hip).normalize(), (to_knee - hip).normalize());
        let swung = to_knee + turn_thigh * (ankle - knee);
        let turn_calf = Quat::from_rotation_arc(
            (swung - to_knee).normalize(),
            (to_ankle - to_knee).normalize(),
        );
        let thigh_turn = turn_thigh * self.thigh.rotation();
        let calf_turn = turn_calf * turn_thigh * self.calf.rotation();
        let mut set = |entity: Entity, rotation: Quat| {
            if let Ok(mut transform) = transforms.get_mut(entity) {
                transform.rotation = rotation.normalize();
            }
        };
        set(thigh, self.hips.rotation().inverse() * thigh_turn);
        set(calf, thigh_turn.inverse() * calf_turn);
        set(foot, calf_turn.inverse() * goal.turn);
        (to_ankle - goal.at).length()
    }
}

/// What a body's legs are kept by this frame, the same for both.
struct Frame {
    curve: [f32; 2],
    base: GlobalTransform,
    /// The body's place (its feet), and the clip's flat ground: the figure's root as shown,
    /// under the body after a step up (`Stepping::below`).
    ground: Vec3,
    root: Vec3,
    up: Vec3,
    off: bool,
    landing: bool,
    passing: Option<Vec3>,
    dt: f32,
}

type Bodies<'a> = (
    Entity,
    &'a mut Kept,
    Option<&'a Motion>,
    Has<Downed>,
    Option<&'a Stepping>,
);

#[allow(clippy::too_many_arguments)]
fn keep(
    time: Res<Time>,
    baked: Res<Baked>,
    graphs: Res<Assets<AnimationGraph>>,
    animators: Query<(&AnimationPlayer, &AnimationGraphHandle)>,
    targets: Query<&AnimationTargetId>,
    animated_by: Query<&AnimatedBy>,
    mut bodies: Query<Bodies<'_>>,
    children: Query<&Children>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
    mut transforms: Query<&mut Transform>,
    mut underfoot: Underfoot,
) {
    let dt = time.delta_secs();
    for (body, mut kept, motion, downed, stepping) in &mut bodies {
        if kept.legs.is_none() {
            let found =
                LEGS.map(|leg| leg.map(|name| character::find(body, name, &children, &names)));
            if found.iter().flatten().any(Option::is_none) {
                kept.gate(body, Some("no legs"), dt);
                continue;
            }
            kept.legs = Some(found.map(|leg| leg.map(Option::unwrap)));
            kept.balls = BALLS.map(|name| character::find(body, name, &children, &names));
        }
        let Some(legs) = kept.legs else { continue };
        if kept.animator.is_none() {
            // The player animating its legs (not one a held thing carries).
            kept.animator = animated_by
                .get(legs[0][2])
                .ok()
                .map(|by| by.0)
                .filter(|&each| animators.contains(each));
        }
        // Each foot's lock curve over its clips, by their markers.
        let leg = legs.map(|[.., foot]| targets.get(foot).ok().copied());
        let player = kept.animator.and_then(|each| animators.get(each).ok());
        let Some(curve) = kept.curve_of(body, player, &graphs, &baked, leg, dt) else {
            continue;
        };
        // What a layer above set this frame lasts the frame: a body no longer a fighter's
        // reads a stride's feet on the ground it passes again.
        let let_go = std::mem::take(&mut kept.let_go);
        let stance = std::mem::take(&mut kept.stance);
        kept.restore_pelvis(&mut transforms);
        let Some(base) = world(body, &transforms, &parents) else {
            continue;
        };
        let ground = base.translation();
        let up = ground.normalize_or(Vec3::Y);
        // Down, off the ground (a jump, a fall, swimming), or let go by a layer above.
        let aloft = motion.is_some_and(|motion| !motion.on_ground);
        let landing = kept.was_off && !aloft;
        kept.was_off = aloft;
        let passing = (!stance).then(|| base.affine().inverse().transform_vector3(kept.ground));
        let frame = Frame {
            curve,
            base,
            ground,
            root: ground - up * stepping.map_or(0.0, |s| s.below),
            up,
            off: let_go || downed || aloft,
            landing,
            passing,
            dt,
        };
        // Each foot locked and its ground found, then the pelvis lowered for the lower, then
        // each leg bent to its foot placed.
        let grounded = [0, 1].map(|side| {
            kept.ground_leg(
                side,
                legs[side],
                &frame,
                &transforms,
                &parents,
                &mut underfoot,
            )
        });
        let stride = kept.reach_need[0].max(kept.reach_need[1]);
        let drop = match &grounded {
            [Some((_, left)), Some((_, right))] => {
                kept.placing
                    .pelvis_with([left, right], frame.root, dt, stride)
            }
            _ => 0.0,
        };
        if let Ok(pelvis) = parents.get(legs[0][0]).map(ChildOf::parent) {
            kept.drop_pelvis(pelvis, drop, up, &mut transforms, &parents);
        }
        for (side, each) in grounded.into_iter().enumerate() {
            kept.over[side] = [0.0; 2];
            if let Some((clip, grounded)) = each {
                let goal = grounded.placed(drop);
                kept.over[side] = grounded.over(goal);
                kept.bend_leg(
                    side,
                    legs[side],
                    clip,
                    goal,
                    drop,
                    &frame,
                    &mut transforms,
                    &parents,
                );
            }
        }
    }
}

impl Kept {
    /// Leg `side`'s foot this frame: locked by its curve (`Foot::update`) and its ground found
    /// (`Placing::ground`), with the clip's own pose of it; none without its joints.
    fn ground_leg(
        &mut self,
        side: usize,
        joints: [Entity; 3],
        frame: &Frame,
        transforms: &Query<&mut Transform>,
        parents: &Query<&ChildOf>,
        underfoot: &mut Underfoot,
    ) -> Option<(Pose, placement::Grounded)> {
        let Frame {
            ground, root, up, ..
        } = *frame;
        self.shorts[side] = [0.0; 2];
        let leg = Leg::of(joints, transforms, parents)?;
        let ankle = leg.foot.translation();
        let clip = Pose {
            at: ankle,
            turn: leg.foot.rotation(),
        };
        let ball_local = self.balls[side].and_then(|ball| transforms.get(ball).ok());
        let clip_ball = ball_local.map_or(ankle, |ball| leg.foot.transform_point(ball.translation));
        self.clip_soles[side] = [(ankle - ground).dot(up), (clip_ball - ground).dot(up)];
        // A stride read on the ground it passes is shown as warped: its feet stretched along
        // the way, the lock and placement taking them from there.
        let hip = leg.thigh.translation();
        let (ankle, clip_ball, need) = if frame.passing.is_some() && !frame.off {
            self.warped(ankle, clip_ball, hip, up, frame.curve[side])
        } else {
            (ankle, clip_ball, 0.0)
        };
        self.reach_need[side] = need;
        let shown = Pose {
            at: ankle,
            turn: clip.turn,
        };
        // The ball in the foot's frame at the figure's size: the joint's own scale is not all of
        // it (a body drawn 1.08 times its rig puts the ball 1.1 cm farther out than the bone's
        // offset scaled by the foot alone).
        let ball = clip.turn.inverse() * (clip_ball - ankle);
        let in_body = frame.base.affine().inverse().transform_point3(ankle);
        let dt = frame.dt;
        let swing_mps = self.was[side].map_or(0.0, |was| match frame.passing {
            Some(passing) => feet::swing_mps(was, in_body, passing, Vec3::Y, dt),
            None if dt > 0.0 => (in_body - was).length() / dt,
            None => 0.0,
        });
        self.was[side] = Some(in_body);
        let posed = Posed {
            pose: shown,
            swing_mps,
            lock: frame.curve[side],
            up,
            strained: self.strained[side],
            off: frame.off,
            landing: frame.landing,
            low: (ankle - ground).dot(up) <= feet::LOW_ANKLE_M,
            ball,
            sole: [(ankle - root).dot(up), (clip_ball - root).dot(up)],
            over: self.over[side],
        };
        let other = self.feet[1 - side].moving();
        let kept_at = self.feet[side].update(posed, dt, other);
        // What the lock's goal alone leaves the leg short, before the foot is placed and the
        // pelvis lowered.
        let (_, to_lock) = leg.solve(kept_at.at, *frame.base.forward());
        self.shorts[side][1] = (to_lock - kept_at.at).length();
        let reach = hip.distance(leg.calf.translation()) + leg.calf.translation().distance(ankle);
        let grounded = self.placing.ground(
            side,
            placement::Foot {
                shown: kept_at,
                ball,
                lock: self.feet[side].locked(),
                root,
                up,
                off: frame.off,
                hip,
                reach,
            },
            dt,
            |at| underfoot.under(at, root, up, (ground - root).dot(up)),
        );
        Some((clip, grounded))
    }

    /// Leg `side` bent to `goal`, its foot placed, the pelvis lowered by `drop`; left as the
    /// clip has it where neither moved it.
    #[allow(clippy::too_many_arguments)]
    fn bend_leg(
        &mut self,
        side: usize,
        joints: [Entity; 3],
        clip: Pose,
        goal: Pose,
        drop: f32,
        frame: &Frame,
        transforms: &mut Query<&mut Transform>,
        parents: &Query<&ChildOf>,
    ) {
        if goal == clip && drop <= DROP_STILL_M {
            self.strained[side] = false;
            self.shorts[side] = [0.0; 2];
            return;
        }
        let Some(leg) = Leg::of(joints, transforms, parents) else {
            return;
        };
        let short = leg.bend(joints, goal, *frame.base.forward(), transforms);
        self.strained[side] = short > 0.01;
        self.shorts[side][0] = short;
    }

    /// The pelvis as the clip has it: last frame's drop taken out again where nothing wrote the
    /// joint since (a clip that does not move the pelvis).
    fn restore_pelvis(&mut self, transforms: &mut Query<&mut Transform>) {
        if let Some((pelvis, written, by)) = self.pelvis.take()
            && let Ok(mut transform) = transforms.get_mut(pelvis)
            && transform.translation == written
        {
            transform.translation -= by;
        }
    }

    /// Lowers the pelvis (the joint the thighs hang from) by `drop` along `up`, the whole body
    /// over it with it (ALS's pelvis offset).
    fn drop_pelvis(
        &mut self,
        pelvis: Entity,
        drop: f32,
        up: Vec3,
        transforms: &mut Query<&mut Transform>,
        parents: &Query<&ChildOf>,
    ) {
        if drop <= DROP_STILL_M {
            return;
        }
        let Some(over) = parents
            .get(pelvis)
            .ok()
            .and_then(|parent| world(parent.parent(), transforms, parents))
        else {
            return;
        };
        let by = over.affine().inverse().transform_vector3(-up * drop);
        if let Ok(mut transform) = transforms.get_mut(pelvis) {
            transform.translation += by;
            self.pelvis = Some((pelvis, transform.translation, by));
        }
    }
}

/// A pelvis dropped by less than this is left where the clip has it, m.
const DROP_STILL_M: f32 = 1e-4;

#[cfg(test)]
mod tests {
    use super::*;

    /// The body files whose clips the game plays: every clip carries its foot locks
    /// (`tools/characters/markers.ts`; a re-export without them leaves every foot free), a
    /// stride has each foot locked for part of its cycle, not all of it, and a walk always one.
    #[test]
    fn every_clip_played_carries_its_foot_down_markers() {
        for file in [crate::appearance::BASE, crate::figure::CLIPS] {
            let json = crate::model_files::json(file);
            let markers: HashMap<String, Markers> = serde_json::from_value(
                crate::model_files::extra(&json, "markers")
                    .unwrap_or_else(|| panic!("{file} carries no markers")),
            )
            .unwrap();
            let paces: HashMap<String, f32> =
                serde_json::from_value(crate::model_files::extra(&json, "paces").unwrap()).unwrap();
            for clip in crate::model_files::animations(&json) {
                let each = markers
                    .get(&clip)
                    .unwrap_or_else(|| panic!("{file}: {clip} has no markers"));
                if !paces.contains_key(&clip) {
                    continue;
                }
                let at = |side, i: usize| each.lock(side, each.length_s * i as f32 / 100.0);
                for side in 0..2 {
                    let down = (0..100).filter(|&i| at(side, i) >= feet::FULL).count();
                    assert!(
                        (5..95).contains(&down),
                        "{file}: {clip}'s foot {side} is locked {down}% of its stride"
                    );
                }
                // A walk always has a foot down (Crouch_Fwd_Loop's plants skate: to remake).
                if paces[&clip] < 2.0 && clip != "Crouch_Fwd_Loop" {
                    let neither = (0..100).find(|&i| at(0, i) <= 0.0 && at(1, i) <= 0.0);
                    assert_eq!(neither, None, "{file}: {clip} has neither foot down");
                }
            }
        }
    }
}
