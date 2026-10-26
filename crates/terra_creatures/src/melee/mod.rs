//! Sword and shield, the movement prototype (D405, docs/movement.md "Prototype"): T draws a
//! sword and shield and brings a sparring partner, who fights back on a rhythm a player can
//! read. Drawn, WASD strafes facing the view (or the partner, locked on with Q), Space jumps,
//! Ctrl rolls through blows, left click runs the light chain and Shift with it the heavy
//! blow, and right click held blocks, a guard raised just before a blow parrying it. Each
//! move costs the walker's stamina as its set says, and a blow on the guard costs it too.
//!
//! The rules are `terra_motion`'s (D447): the moves as data (`assets/data/moves/
//! sword_and_shield.ron`) and their runner, a blade swept against a body's hurt capsule, and
//! what a blow does. Here is only what reads and writes the bodies, plays clips, flashes and
//! sounds: the player's stance below, what a blow shows in `contact` (where the blade met the
//! body in `burst`, D487), the partner's mind in `partner`, the bars and the controls on
//! screen in `hud`.

mod burst;
mod contact;
mod feet;
mod hud;
mod partner;
mod record;
mod script;
mod sole;

use bevy::{gltf::GltfAssetLabel, prelude::*};
use std::ops::{Deref, DerefMut};
use std::time::Duration;
use terra_motion::blade;
use terra_motion::contact::{Fighting, NEAREST_M};
use terra_motion::moves::{self, MoveSet, Ran};
use terra_motion::stamina;
use terra_survival::held::InHand;
use terra_survival::inventory::HandsFull;
use terra_walker::camera::framing::FightView;
use terra_walker::character;
use terra_walker::controls::Action;
use terra_walker::figure::{Act, Acting, Grip, Moves};
use terra_walker::intent::Intent;
use terra_walker::walker::Steer;
use terra_walker::{Downed, Motion, Player, Stamina, Stance, View};
use terra_world::tick::Sim;

/// The move set every fighter here uses.
const SET: &str = "moves/sword_and_shield.ron";
/// How fast a drawn body walks, metres a second, and with its guard up.
const PACE: f32 = 2.2;
const GUARD_PACE: f32 = 1.4;
/// A fighter's health in a spar.
pub const HEALTH: f32 = 100.0;
/// How far the lock-on reaches, metres.
const LOCK_M: f32 = 15.0;
/// How long a beaten fighter lies before the spar starts again, seconds.
const DOWN_S: f32 = 3.0;
/// The looks drawn: a sword in the right hand, a round shield in the left, which its file
/// straps to the forearm (`terra_walker::strap`).
const SWORD: &str = "models/held/arming_sword.glb";
const SHIELD: &str = "models/held/round_shield.glb";
const HAND_L: &str = "socket.hand.L";
/// The walk a drawn body goes forward and back in: the library's walk with both arms held as
/// the guard holds them (D458), so the sword and shield stay out of the legs.
const WALK: &str = "Fight_Walk";

/// The move set, read once.
#[derive(Resource)]
pub struct Set(pub MoveSet);

/// A body in a spar, the player's or the partner's: its state in the fight by
/// `terra_motion`'s rules (its move, health, guard, hit-stop, reeling, being down, a shove),
/// which it derefs to, and what the game keeps of it: its sword hand, its blade's last place
/// and the way its move drives it.
#[derive(Component)]
#[require(terra_walker::feet::Kept)]
pub struct Fighter {
    pub fight: Fighting,
    /// The socket in its right hand, whose frame the sword's blade is in.
    pub hand: Option<Entity>,
    pub was: Option<blade::Edge>,
    /// The way its move drives it: ahead when it started, or where a roll goes.
    pub way: Vec3,
    /// In its blow's active frames last tick, so the swing is heard once.
    pub swung: bool,
}

impl Fighter {
    fn new(hand: Option<Entity>) -> Self {
        Self {
            fight: Fighting::new(HEALTH),
            hand,
            was: None,
            way: Vec3::ZERO,
            swung: false,
        }
    }
}

