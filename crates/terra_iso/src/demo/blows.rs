//! The mobs' blows on the hero (iso-forest-demo; the developer's brief: "an interesting
//! fight"): each swing of a mob's attack (mobs.rs) is told on the ground before it lands, the
//! red fan of what it reaches filling out to its rim over `TELL` before its hit (tell.rs), as
//! the ARPGs telegraph a monster's blow (Diablo IV's, PoE 2's ground warnings), and lands once on its
//! hit, judged by the sword set's own rule (terra_motion::moves' `judge`, the duel's): rolled
//! through in the roll's tuck, taken on the shield held toward it, else full on him. A blow
//! that lands or is blocked flinches him and plays the biome's hit effects at him (Cues, as
//! strike.rs's land does); the flinch is laid after his moves and taken off in `First`, as
//! strike.rs's show and settle do, so hero/ is untouched.

use super::mobs::{self, Foe, Gait};
use super::tell::{self, TellMaterial};
use crate::hero::{Doing, HeroSystems, Kit};
use crate::kit::effects::{Contact, Cues};
use crate::play::{Fallen, Hero, in_arc};
use crate::ruin::effects::hit::{FLINCH, flinch};
use bevy::prelude::*;
use terra_motion::contact::guards;
use terra_motion::moves::{MoveSet, Taken, judge};

/// Where in its attack clip a mob's blow lands, as a share of the clip: a reading (no clip
/// of the roster names its hit frame yet; a treant's slash comes down a little before half).
const HIT: f32 = 0.45;
/// How long before its hit a blow is told on the ground (s).
const TELL: f32 = 0.5;
/// How high over the mob's feet the tell lies (m): only enough to keep it off the ground's
/// own plane. It is drawn under the bodies and over the ground: depth tested, and moved
/// `tell::RAISE` toward the camera besides (`land`), so low cover stays under it.
const LIFT: f32 = 0.03;
/// A blow on his shield flinches him this share of a full one.
const ON_GUARD: f32 = 0.4;

pub fn plugin(app: &mut App) {
    app.add_plugins(tell::plugin)
        .init_resource::<Taking>()
        .add_systems(First, settle)
        .add_systems(
            Update,
            (tells, (land, show).chain())
                .chain()
                .after(HeroSystems::Act)
                .after(mobs::gait),
        );
}

/// A mob's tell: its fill and rim (tell.rs) with their materials, and the start of the last
/// swing that landed.
#[derive(Component)]
struct Tell {
    fill: (Entity, Handle<TellMaterial>),
    edge: (Entity, Handle<TellMaterial>),
    landed: f32,
    leaving: Leaving,
}

#[derive(Component)]
struct TellMark;

/// His flinch: when the last blow on him came, from where, how hard, and the tilt laid.
#[derive(Resource)]
struct Taking {
    at: f32,
    away: Vec2,
    strength: f32,
    tilt: Quat,
}

impl Default for Taking {
    fn default() -> Self {
        Taking {
            at: -10.0,
            away: Vec2::Y,
            strength: 0.0,
            tilt: Quat::IDENTITY,
        }
    }
}

/// Whether he left a swing's arc in a roll's tuck, seen each frame of the swing: a blow he
/// rolled out of is dodged, not merely out of reach. A roll drives him from its first frame,
/// so by the hit he is often past the arc, where the tuck's i-frames were never asked
/// (iso-forest-demo j11's route: rolls at 1031 to 1033 left mob 6's 1034 blow `out of reach`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Leaving {
    /// The start of the swing seen (s).
    swing: f32,
    /// He stood in its arc on the last frame seen.
    was_in: bool,
    /// He crossed out of it while invulnerable, and has not come back in.
    rolled_out: bool,
}

impl Default for Leaving {
    fn default() -> Self {
        Leaving {
            swing: f32::NEG_INFINITY,
            was_in: false,
            rolled_out: false,
        }
    }
}

impl Leaving {
    /// A frame of the swing begun at `swing`: him `inside` its arc or not, `invulnerable` or
    /// not. A new swing starts afresh from where he stands.
    fn see(&mut self, swing: f32, inside: bool, invulnerable: bool) {
        if self.swing != swing {
            *self = Leaving {
                swing,
                was_in: inside,
                rolled_out: false,
            };
            return;
        }
        if self.was_in && !inside {
            self.rolled_out = invulnerable;
        } else if inside {
            self.rolled_out = false;
        }
        self.was_in = inside;
    }
}

