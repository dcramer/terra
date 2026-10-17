//! Meetings: a group put out ahead of a player on the move, where the camera will see it (D384).

use super::*;

/// A player's way as the meetings follow it: where they were at the last look, how long until
/// they next meet something, and the meeting being staged, if one is.
pub(super) struct Way {
    last: Vec3,
    left: f32,
    staging: Option<Staging>,
    /// Whether they were on the move at the last look, and how many meetings their walks have
    /// had.
    moving: bool,
    met: u32,
}

/// How far ahead of a player and aside from their way a meeting's spot is, metres.
#[derive(Clone, Copy)]
pub(super) struct Band {
    pub(super) ahead_m: (f32, f32),
    pub(super) aside_m: (f32, f32),
}

/// Where the camera is when its player stands at a point, going a way.
pub(super) fn eye_at(around: &Surroundings, at: Vec3, ahead: Vec3) -> Vec3 {
    let up = at.normalize_or(Vec3::Y);
    around.ground(at) + up * EYE_UP_M - ahead * EYE_BEHIND_M
}

/// Whether an animal standing at a point shows from an eye: its middle or either end in view,
/// as one half behind a trunk is still seen (`in_view` holds a line hidden by anything on it).
pub(super) fn shows_from(around: &Surroundings, eye: Vec3, at: Vec3) -> bool {
    let up = at.normalize_or(Vec3::Y);
    let across = (at - eye).cross(up).normalize_or_zero() * BODY_SPAN_M;
    [at, at + across, at - across]
        .into_iter()
        .any(|point| around.in_view(eye, point))
}

/// How far on along their way a player coming on is when a spot so far ahead of them along
/// it and so far aside is seen at each angle off the camera's way (`SEEN_OFF`), radians:
/// none behind them.
fn eyes_on(along_m: f32, aside_m: f32) -> [f32; 2] {
    SEEN_OFF.map(|off| (along_m + EYE_BEHIND_M - aside_m / libm::tanf(off)).max(0.0))
}

/// From how many of the eyes a player coming on sees a spot ahead and aside through as it
/// passes through the open third of the picture it is in view (`eyes_on`): each eye a
/// millisecond. Where they are now does not count: one put out unseen comes into view.
fn seen_coming_on(around: &Surroundings, here: Vec3, ahead: Vec3, spot: Vec3) -> usize {
    let up = here.normalize_or(Vec3::Y);
    let off = (spot - here).reject_from(up);
    let along = off.dot(ahead);
    let aside = (off - ahead * along).length();
    let at = spot + up * 0.4;
    eyes_on(along, aside)
        .iter()
        .filter(|&&on| shows_from(around, eye_at(around, here + ahead * on, ahead), at))
        .count()
}

/// A meeting being staged: a spot tried each look (each costs a millisecond or two, the ground
/// and what grows on it asked), the best of each kind so far, and the dice it is rolled with.
struct Staging {
    dice: terra_gen::Rng,
    tried: usize,
    /// Whether it is with a big animal (`BIG_EVERY`).
    big: bool,
    /// For each kind that fits, its best spot, the way aside from the player there, and how
    /// well it shows: an index into the kinds about.
    fits: Vec<(Vec3, Vec3, usize, f32, bool)>,
}