impl Deref for Fighter {
    type Target = Fighting;
    fn deref(&self) -> &Fighting {
        &self.fight
    }
}

impl DerefMut for Fighter {
    fn deref_mut(&mut self) -> &mut Fighting {
        &mut self.fight
    }
}

/// A player with a sword drawn: what they are locked on to, the looks in their hands, and
/// the clip the stance last asked of the figure, so it is asked once.
#[derive(Component)]
pub struct Drawn {
    lock: Option<Entity>,
    looks: Vec<Entity>,
    showing: Option<(String, f32)>,
}

pub fn plugin(app: &mut App) {
    let text = std::fs::read_to_string(terra_world::data::path(SET))
        .unwrap_or_else(|problem| panic!("Could not read {SET}: {problem}"));
    let set = MoveSet::read(&text).unwrap_or_else(|problem| panic!("{SET}: {problem}"));
    app.insert_resource(Set(set))
        .add_systems(
            FixedUpdate,
            (draw, act, partner::think, contact::strike, contact::again)
                .chain()
                .before(Sim::Act),
        )
        .add_systems(Update, (contact::fade, view));
    feet::plugin(app);
    burst::plugin(app);
    hud::plugin(app);
    record::plugin(app);
    script::plugin(app);
}

/// T draws a sword and shield, or sheathes them: drawn, the bar's item is put away and a
/// sparring partner steps up if none is near.
fn draw(
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut players: Query<
        (
            Entity,
            &Intent,
            &Transform,
            &Motion,
            Option<&Drawn>,
            &mut Stance,
        ),
        With<Player>,
    >,
    mut grips: Query<(Entity, &Grip, &mut InHand)>,
    partners: Query<&Transform, With<partner::Partner>>,
    children: Query<&Children>,
    names: Query<&Name>,
) {
    for (player, intent, at, motion, drawn, mut stance) in &mut players {
        if !intent.pressed(Action::Draw) || intent.busy {
            continue;
        }
        let grip = grips.iter_mut().find(|(_, grip, _)| grip.player == player);
        let Some((hand, _, mut in_hand)) = grip else {
            continue;
        };
        if let Some(drawn) = drawn {
            for &look in &drawn.looks {
                commands.entity(look).despawn();
            }
            in_hand.drawn = false;
            stance.steer = Steer::default();
            commands
                .entity(player)
                .remove::<(Drawn, Fighter, HandsFull, FightView)>()
                .insert(Acting::default());
            info!("Melee: sheathed");
            continue;
        }
        in_hand.drawn = true;
        let sword = character::hold(
            &mut commands,
            assets.load(GltfAssetLabel::Scene(0).from_asset(SWORD)),
            hand,
        );
        let mut looks = vec![sword];
        match character::find(player, HAND_L, &children, &names) {
            Some(left) => looks.push(character::hold(
                &mut commands,
                assets.load(GltfAssetLabel::Scene(0).from_asset(SHIELD)),
                left,
            )),
            None => warn!("Melee: the figure has no {HAND_L}"),
        }
        commands.entity(player).insert((
            Drawn {
                lock: None,
                looks,
                showing: None,
            },
            Fighter::new(Some(hand)),
            Acting::default(),
            HandsFull,
            FightView::default(),
        ));
        info!("Melee: drawn");
        let near = partners
            .iter()
            .any(|place| place.translation.distance(at.translation) < 30.0);
        if !near {
            partner::bring(&mut commands, at.translation, motion.facing);
        }
    }
}

/// The camera frames the fight, keeping what the player is locked on to in view (D180).
fn view(mut players: Query<(&Drawn, &mut FightView), Changed<Drawn>>) {
    for (drawn, mut view) in &mut players {
        view.target = drawn.lock;
    }
}

type Drawing<'a> = (
    &'a Transform,
    &'a Motion,
    &'a mut Intent,
    &'a mut View,
    &'a mut Stance,
    &'a mut Fighter,
    &'a mut Drawn,
    &'a mut Acting,
    Option<&'a mut Stamina>,
    Has<Downed>,
);