/// What a blow from `mob` facing `mob_face` does to the hero at `hero` facing `hero_face`,
/// doing what `doing` says with his guard up `held_s`: out of its reach and arc, dodged if he
/// `rolled_out` of it in a roll's tuck (`Leaving`) and else none; in it, the set's judgement
/// (a mob's blow is never heavy).
pub fn taken(
    (mob, mob_face): (Vec2, Vec2),
    (hero, hero_face): (Vec2, Vec2),
    doing: &Doing,
    (held_s, rolled_out): (f32, bool),
    set: &MoveSet,
) -> Option<Taken> {
    if !in_arc(mob, mob_face, hero) {
        return rolled_out.then_some(Taken::Dodged);
    }
    let from = (mob - hero).normalize_or(Vec2::Y);
    let facing = guards(
        Vec3::new(hero_face.x, 0.0, hero_face.y),
        Vec3::new(from.x, 0.0, from.y),
    );
    let guard = doing.blocking(set).then_some(held_s);
    Some(judge(
        doing.invulnerable(set),
        guard,
        facing,
        false,
        set.parry_s,
    ))
}

/// Each mob given its tell, hidden, once it stands in play: its fill and its rim, one pool
/// a mob, spawned once (no entity spawned at run time).
#[allow(clippy::type_complexity)]
fn tells(
    mut commands: Commands,
    mut materials: ResMut<Assets<TellMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mobs: Query<Entity, (With<Foe>, With<Gait>, Without<Tell>)>,
    mut shapes: Local<Option<(Handle<Mesh>, Handle<Mesh>)>>,
) {
    for body in &mobs {
        let (fill, edge) =
            shapes.get_or_insert_with(|| (meshes.add(tell::fill()), meshes.add(tell::edge())));
        let mut part = |mesh: &Handle<Mesh>, fan: tell::Fan, colour, alpha| {
            let material = materials.add(tell::material(colour, alpha));
            let id = commands
                .spawn((
                    TellMark,
                    fan,
                    Name::new("a foe's tell"),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    Visibility::Hidden,
                    bevy::light::NotShadowCaster,
                ))
                .id();
            (id, material)
        };
        let fill = part(fill, tell::FILL_FAN, tell::FILL, AlphaMode::Blend);
        let edge = part(edge, tell::EDGE_FAN, tell::EDGE, AlphaMode::Add);
        commands.entity(body).insert(Tell {
            fill,
            edge,
            landed: f32::NEG_INFINITY,
            leaving: Leaving::default(),
        });
    }
}