/// Puts a group out ahead of each player on the move, now and then: at the spot beside their
/// way that shows it best, of its niche, where none of its kind already is.
pub(super) fn meet(
    commands: &mut Commands,
    cells: &mut Cells,
    (kinds, bestiary): (&Kinds, &Bestiary),
    around: &Surroundings,
    wild: &[Wild],
    moving: &[(Entity, Vec3, Vec3)],
    placed: &Query<(&Transform, &creatures::Creature)>,
) {
    let seed = around.planet.seed().derive("wildlife meetings");
    cells
        .ways
        .retain(|player, _| moving.iter().any(|(each, ..)| each == player));
    for &(player, here, facing) in moving {
        let mut way = cells.ways.remove(&player).unwrap_or(Way {
            last: here,
            left: 1.0,
            staging: None,
            moving: false,
            met: 0,
        });
        let up = here.normalize_or(Vec3::Y);
        let went = (here - way.last).reject_from(up);
        way.last = here;
        let speed = went.length() / LOOK_EVERY_S;
        // On the move, ahead is the way they go; standing, the way they face, where a few
        // are met too, so a walk does not begin on an empty land.
        let moving = speed > MOVING_MPS;
        let ahead = if moving {
            went.normalize_or_zero()
        } else {
            facing.reject_from(up).normalize_or_zero()
        };
        let most = if moving { MET_MOST } else { MET_STANDING };
        if moving && !way.moving {
            // Setting off: what was staged standing is for where they faced.
            way.left = way.left.min(SET_OFF_S);
            way.staging = None;
        }
        way.moving = moving;
        if ahead == Vec3::ZERO || (!moving && cells.met.len() >= most) {
            cells.ways.insert(player, way);
            continue;
        }
        way.left -= LOOK_EVERY_S;
        if way.left <= 0.0 && way.staging.is_none() && cells.met.len() < most {
            cells.meetings += 1;
            let mut dice = seed.index(cells.meetings).rng();
            way.left = dice.range(MEET_EVERY_S.0, MEET_EVERY_S.1);
            way.staging = Some(Staging {
                dice,
                tried: 0,
                big: moving && way.met.is_multiple_of(BIG_EVERY),
                fits: Vec::new(),
            });
        }
        let Some(mut staging) = way.staging.take() else {
            cells.ways.insert(player, way);
            continue;
        };
        try_spots(&mut staging, around, wild, placed, (here, ahead));
        if staging.tried < MEET_TRIES * 2 * SPOTS_A_LOOK {
            way.staging = Some(staging);
            cells.ways.insert(player, way);
            continue;
        }
        let chosen = choose(&mut staging, bestiary, wild);
        let dice = &mut staging.dice;
        let met = 'met: {
            let Some((spot, aside, index, _, in_view)) = chosen else {
                break 'met false;
            };
            let (species, _, (fewest, most), _) = wild[index];
            // Met as a group of the larger sort its kind keeps.
            let count = (fewest + most).div_ceil(2)
                + (dice.unit() * ((most - fewest) / 2 + 1) as f32) as u32;
            // One standing has nothing to cross ahead of.
            let share = if staging.big {
                BIG_CROSSINGS
            } else {
                CROSSINGS
            };
            let crossing = if moving && (!in_view || dice.unit() < share) {
                cross(
                    commands,
                    (kinds, bestiary),
                    around,
                    species,
                    (here, ahead, aside, speed),
                    count.min(most),
                    dice,
                )
            } else {
                None
            };
            let crosses = crossing.is_some();
            // A big animal's spot out of view stood only for its kind, for a crossing.
            let group = match crossing {
                Some(group) => group,
                None if in_view => put_group(
                    commands,
                    (kinds, bestiary),
                    around,
                    species,
                    (spot, aside * 2.0),
                    count.min(most),
                    dice,
                ),
                None => break 'met false,
            };
            let group_len = group.len();
            cells.met.extend(group);
            info!(
                "Wildlife: {} {} met ahead, {}.",
                group_len,
                bestiary.name(species),
                if crosses {
                    "crossing the way"
                } else {
                    "beside it"
                }
            );
            true
        };
        if met {
            way.met += moving as u32;
        } else {
            // Nowhere in view: try again at once, a little further on.
            debug!(
                "Wildlife: of {} spots none in view to meet at, nor a crossing.",
                staging.tried
            );
            way.left = way.left.min(LOOK_EVERY_S);
        }
        cells.ways.insert(player, way);
    }
}