/// A drawn player's tick: their hands kept off building and tools, the lock-on, the guard,
/// the moves they ask for and run, how the walk is steered, and the clip their figure shows.
fn act(
    time: Res<Time>,
    set: Res<Set>,
    figure: Option<Res<Moves>>,
    mut players: Query<Drawing<'_>, (With<Player>, Without<partner::Partner>)>,
    partners: Query<(Entity, &Transform), With<partner::Partner>>,
    mut said: Option<ResMut<record::Said>>,
) {
    let dt = time.delta_secs();
    let set = &set.0;
    for (
        at,
        motion,
        mut intent,
        mut view,
        mut stance,
        mut fighter,
        mut drawn,
        mut acting,
        mut pool,
        downed,
    ) in &mut players
    {
        // The keys are the sword's now: building, tools and screens wait (D405).
        intent.busy = true;
        fighter.age(dt);
        let rate = fighter.rate();
        let up = at.translation.normalize();
        // Q locks on to the nearest partner in reach, or lets go.
        if intent.pressed(Action::TakeApart) {
            drawn.lock = match drawn.lock {
                Some(_) => None,
                None => partners
                    .iter()
                    .map(|(each, place)| (each, place.translation.distance(at.translation)))
                    .filter(|(_, far)| *far < LOCK_M)
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(each, _)| each),
            };
            info!(
                "Melee: lock {}",
                if drawn.lock.is_some() { "on" } else { "off" }
            );
        }
        let target = drawn
            .lock
            .and_then(|lock| partners.get(lock).ok())
            .map(|(_, place)| place.translation);
        if drawn.lock.is_some() && target.is_none() {
            drawn.lock = None;
        }
        if let Some(target) = target {
            let toward = (target - at.translation)
                .reject_from(up)
                .normalize_or_zero();
            if toward != Vec3::ZERO {
                let k = 1.0 - libm::expf(-dt * 10.0);
                view.forward = view.forward.lerp(toward, k).normalize_or(toward);
            }
        }
        let ahead = view.forward.reject_from(up).normalize_or(motion.facing);
        let right = ahead.cross(up);
        let held = |action| f32::from(intent.held(action));
        let wish = (ahead * (held(Action::Forward) - held(Action::Back))
            + right * (held(Action::Right) - held(Action::Left)))
        .normalize_or_zero();

        // Beaten: lies there until the spar starts again (`contact::again`).
        if downed || fighter.down_s.is_some() {
            stance.steer = Steer {
                drive: Some(Vec3::ZERO),
                no_roll: true,
                ..default()
            };
            show(&mut drawn, &mut acting, "Death01", 1.0, true, 0.0, None);
            continue;
        }
        let free = fighter.free();
        // The guard: up while right click is held, if not in a blow or reeling.
        let in_move = fighter.runner.name().is_some();
        let guarding = free && intent.held(Action::BuildMenu) && !in_move;
        fighter.guard_s = guarding.then(|| fighter.guard_s.map_or(0.0, |held| held + dt));
        if fighter.guard_s == Some(0.0) {
            info!("Melee: guard up");
        }
        // Moves asked: a roll, the heavy blow, the next of the light chain.
        if free {
            // A body without stamina, as in a test, never tires.
            let left = pool.as_ref().map_or(f32::MAX, |pool| pool.left);
            if let Some(name) = asked(&intent, &fighter, set) {
                let got = stamina::ask(&mut fighter.runner, set, &name, left);
                record::asked(said.as_deref_mut(), &name, got);
                match got {
                    Some(true) => begin(
                        &mut fighter,
                        &name,
                        (wish, ahead),
                        set,
                        pool.as_deref_mut(),
                        (&mut drawn, &mut acting),
                    ),
                    Some(false) => {}
                    None => info!("Melee: too tired to {name} ({left:.0} stamina)"),
                }
            }
        }
        if let Ran::Started(name) = fighter.runner.tick(set, dt * rate) {
            begin(
                &mut fighter,
                &name,
                (wish, ahead),
                set,
                pool.as_deref_mut(),
                (&mut drawn, &mut acting),
            );
        }
        let nearest = partners
            .iter()
            .map(|(_, place)| place.translation)
            .min_by(|a, b| {
                a.distance(at.translation)
                    .total_cmp(&b.distance(at.translation))
            });
        steer(
            &mut stance,
            &fighter,
            set,
            (at.translation, target, nearest),
            ahead,
            rate,
        );
        if fighter.runner.name().is_none() && fighter.stagger_s <= 0.0 {
            let pace = |clip: &str| figure.as_ref().and_then(|moves| moves.pace(clip));
            walk_clip(
                &mut drawn,
                &mut acting,
                motion,
                up,
                &set.guard,
                fighter.guard_s.and(set.get("block")),
                pace,
            );
        }
        acting.set_speed(drawn.showing.as_ref().map_or(1.0, |(_, speed)| *speed) * rate);
    }
}