/// Each swing told on the ground as its hit nears, and landed on its hit, once.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn land(
    (time, frame, camera): (
        Res<Time>,
        Res<bevy::diagnostic::FrameCount>,
        Query<&GlobalTransform, With<crate::camera::MainCamera>>,
    ),
    kit: Option<Res<Kit>>,
    (mut cues, mut taking): (Option<ResMut<Cues>>, ResMut<Taking>),
    mut mobs: Query<
        (&Foe, &Gait, &Transform, &mut Tell, Has<Fallen>),
        (Without<Hero>, Without<TellMark>),
    >,
    hero: Query<(&Transform, &Doing), (With<Hero>, Without<Foe>, Without<TellMark>)>,
    mut marks: Query<
        (&mut Transform, &mut Visibility),
        (With<TellMark>, Without<Foe>, Without<Hero>),
    >,
    mut materials: ResMut<Assets<TellMaterial>>,
    mut guard_since: Local<Option<f32>>,
) {
    let (Some(kit), Ok((htr, doing))) = (kit, hero.single()) else {
        return;
    };
    let now = time.elapsed_secs();
    // The tell drawn nearer the camera than the ground it lies on (tell.rs `RAISE`).
    let raise = camera
        .single()
        .map_or(Vec3::ZERO, |c| -c.forward().as_vec3() * tell::RAISE);
    let set = &kit.set;
    let held = if doing.blocking(set) {
        now - *guard_since.get_or_insert(now)
    } else {
        *guard_since = None;
        0.0
    };
    let him = htr.translation.xz();
    let his_face = (htr.rotation * Vec3::Z).xz().normalize_or(Vec2::Y);
    for (foe, gait, tr, mut tell, fallen) in &mut mobs {
        let at = tr.translation.xz();
        let face = (tr.rotation * Vec3::Z).xz().normalize_or(Vec2::Y);
        let swing = gait.swinging(now).filter(|_| !fallen);
        let hit_at = swing.map(|(start, len)| start + len * HIT);
        if let Some((start, _)) = swing {
            let inside = in_arc(at, face, him);
            tell.leaving.see(start, inside, doing.invulnerable(set));
        }
        // The tell: up over TELL before the hit, gone at it.
        let k = hit_at
            .filter(|h| now < *h && *h - now <= TELL)
            .map(|h| 1.0 - (h - now) / TELL);
        // The fill grows out from the mob and strengthens to the hit, never past half so the
        // ground and the bodies under it show (at 0.8 it hid the mob, j7's first still); the
        // rim is whole at once (a tenth of the tell in), so the reach reads before the fill.
        let parts = [
            (
                &tell.fill,
                tell::FILL,
                k.map(|k| (0.35 + 0.65 * k, 0.12 + 0.33 * k)),
            ),
            (
                &tell.edge,
                tell::EDGE,
                k.map(|k| (1.0, (k * 10.0).min(1.0) * 0.9)),
            ),
        ];
        for ((id, material), colour, shown) in parts {
            let Ok((mut mtr, mut vis)) = marks.get_mut(*id) else {
                continue;
            };
            let want = if shown.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *vis != want {
                *vis = want;
            }
            let Some((scale, alpha)) = shown else {
                continue;
            };
            *mtr = Transform::from_translation(tr.translation + Vec3::Y * LIFT + raise)
                .with_rotation(Quat::from_rotation_y(libm::atan2f(face.x, face.y)))
                .with_scale(Vec3::splat(scale));
            if let Some(mut m) = materials.get_mut(material) {
                m.base.base_color = colour.with_alpha(alpha).into();
            }
        }
        let (Some((start, _)), Some(h)) = (swing, hit_at) else {
            continue;
        };
        if now < h || tell.landed == start {
            continue;
        }
        tell.landed = start;
        let rolled_out = tell.leaving.rolled_out;
        let Some(t) = taken((at, face), (him, his_face), doing, (held, rolled_out), set) else {
            info!(
                "frame {}: mob {}'s blow on him: out of reach (it at ({:.1}, {:.1}), he at ({:.1}, {:.1}))",
                frame.0, foe.i, at.x, at.y, him.x, him.y
            );
            continue;
        };
        // What he was doing and how far he faced from it, so a film's script is timed off the
        // log (a block that lands as Hit was not up yet, or faced away past the guard).
        let doing_now = doing.runner.current.as_ref().map_or("nothing", |(n, _)| n);
        let off = libm::acosf(
            his_face
                .dot((at - him).normalize_or(Vec2::Y))
                .clamp(-1.0, 1.0),
        );
        info!(
            "frame {}: mob {}'s blow on him: {t:?} (doing {doing_now}, facing {:.0} deg off it; \
             it at ({:.1}, {:.1}), he at ({:.1}, {:.1}), swung {:.2} s before)",
            frame.0,
            foe.i,
            off.to_degrees(),
            at.x,
            at.y,
            him.x,
            him.y,
            now - start
        );
        let strength = match t {
            Taken::Hit | Taken::GuardBroken => 1.0,
            Taken::Blocked | Taken::Parried => ON_GUARD,
            Taken::Dodged => continue,
        };
        let away = (him - at).normalize_or(Vec2::Y);
        *taking = Taking {
            at: now,
            away,
            strength,
            tilt: taking.tilt,
        };
        if let Some(c) = cues.as_deref_mut() {
            c.blow = now;
            c.contact = Some(Contact {
                point: htr.translation + Vec3::Y * 1.1 - away.extend(0.0).xzy() * 0.35,
                floor: htr.translation.y,
                toward: away,
            });
        }
    }
}

/// His flinch from the last blow, laid on his facing as it is this frame.
fn show(time: Res<Time>, mut taking: ResMut<Taking>, mut hero: Query<&mut Transform, With<Hero>>) {
    let Ok(mut tr) = hero.single_mut() else {
        return;
    };
    let f = flinch(time.elapsed_secs() - taking.at) * taking.strength;
    let axis = Vec3::Y
        .cross(taking.away.extend(0.0).xzy())
        .normalize_or(Vec3::X);
    let tilt = Quat::from_axis_angle(axis, FLINCH * f);
    tr.rotation = tilt * tr.rotation;
    taking.tilt = tilt;
}

