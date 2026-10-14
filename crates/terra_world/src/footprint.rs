//! Where something built stands on the ground, as the world sees it: the places on its grid
//! it covers, its door steps among them. What grows keeps out of it (`flora`), and whatever
//! builds it keeps it up to date, so the world never needs to know how building works.

use bevy::{platform::collections::HashSet, prelude::*};

/// The ground under something built, in its own space: `frame` places it on the planet,
/// with the grid's x and z along the ground, `cell_m` apart.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Footprint {
    pub frame: Transform,
    pub cell_m: f32,
    /// Places in a room, or with a floor, on the ground storey.
    pub ground: HashSet<IVec2>,
    /// Places where anything stands, a lone wall as much as a room.
    pub standing: HashSet<IVec2>,
}

/// Further than this from a footprint's origin, a point is on none of it.
const FAR_M: f32 = 100_000.0;

impl Footprint {
    /// Whether a point on the planet is on the footprint, or within a margin of it: in a
    /// room or on a floor, or within the margin of anything standing, door steps too.
    /// Every place in reach is looked at, so a wall one place thick is never missed.
    pub fn covers(&self, point: Vec3, margin: f32) -> bool {
        self.covers_local(self.to_local().transform_point3(point), margin)
    }

    /// What takes a point on the planet into its own space, worked out once for many.
    pub fn to_local(&self) -> bevy::math::Affine3A {
        self.frame.compute_affine().inverse()
    }

    /// As [`Footprint::covers`], for a point already in its own space.
    pub fn covers_local(&self, local: Vec3, margin: f32) -> bool {
        // Nowhere near it, or no point at all, as ground searched for under a dug cliff
        // can give: not on it, and no sum below overflows.
        if !local.is_finite() || local.abs().max_element() > FAR_M {
            return false;
        }
        let at = (local / self.cell_m).floor().as_ivec3();
        let reach = (margin / self.cell_m).ceil() as i32;
        self.ground.contains(&IVec2::new(at.x, at.z))
            || (-reach..=reach).any(|dx| {
                (-reach..=reach).any(|dz| self.standing.contains(&IVec2::new(at.x + dx, at.z + dz)))
            })
    }

    /// Its places, rooms and floors and whatever stands.
    pub fn places(&self) -> impl Iterator<Item = IVec2> + '_ {
        self.ground.iter().chain(&self.standing).copied()
    }

    /// What changed from `before` to it, as footprints whose places are those changed,
    /// each in its own frame: the places in one and not the other, or, moved or rescaled,
    /// all of both. Nothing changed gives nothing.
    pub fn changes_from(&self, before: Option<&Footprint>) -> Vec<Footprint> {
        let moved = |before: &Footprint| before.frame != self.frame || before.cell_m != self.cell_m;
        match before {
            Some(before) if !moved(before) => {
                let changed = Footprint {
                    ground: self
                        .ground
                        .symmetric_difference(&before.ground)
                        .copied()
                        .collect(),
                    standing: self
                        .standing
                        .symmetric_difference(&before.standing)
                        .copied()
                        .collect(),
                    ..self.clone()
                };
                if changed.ground.is_empty() && changed.standing.is_empty() {
                    Vec::new()
                } else {
                    vec![changed]
                }
            }
            Some(before) => vec![before.clone(), self.clone()],
            None => vec![self.clone()],
        }
    }

    /// The box its places span in its own space, in metres, from low to high corner on
    /// the ground (x and z), if it has any.
    pub fn bounds(&self) -> Option<(Vec2, Vec2)> {
        let mut places = self.places();
        let first = places.next()?;
        let (low, high) = places.fold((first, first), |(l, h), p| (l.min(p), h.max(p)));
        Some((
            low.as_vec2() * self.cell_m,
            (high + IVec2::ONE).as_vec2() * self.cell_m,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_footprint_covers_its_rooms_and_the_reach_of_what_stands() {
        // Somewhere on a planet, its up the ground's and turned about it, as a base is.
        let up = Vec3::new(0.3, 0.9, 0.5).normalize();
        let frame = Transform::from_translation(up * 2000.0)
            .with_rotation(Quat::from_rotation_arc(Vec3::Y, up) * Quat::from_rotation_y(0.7));
        let footprint = Footprint {
            frame,
            cell_m: 0.5,
            ground: [IVec2::new(0, 0)].into_iter().collect(),
            standing: [IVec2::new(4, 0)].into_iter().collect(),
        };
        // A place on the planet, given in the footprint's own space.
        let at = |x: f32, z: f32| frame.transform_point(Vec3::new(x, 0.0, z));
        assert!(footprint.covers(at(0.25, 0.25), 0.0));
        assert!(!footprint.covers(at(1.25, 0.25), 0.0));
        // A wall at x 2 to 2.5 m: covered within half a metre of it, not beyond.
        assert!(footprint.covers(at(1.75, 0.25), 0.5));
        assert!(!footprint.covers(at(1.25, 0.25), 0.5));
    }

    #[test]
    fn what_changed_is_the_places_taken_or_given_up_or_all_of_it_moved() {
        let place = |x: i32| IVec2::new(x, 0);
        let before = Footprint {
            cell_m: 0.5,
            ground: [place(0), place(1)].into_iter().collect(),
            standing: [place(5)].into_iter().collect(),
            ..default()
        };
        let mut after = before.clone();
        after.ground.remove(&place(1));
        after.standing.insert(place(9));
        let changed = after.changes_from(Some(&before));
        assert_eq!(changed.len(), 1);
        let mut places: Vec<i32> = changed[0].places().map(|p| p.x).collect();
        places.sort();
        assert_eq!(places, vec![1, 9]);
        assert!(before.changes_from(Some(&before)).is_empty());
        // Moved, all of both; new, all of it.
        let mut moved = before.clone();
        moved.frame.translation.x += 3.0;
        assert_eq!(
            moved.changes_from(Some(&before)),
            vec![before.clone(), moved.clone()]
        );
        assert_eq!(before.changes_from(None), vec![before.clone()]);
    }
}