/// The move a drawn player's presses ask for: a roll, the heavy blow, the next of the light
/// chain.
fn asked(intent: &Intent, fighter: &Fighter, set: &MoveSet) -> Option<String> {
    if intent.pressed(Action::Dodge) {
        Some("roll".to_string())
    } else if intent.pressed(Action::Use) && intent.held(Action::Sprint) {
        Some("heavy".to_string())
    } else if intent.pressed(Action::Use) {
        Some(fighter.runner.chained(set, "slash").to_string())
    } else {
        None
    }
}

/// A move started: which way it goes (the way asked, and ahead), its stamina paid, and its
/// clip (its chained take mixed in when it cut short a blow still carrying its shield out),
/// slower when it came tired.
fn begin(
    fighter: &mut Fighter,
    name: &str,
    (wish, ahead): (Vec3, Vec3),
    set: &MoveSet,
    pool: Option<&mut Stamina>,
    (drawn, acting): (&mut Drawn, &mut Acting),
) {
    let Some(each) = set.get(name) else { return };
    if let Some(pool) = pool
        && each.stamina > 0.0
    {
        pool.spend(each.stamina);
    }
    fighter.way = if name == "roll" {
        if wish == Vec3::ZERO { -ahead } else { wish }
    } else {
        ahead
    };
    fighter.guard_s = None;
    fighter.was = None;
    let tired = if fighter.runner.tired { " tired" } else { "" };
    info!("Melee: you {name}{tired}");
    let takes = fighter.runner.takes(set);
    let clip = takes.map_or(each.clip.as_str(), |takes| takes.clip);
    let speed = each.speed * fighter.runner.pace(set);
    drawn.showing = Some((clip.to_string(), speed));
    let mut act = Act::once(clip, each.from, speed);
    act.blend = Duration::from_millis(110);
    act.mix = takes
        .and_then(|takes| takes.mix)
        .map(|(mix, share)| (mix.to_string(), share));
    acting.play(act);
}

/// Asks the figure for a clip unless it shows it already.
fn show(
    drawn: &mut Drawn,
    acting: &mut Acting,
    clip: &str,
    speed: f32,
    once: bool,
    from_s: f32,
    blend_ms: Option<u64>,
) {
    if drawn.showing.as_ref().is_some_and(|(now, _)| now == clip) {
        if !once {
            drawn.showing = Some((clip.to_string(), speed));
        }
        return;
    }
    drawn.showing = Some((clip.to_string(), speed));
    let mut act = if once {
        Act::once(clip, from_s, speed)
    } else {
        Act::looping(clip, speed)
    };
    if let Some(ms) = blend_ms {
        act.blend = Duration::from_millis(ms);
    }
    acting.play(act);
}

/// How the walk goes this tick: a move's own drive (closing on a locked target in a blow's
/// wind-up), a shove, or the stance's pace, facing the view or the target.
/// Where the player is, the partner locked on to, and the nearest partner.
type Places = (Vec3, Option<Vec3>, Option<Vec3>);