/// The last frame's flinch taken off his facing, before anything turns him.
fn settle(mut taking: ResMut<Taking>, mut hero: Query<&mut Transform, With<Hero>>) {
    if let Ok(mut tr) = hero.single_mut() {
        tr.rotation = taking.tilt.inverse() * tr.rotation;
    }
    taking.tilt = Quat::IDENTITY;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set() -> MoveSet {
        MoveSet::read(include_str!(
            "../../../../assets/data/moves/sword.shielded.ron"
        ))
        .unwrap()
    }

    /// Him doing `name` at `clip` (a share of its clip), else nothing.
    fn doing(set: &MoveSet, name: &str, clip: f32) -> Doing {
        let mut d = Doing::default();
        let t = set.get(name).unwrap().at_clip(clip);
        d.runner.current = Some((name.to_string(), t));
        d
    }

    /// A mob 1.8 m south of him facing him; him at the origin facing `face`.
    fn blow(d: &Doing, face: Vec2, held: f32, set: &MoveSet) -> Option<Taken> {
        let mob = Vec2::new(0.0, -1.8);
        taken((mob, Vec2::Y), (Vec2::ZERO, face), d, (held, false), set)
    }

    /// A roll that carries him out of a swing's arc in its tuck dodges it: the blow lands
    /// with him past its arc and is Dodged, not out of reach. Out of it on foot, or before the
    /// tuck began, or back into it, it is not.
    #[test]
    fn a_roll_out_of_the_arc_is_a_dodge() {
        let set = set();
        let (from, to) = set.get("roll").unwrap().invulnerable.unwrap();
        let tuck = doing(&set, "roll", (from + to) / 2.0);
        let early = doing(&set, "roll", from * 0.5);
        let mob = (Vec2::new(0.0, -1.8), Vec2::Y);
        let past = (Vec2::new(4.0, 0.0), Vec2::X);
        assert!(
            !in_arc(mob.0, mob.1, past.0),
            "the hit finds him past the arc"
        );
        let ends = |frames: &[(bool, &Doing)], at_hit: &Doing| {
            let mut l = Leaving::default();
            for (inside, d) in frames {
                l.see(1.0, *inside, d.invulnerable(&set));
            }
            taken(mob, past, at_hit, (0.0, l.rolled_out), &set)
        };
        let still = Doing::default();
        assert_eq!(
            ends(&[(true, &still), (true, &early), (false, &tuck)], &tuck),
            Some(Taken::Dodged),
            "rolled out in the tuck"
        );
        assert_eq!(
            ends(&[(true, &still), (false, &still)], &still),
            None,
            "walked out"
        );
        assert_eq!(
            ends(&[(true, &still), (false, &early), (false, &tuck)], &tuck),
            None,
            "out before the tuck"
        );
        assert_eq!(
            ends(
                &[
                    (true, &still),
                    (false, &tuck),
                    (true, &tuck),
                    (false, &still)
                ],
                &still
            ),
            None,
            "back in and out on foot"
        );
        assert_eq!(ends(&[(false, &tuck)], &tuck), None, "never in it");
    }

    /// In the roll's tuck a blow passes through him; before or after it, it lands.
    #[test]
    fn a_roll_dodges_only_in_its_tuck() {
        let set = set();
        let (from, to) = set.get("roll").unwrap().invulnerable.unwrap();
        let mid = (from + to) / 2.0;
        let rolling = doing(&set, "roll", mid);
        assert_eq!(blow(&rolling, Vec2::Y, 0.0, &set), Some(Taken::Dodged));
        for late in [to + 0.05, 0.99] {
            let out = doing(&set, "roll", late);
            assert_eq!(
                blow(&out, Vec2::Y, 0.0, &set),
                Some(Taken::Hit),
                "at {late}"
            );
        }
        assert_eq!(
            blow(&Doing::default(), Vec2::Y, 0.0, &set),
            Some(Taken::Hit)
        );
    }

    /// His shield held covers a blow from in front, not one from behind; raised just as it
    /// lands it parries.
    #[test]
    fn a_block_covers_the_front_not_the_back() {
        let set = set();
        let hold = set.get("block").unwrap().hold.unwrap();
        let blocking = doing(&set, "block", hold);
        let toward = Vec2::NEG_Y;
        assert_eq!(blow(&blocking, toward, 1.0, &set), Some(Taken::Blocked));
        assert_eq!(blow(&blocking, -toward, 1.0, &set), Some(Taken::Hit));
        assert_eq!(blow(&blocking, toward, 0.0, &set), Some(Taken::Parried));
    }

    /// A blow from past its reach, or with the mob turned away, does not reach him.
    #[test]
    fn a_blow_reaches_only_in_its_arc() {
        let set = set();
        let d = Doing::default();
        assert_eq!(
            taken(
                (Vec2::new(0.0, -4.0), Vec2::Y),
                (Vec2::ZERO, Vec2::Y),
                &d,
                (0.0, false),
                &set
            ),
            None
        );
        assert_eq!(
            taken(
                (Vec2::new(0.0, -1.8), Vec2::NEG_Y),
                (Vec2::ZERO, Vec2::Y),
                &d,
                (0.0, false),
                &set
            ),
            None
        );
    }
}