/// Tries a look's spots for a meeting being staged, keeping each kind's best.
fn try_spots(
    staging: &mut Staging,
    around: &Surroundings,
    wild: &[Wild],
    placed: &Query<(&Transform, &creatures::Creature)>,
    (here, ahead): (Vec3, Vec3),
) {
    let up = here.normalize_or(Vec3::Y);
    // A spot one side of the way and then the other, with the kinds whose niche it is and
    // that are not about there already, weighed by how well it shows them: in view, near
    // the way, in the light, on bare or short ground. A kind already met near is less
    // likely again, so a walk by a wood and a lake meets both.
    for _ in 0..SPOTS_A_LOOK {
        let aside = ahead.cross(up)
            * if staging.tried.is_multiple_of(2) {
                1.0
            } else {
                -1.0
            };
        staging.tried += 1;
        let band = if staging.big { MEET_BIG } else { MEET_SMALL };
        let dice = &mut staging.dice;
        let off = dice.range(band.aside_m.0, band.aside_m.1);
        let spot =
            around.ground(here + ahead * dice.range(band.ahead_m.0, band.ahead_m.1) + aside * off);
        let eyes = seen_coming_on(around, here, ahead, spot);
        if eyes > 0 || staging.big {
            let place = around.place(spot);
            // In a patch of sunlight most of all, as a shaft through the trees picks one out.
            let lit = if around.sunlit(spot + up * 0.3) {
                SUNLIT
            } else {
                1.0
            };
            let shows = lit
            * (1.0 - 0.75 * place.shade.clamp(0.0, 1.0))
            * if place.bare { 1.5 } else { 0.6 }
            * (1.0 - place.grass_m / HIDING_GRASS_M).clamp(0.1, 1.0)
            // Seen all the way through the picture far the best.
            * [0.01, 1.0, 4.0][eyes.min(SEEN_OFF.len())];
            for (index, &(species, per_ha, _, niche)) in wild.iter().enumerate() {
                if !place.fits(niche) {
                    continue;
                }
                let (mut close, mut seen) = (false, 0);
                for (at, creature) in placed {
                    if creature.species == species {
                        let apart = at.translation.distance(spot);
                        close |= apart < MEET_APART_M;
                        seen += (apart < MEET_KNOWN_M) as u32;
                    }
                }
                if close {
                    continue;
                }
                // The best shown of a kind's spots stands for it. A kind met near already is
                // much the less likely again, so a walk meets one of each before a second.
                let again = 1.0 + seen as f32;
                let weight = per_ha.sqrt() / (again * again) * shows * shows;
                match staging.fits.iter_mut().find(|fit| fit.2 == index) {
                    Some(best) if best.3 < weight => *best = (spot, aside, index, weight, eyes > 0),
                    Some(_) => {}
                    None => staging.fits.push((spot, aside, index, weight, eyes > 0)),
                }
            }
        }
    }
}

/// Which of the kinds that fit a meeting being staged is met, and where, rolled by how well each shows.
fn choose(
    staging: &mut Staging,
    bestiary: &Bestiary,
    wild: &[Wild],
) -> Option<(Vec3, Vec3, usize, f32, bool)> {
    // A big animal's meeting is with one of the big kinds that fit, and the others' with
    // the small, where there are any of that size.
    let big =
        |fit: &(Vec3, Vec3, usize, f32, bool)| bestiary.get(wild[fit.2].0).body.radius >= BIG_M;
    if staging.fits.iter().any(|fit| big(fit) == staging.big) {
        let wanted = staging.big;
        staging.fits.retain(|fit| big(fit) == wanted);
    }
    let dice = &mut staging.dice;
    let total: f32 = staging.fits.iter().map(|fit| fit.3).sum();
    let mut roll = dice.unit() * total;
    staging.fits.iter().copied().find(|fit| {
        roll -= fit.3;
        roll <= 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_meeting_is_seen_off_the_way_ahead_of_the_player_never_behind_them() {
        for band in [MEET_BIG, MEET_SMALL] {
            for along in [band.ahead_m.0, band.ahead_m.1] {
                for aside in [band.aside_m.0, band.aside_m.1] {
                    for on in eyes_on(along, aside) {
                        // From the camera, behind the player once they have come so far on.
                        let ahead_of_eye = along - on + EYE_BEHIND_M;
                        let off = libm::atanf(aside / ahead_of_eye).to_degrees();
                        assert!((15.0..=30.0).contains(&off), "{along} {aside}: {off}°");
                        // Still ahead of the player, not behind their back.
                        assert!(along - on > 1.0, "{along} {aside}: {}", along - on);
                    }
                }
            }
        }
    }
}