fn steer(
    stance: &mut Stance,
    fighter: &Fighter,
    set: &MoveSet,
    (here, target, nearest): Places,
    ahead: Vec3,
    rate: f32,
) {
    let mut steer = Steer {
        face: Some(ahead),
        pace: Some(if fighter.guard_s.is_some() {
            GUARD_PACE
        } else {
            PACE
        }),
        drive: None,
        no_roll: true,
    };
    if let Some(each) = fighter.runner.now(set) {
        let t = fighter.runner.t();
        let mut speed = each.drive_at(t);
        if let (Some(warp), Some(target), Some(left)) = (each.warp, target, each.until_active(t)) {
            let to = target - here;
            let far = to.length();
            let cone = libm::cosf(warp.cone.to_radians());
            if far < warp.within && fighter.way.dot(to / far.max(1e-3)) > cone {
                speed += moves::warp_speed(far, warp.stop_at, left, warp.most_mps);
            }
        }
        // Never into the partner: a blow's drive toward them stops at arm's length.
        if let Some(nearest) = nearest.filter(|_| each.damage > 0.0) {
            let to = (nearest - here).reject_from(here.normalize());
            if to.length() < NEAREST_M && fighter.way.dot(to) > 0.0 {
                speed = speed.min(0.0);
            }
        }
        steer.drive = Some(fighter.way * speed * rate);
        steer.face = Some(fighter.way);
    } else if !fighter.free() || fighter.shove_s > 0.0 {
        steer.drive = Some(if fighter.shove_s > 0.0 {
            fighter.shove
        } else {
            Vec3::ZERO
        });
    }
    stance.steer = steer;
}

/// The clip a drawn body walks with: its guard still (the set's `guard`), a strafe to either
/// side, a walk forward or back, by which way it mostly goes; in the air, a jump. Blocking,
/// the set's `block` move, raised and held at its `hold`.
fn walk_clip(
    drawn: &mut Drawn,
    acting: &mut Acting,
    motion: &Motion,
    up: Vec3,
    guard: &str,
    blocking: Option<&moves::Move>,
    pace: impl Fn(&str) -> Option<f32>,
) {
    if let Some(block) = blocking {
        // The guard raised and held where the shield is up.
        if drawn
            .showing
            .as_ref()
            .is_none_or(|(now, _)| *now != block.clip)
        {
            drawn.showing = Some((block.clip.clone(), block.speed));
            let mut act = Act::once(&block.clip, block.from, block.speed);
            act.hold_at = block.hold;
            act.blend = Duration::from_millis(90);
            acting.play(act);
        }
        return;
    }
    if !motion.on_ground {
        show(drawn, acting, "Jump_Loop", 1.0, false, 0.0, None);
        return;
    }
    let velocity = motion.velocity.reject_from(up);
    let facing = motion.facing;
    let right = facing.cross(up);
    let (forward, side) = (velocity.dot(facing), velocity.dot(right));
    let (clip, along) = if velocity.length() < 0.2 {
        (guard, 0.0)
    } else if side.abs() > forward.abs() {
        let clip = if side > 0.0 {
            "Fight_Strafe_R"
        } else {
            "Fight_Strafe_L"
        };
        (clip, side.abs())
    } else {
        (WALK, forward)
    };
    // A stride at the pace it goes, so the feet keep to the ground; back is a walk played
    // backwards, there being no clip of its own.
    let speed = match pace(clip) {
        Some(pace) if along != 0.0 => {
            along.signum() * terra_walker::pace::rate(along.abs(), pace, 1.0)
        }
        _ => 1.0,
    };
    show(drawn, acting, clip, speed, false, 0.0, Some(180));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    /// A drawn player with so much stamina presses an action once: the move it is in after,
    /// and the stamina it has left.
    fn press(action: Action, left: f32, with: Option<Action>) -> (Option<String>, f32) {
        let (world, player) = pressed(action, left, with);
        let fighter = world.get::<Fighter>(player).unwrap();
        let name = fighter.runner.name().map(str::to_string);
        (name, world.get::<Stamina>(player).unwrap().left)
    }

    /// The world a drawn player with so much stamina pressed an action once in, and the player.
    fn pressed(action: Action, left: f32, with: Option<Action>) -> (World, Entity) {
        let mut world = World::new();
        world.init_resource::<Time>();
        let path = terra_world::data::path(SET);
        let set = MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap();
        world.insert_resource(Set(set));
        let mut intent = Intent::default();
        with.into_iter().for_each(|held| intent.hold(held));
        intent.hold(action);
        let mut pool = Stamina::default();
        pool.spend(Stamina::FULL - left);
        let player = world
            .spawn((
                Player,
                Transform::from_translation(Vec3::Y * 2000.0),
                Motion::facing(Vec3::X),
                intent,
                View {
                    forward: Vec3::X,
                    pitch: 0.0,
                },
                Stance::default(),
                Fighter::new(None),
                Drawn {
                    lock: None,
                    looks: Vec::new(),
                    showing: None,
                },
                Acting::default(),
                pool,
            ))
            .id();
        world.run_system_once(act).unwrap();
        (world, player)
    }

    #[test]
    fn a_held_block_raises_the_sets_block_clip_and_holds_it_at_its_hold() {
        let (world, player) = pressed(Action::BuildMenu, Stamina::FULL, None);
        let block = world.resource::<Set>().0.get("block").unwrap().clone();
        let drawn = world.get::<Drawn>(player).unwrap();
        assert_eq!(drawn.showing, Some((block.clip.clone(), block.speed)));
        let acting = world.get::<Acting>(player).unwrap();
        let act = acting.clip.as_ref().unwrap();
        assert_eq!(
            (act.clip.as_str(), act.hold_at),
            (block.clip.as_str(), block.hold)
        );
    }

    #[test]
    fn drawn_a_roll_and_each_blow_pay_their_cost_and_too_tired_a_roll_or_heavy_does_not_start() {
        let path = terra_world::data::path(SET);
        let set = MoveSet::read(&std::fs::read_to_string(path).unwrap()).unwrap();
        let cost = |name: &str| set.get(name).unwrap().stamina;
        let full = Stamina::FULL;
        let roll = (Some("roll".to_string()), full - cost("roll"));
        assert_eq!(press(Action::Dodge, full, None), roll);
        let slash = (Some("slash".to_string()), full - cost("slash"));
        assert_eq!(press(Action::Use, full, None), slash);
        let heavy = (Some("heavy".to_string()), full - cost("heavy"));
        assert_eq!(press(Action::Use, full, Some(Action::Sprint)), heavy);
        // Too little for the roll or the heavy blow: neither starts, nothing is spent.
        let short = cost("roll") - 1.0;
        assert_eq!(press(Action::Dodge, short, None), (None, short));
        let short = cost("heavy") - 1.0;
        assert_eq!(
            press(Action::Use, short, Some(Action::Sprint)),
            (None, short)
        );
        // A light blow still comes, tired, and takes what there is.
        assert_eq!(
            press(Action::Use, 4.0, None),
            (Some("slash".to_string()), 0.0)
        );
    }

    /// The clip a drawn body shows going so fast this way, as `walk_clip` picks it.
    fn walking(velocity: Vec3) -> String {
        let mut drawn = Drawn {
            lock: None,
            looks: Vec::new(),
            showing: None,
        };
        let mut motion = Motion::facing(Vec3::NEG_Z);
        motion.on_ground = true;
        motion.velocity = velocity;
        let mut acting = Acting::default();
        walk_clip(
            &mut drawn,
            &mut acting,
            &motion,
            Vec3::Y,
            "Keyed_Guard",
            None,
            |_| Some(1.1),
        );
        drawn.showing.unwrap().0
    }

    #[test]
    fn drawn_a_body_walks_with_the_guards_arms_and_never_the_bare_walk() {
        // Forward and back, the walk with the arms held at the guard (D458); standing, the
        // guard itself; the library's walk, its arms swinging the sword through the legs, never.
        assert_eq!(walking(Vec3::NEG_Z), WALK);
        assert_eq!(walking(Vec3::Z), WALK);
        assert_eq!(walking(Vec3::ZERO), "Keyed_Guard");
        let clips = terra_walker::model_files::json(terra_walker::figure::CLIPS);
        let clips = terra_walker::model_files::animations(&clips);
        assert!(clips.iter().any(|c| c == WALK), "fight.glb has no {WALK}");
    }
}
