//! The rooms behind a facade's glass (iso-kit j37, D634): each bay of a front a room from
//! behind the modules' reveals back to the block's set-back face (`Bays::back`), floored,
//! ceiled and walled from the next, so a window shows a room's floor and furniture below its
//! sill, moving against the frame as the eye does, not a flat glowing card. What a room
//! holds is a hash of its front, column and storey (`terrain::hash`), the same each run: lit
//! in one of the front's colours at its own brightness and warmth; of the lit ones above the
//! street, some lit only by a screen's cold glow, some with blinds or curtains part drawn;
//! of the dark ones, some behind a closed roller shutter. A lit room's light is brightest on
//! its back wall, less on its floor, sides and ceiling, and a piece of furniture stands dark
//! in it. How games fake interiors behind glass: real boxes for a few, interior mapping
//! (Joost van Dongen, "Interior Mapping", CGI 2008; Marvel's Spider-Man's city) for many;
//! a street's hundred-odd rooms are boxes.

use super::facade::Bays;
use super::light::Colour;
use super::street::Side;
use bevy::prelude::*;
use serde::Deserialize;
use std::f32::consts::FRAC_PI_2;

/// How deep behind the modules' face a room starts: clear of their reveals (to 0.24 m).
pub const FRONT: f32 = 0.27;

/// The shares of a front's rooms' kinds: of the lit ones above the street storey, `tv` lit
/// only by a `screen`, `blinds` and `curtains` (in one of `cloth`) part drawn; of the dark
/// ones, `shut` behind a roller shutter (in one of `shutters`).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Rooms {
    #[serde(default)]
    pub tv: f32,
    #[serde(default)]
    pub blinds: f32,
    #[serde(default)]
    pub curtains: f32,
    #[serde(default)]
    pub shut: f32,
    #[serde(default = "screen")]
    pub screen: Colour,
    #[serde(default)]
    pub cloth: Vec<Colour>,
    #[serde(default)]
    pub shutters: Vec<Colour>,
    /// A lit shop's light spilling out of its window onto the pavement: a spot of `spill.0`
    /// lumens (at the room's brightness; half behind blinds or curtains) from the glass's
    /// head down to `spill.1` metres out, in the room's colour, unshadowed (the ARPGs' and
    /// city games' pool before a lit shop front). Only from the street storey's bays whose
    /// `ground` scene is one of `windows`, so a plain wall or a door spills nothing.
    #[serde(default)]
    pub spill: (f32, f32),
    #[serde(default)]
    pub windows: Vec<usize>,
}

fn screen() -> Colour {
    Colour::Srgb(0.45, 0.62, 1.0)
}

/// What a room holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Kind {
    Dark,
    Shut,
    Lit,
    Tv,
    Blinds,
    Curtains,
}

/// A room: its kind, which of the front's colours lights it, how bright (0.6 to 1.2 of its
/// storey's nits) and warm (0 to 1), how far its blinds or curtains are drawn (0.3 to 0.9),
/// and a pick (0 to 1) for its cloth, shutter and furniture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Room {
    pub kind: Kind,
    pub colour: usize,
    pub bright: f32,
    pub warm: f32,
    pub drawn: f32,
    pub pick: f32,
}

/// The room behind column `column`'s bay in storey `storey` of the front `along`.
pub fn room(b: &Bays, r: &Rooms, along: (f32, f32), column: usize, storey: usize) -> Room {
    let seed = along.0.to_bits() ^ 0x700_3e55;
    let h = |k: u32| {
        crate::terrain::hash(
            column as i32,
            storey as i32,
            seed.wrapping_add(k.wrapping_mul(0x9e37_79b9)),
        )
    };
    let share = if storey == 0 { b.shops.0 } else { b.lit };
    let up = storey > 0;
    let roll = h(2);
    let kind = if h(1) < share && !b.colours.is_empty() {
        if up && roll < r.tv {
            Kind::Tv
        } else if up && roll < r.tv + r.blinds {
            Kind::Blinds
        } else if up && roll < r.tv + r.blinds + r.curtains && !r.cloth.is_empty() {
            Kind::Curtains
        } else {
            Kind::Lit
        }
    } else if roll < r.shut && !r.shutters.is_empty() {
        Kind::Shut
    } else {
        Kind::Dark
    };
    let n = b.colours.len().max(1);
    Room {
        kind,
        colour: (h(3) * n as f32) as usize % n,
        bright: 0.6 + 0.6 * h(4),
        warm: h(7),
        drawn: 0.3 + 0.6 * h(5),
        pick: h(6),
    }
}

fn glow(
    materials: &mut Assets<StandardMaterial>,
    base: f32,
    light: LinearRgba,
) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::srgb(base, base, base),
        emissive: light,
        perceptual_roughness: 0.9,
        ..default()
    })
}

/// Two colours multiplied channel by channel.
fn times(a: LinearRgba, b: LinearRgba) -> LinearRgba {
    LinearRgba::rgb(a.red * b.red, a.green * b.green, a.blue * b.blue)
}

/// What every room of a front shares: its meshes, the dark of an unlit room, which way
/// its face looks, its wall line and its bays.
struct Front<'a> {
    quad: Handle<Mesh>,
    cube: Handle<Mesh>,
    dark: Handle<StandardMaterial>,
    turn: Quat,
    s: f32,
    wall: f32,
    b: &'a Bays,
    r: &'a Rooms,
}

/// One room's place: its bay's middle `x` and width `w`, its floor `y0`, its kind and its
/// light (none when unlit).
struct Bay {
    x: f32,
    w: f32,
    y0: f32,
    nits: f32,
    rm: Room,
    light: Option<LinearRgba>,
}

impl Front<'_> {
    fn put(&self, commands: &mut Commands, m: Handle<StandardMaterial>, cube: bool, t: Transform) {
        let mesh = if cube { &self.cube } else { &self.quad };
        commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(m), t));
    }

    /// The glass's plane, just behind the modules' reveals, and the room's middle in depth.
    fn glass(&self) -> f32 {
        self.s * (self.wall + FRONT)
    }

    /// The room's box: floor, ceiling and the walls either side, lit at their shares of its
    /// light; a lit room's back wall before the block's dark face, a piece of furniture
    /// (a table, a sofa or a bed) dark against its floor by the back wall.
    fn shell(&self, commands: &mut Commands, materials: &mut Assets<StandardMaterial>, y: &Bay) {
        let (b, h) = (self.b, self.b.storey);
        let deep = b.back - FRONT;
        let mid = self.s * (self.wall + FRONT + deep / 2.0);
        let mut lit = |share: f32| match y.light {
            Some(l) => glow(materials, 0.03, l * share),
            None => self.dark.clone(),
        };
        let floor = lit(0.5);
        let ceiling = lit(0.3);
        let sides = lit(0.4);
        let back = lit(1.0);
        self.put(
            commands,
            floor,
            false,
            Transform::from_xyz(y.x, y.y0 + 0.02, mid)
                .with_rotation(Quat::from_rotation_x(-FRAC_PI_2))
                .with_scale(Vec3::new(y.w, deep, 1.0)),
        );
        self.put(
            commands,
            ceiling,
            false,
            Transform::from_xyz(y.x, y.y0 + h - 0.02, mid)
                .with_rotation(Quat::from_rotation_x(FRAC_PI_2))
                .with_scale(Vec3::new(y.w, deep, 1.0)),
        );
        for (dx, a) in [(-1.0, FRAC_PI_2), (1.0, -FRAC_PI_2)] {
            self.put(
                commands,
                sides.clone(),
                false,
                Transform::from_xyz(y.x + dx * (y.w / 2.0 - 0.01), y.y0 + h / 2.0, mid)
                    .with_rotation(Quat::from_rotation_y(a))
                    .with_scale(Vec3::new(deep, h, 1.0)),
            );
        }
        let Some(l) = y.light else { return };
        self.put(
            commands,
            back,
            false,
            Transform::from_xyz(y.x, y.y0 + h / 2.0, self.s * (self.wall + b.back - 0.02))
                .with_rotation(self.turn)
                .with_scale(Vec3::new(y.w, h, 1.0)),
        );
        let (fw, fh, fd) =
            [(1.0, 0.75, 0.6), (1.8, 0.8, 0.85), (1.5, 0.5, 1.4)][(y.rm.pick * 3.0) as usize % 3];
        let fx = y.x + (y.rm.warm - 0.5) * (y.w - fw - 0.2).max(0.0);
        let fz = self.s * (self.wall + b.back - 0.05 - fd / 2.0);
        let wood = glow(materials, 0.04, l * 0.04);
        self.put(
            commands,
            wood,
            true,
            Transform::from_xyz(fx, y.y0 + fh / 2.0, fz)
                .with_rotation(self.turn)
                .with_scale(Vec3::new(fw, fh, fd)),
        );
    }

    /// What stands in the room by its kind: a screen, blinds, curtains or a shutter.
    fn dress(&self, commands: &mut Commands, materials: &mut Assets<StandardMaterial>, y: &Bay) {
        let (r, h, glass) = (self.r, self.b.storey, self.glass());
        let l = y.light.unwrap_or(LinearRgba::BLACK);
        let at = |x: f32, up: f32, z: f32, size: (f32, f32)| {
            Transform::from_xyz(x, up, z)
                .with_rotation(self.turn)
                .with_scale(Vec3::new(size.0, size.1, 1.0))
        };
        match y.rm.kind {
            Kind::Tv => {
                // The screen low on the back wall, its cold light the room's only one.
                let m = glow(materials, 0.0, r.screen.color().to_linear() * y.nits * 1.5);
                let z = self.s * (self.wall + self.b.back - 0.04);
                let x = y.x + (y.rm.pick - 0.5) * y.w * 0.4;
                self.put(commands, m, false, at(x, y.y0 + 0.85, z, (0.9, 0.55)));
            }
            Kind::Blinds => {
                // Slats down from the head, lit from behind, the room between them.
                let m = glow(
                    materials,
                    0.35,
                    times(l * 0.45, LinearRgba::rgb(1.0, 0.93, 0.8)),
                );
                let n = (y.rm.drawn * 1.9 / 0.08) as usize;
                for i in 0..n {
                    let up = y.y0 + 2.62 - i as f32 * 0.08;
                    self.put(
                        commands,
                        m.clone(),
                        false,
                        at(y.x, up, glass, (y.w * 0.96, 0.055)),
                    );
                }
            }
            Kind::Curtains => {
                // A curtain each side, part drawn, its folds lit unevenly from behind.
                let k = (y.rm.pick * r.cloth.len() as f32) as usize % r.cloth.len().max(1);
                let cloth = r.cloth[k].color().to_linear();
                let fold = [
                    glow(materials, 0.1, times(l * 0.3, cloth)),
                    glow(materials, 0.06, times(l * 0.16, cloth)),
                ];
                let strip = y.w * (0.12 + 0.3 * y.rm.drawn) / 4.0;
                for dir in [-1.0, 1.0] {
                    for (i, m) in fold.iter().cycle().take(4).enumerate() {
                        let x = y.x + dir * (y.w / 2.0 - (i as f32 + 0.5) * strip);
                        self.put(
                            commands,
                            m.clone(),
                            false,
                            at(x, y.y0 + 1.65, glass, (strip, 2.1)),
                        );
                    }
                }
            }
            Kind::Shut => {
                // A roller shutter down behind the glass, its ribs darker.
                let k = (y.rm.pick * r.shutters.len() as f32) as usize % r.shutters.len().max(1);
                let paint = r.shutters[k].color().to_linear();
                let steel = |k: f32| StandardMaterial {
                    base_color: Color::LinearRgba(paint * k),
                    perceptual_roughness: 0.55,
                    metallic: 0.4,
                    ..default()
                };
                let (face, rib) = (materials.add(steel(1.0)), materials.add(steel(0.45)));
                let size = (y.w * 0.96, h * 0.94);
                self.put(commands, face, false, at(y.x, y.y0 + h / 2.0, glass, size));
                for i in 0..(size.1 / 0.12) as usize {
                    let up = y.y0 + h * 0.03 + 0.06 + i as f32 * 0.12;
                    let z = glass + self.s * -0.005;
                    self.put(commands, rib.clone(), false, at(y.x, up, z, (size.0, 0.03)));
                }
            }
            Kind::Lit | Kind::Dark => {}
        }
    }
}

/// Every bay's room on a front on `side`, its modules' face on the wall line `wall`: `cols`
/// as `facade::columns` lays them, `storeys` up.
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    side: Side,
    along: (f32, f32),
    cols: &[(f32, f32)],
    storeys: usize,
    wall: f32,
    b: &Bays,
    r: &Rooms,
) {
    let front = Front {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        dark: glow(materials, 0.015, LinearRgba::BLACK),
        turn: Quat::from_rotation_y(side.facing()),
        s: side.z(),
        wall,
        b,
        r,
    };
    for (c, &(x, k)) in cols.iter().enumerate() {
        for st in 0..storeys {
            let rm = room(b, r, along, c, st);
            let nits = if st == 0 { b.shops.1 } else { b.nits };
            let mut tone = b
                .colours
                .get(rm.colour)
                .map_or(LinearRgba::BLACK, |c| c.color().to_linear());
            tone.green *= 0.9 + 0.2 * rm.warm;
            tone.blue *= 0.8 + 0.4 * rm.warm;
            let light = match rm.kind {
                Kind::Lit | Kind::Blinds | Kind::Curtains => Some(tone * nits * rm.bright),
                Kind::Tv => Some(r.screen.color().to_linear() * nits * 0.1),
                Kind::Dark | Kind::Shut => None,
            };
            let bay = Bay {
                x,
                w: b.width * k,
                y0: st as f32 * b.storey,
                nits,
                rm,
                light,
            };
            front.shell(commands, materials, &bay);
            front.dress(commands, materials, &bay);
            if let Some((at, foot, lumens, colour)) = spill(b, r, c, st, x, wall, side, &bay) {
                commands.spawn((
                    SpotLight {
                        color: colour,
                        intensity: lumens,
                        range: (at - foot).length() + 2.5,
                        radius: 0.4,
                        inner_angle: 25f32.to_radians(),
                        outer_angle: 55f32.to_radians(),
                        shadow_maps_enabled: false,
                        ..default()
                    },
                    Transform::from_translation(at).looking_at(foot, Vec3::X),
                ));
            }
        }
    }
}

/// A street-storey shop's spill (`Rooms::spill`): where its spot sits (at the glass's head,
/// just out of the face) and aims (on the pavement `spill.1` out), its lumens and colour;
/// none for a dark, shut or screen-lit room, an upper storey or a bay with no window.
#[allow(clippy::too_many_arguments)]
fn spill(
    b: &Bays,
    r: &Rooms,
    column: usize,
    storey: usize,
    x: f32,
    wall: f32,
    side: Side,
    bay: &Bay,
) -> Option<(Vec3, Vec3, f32, Color)> {
    let scene = b.ground.get(column % b.ground.len().max(1))?;
    if storey != 0 || r.spill.0 <= 0.0 || !r.windows.contains(scene) {
        return None;
    }
    let share = match bay.rm.kind {
        Kind::Lit => 1.0,
        Kind::Blinds | Kind::Curtains => 0.5,
        _ => return None,
    };
    let l = bay.light?;
    let top = l.red.max(l.green).max(l.blue).max(1e-6);
    let colour = Color::linear_rgb(l.red / top, l.green / top, l.blue / top);
    let s = side.z();
    let at = Vec3::new(x, b.storey * 0.8, s * (wall - 0.3));
    let foot = Vec3::new(x, 0.0, s * (wall - r.spill.1));
    Some((at, foot, r.spill.0 * share * bay.rm.bright, colour))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn front() -> Bays {
        crate::kit::parse(
            "(model: \"m.glb\", ground: [0], upper: [4], top: 8, pier: 9, back: 2.0, \
             lit: 0.4, shops: (0.8, 8.0), nits: 6.0, \
             colours: [Srgb(1.0, 0.7, 0.4), Srgb(0.6, 0.8, 1.0)], \
             rooms: (tv: 0.2, blinds: 0.25, curtains: 0.2, shut: 0.3, \
                     cloth: [Srgb(0.6, 0.2, 0.2)], shutters: [Srgb(0.4, 0.45, 0.4)]))",
            "test",
            "facade.rs",
        )
        .unwrap()
    }

    /// A front's rooms take every kind, the same each run, each kind near its share: the
    /// lit ones near `lit` of the rooms above the street, none of those kinds the street
    /// storey's; a front with no colours lights none.
    #[test]
    fn rooms_vary_by_hash_and_repeat() {
        let b = front();
        let r = b.rooms.clone().unwrap();
        let along = (6.0, 35.0);
        let all: Vec<Room> = (0..10)
            .flat_map(|c| (0..7).map(move |st| (c, st)))
            .map(|(c, st)| room(&b, &r, along, c, st))
            .collect();
        let again: Vec<Room> = (0..10)
            .flat_map(|c| (0..7).map(move |st| (c, st)))
            .map(|(c, st)| room(&b, &r, along, c, st))
            .collect();
        assert_eq!(all, again);
        let mut kinds: Vec<Kind> = all.iter().map(|m| m.kind).collect();
        kinds.sort();
        kinds.dedup();
        assert_eq!(kinds.len(), 6, "seventy rooms took only {kinds:?}");
        let up: Vec<&Room> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 7 != 0)
            .map(|(_, m)| m)
            .collect();
        let lit = up
            .iter()
            .filter(|m| !matches!(m.kind, Kind::Dark | Kind::Shut))
            .count() as f32
            / up.len() as f32;
        assert!(
            (0.2..0.6).contains(&lit),
            "{lit} of the upper rooms lit, not near 0.4"
        );
        for (i, m) in all.iter().enumerate() {
            if i % 7 == 0 {
                assert!(
                    matches!(m.kind, Kind::Dark | Kind::Shut | Kind::Lit),
                    "{m:?}"
                );
            }
            assert!((0.6..=1.2).contains(&m.bright) && m.colour < 2);
        }
        let mut none = b.clone();
        none.colours.clear();
        assert!((0..10).all(|c| !matches!(
            room(&none, &r, along, c, 3).kind,
            Kind::Lit | Kind::Tv | Kind::Blinds | Kind::Curtains
        )));
    }

    /// A lit shop's window spills its light out and down onto the pavement before it; a
    /// dark or shut one, an upper storey and a bay whose scene is not a window spill none.
    #[test]
    fn a_lit_shop_spills_onto_the_pavement() {
        let b = front();
        let mut r = b.rooms.clone().unwrap();
        r.spill = (20000.0, 2.0);
        r.windows = vec![0];
        let count = |r: &Rooms| {
            let (mut lit, mut none) = (0, 0);
            for c in 0..40 {
                for st in 0..2 {
                    let rm = room(&b, r, (6.0, 126.0), c, st);
                    let on = matches!(rm.kind, Kind::Lit | Kind::Blinds | Kind::Curtains);
                    let light = on.then_some(LinearRgba::rgb(1.0, 0.7, 0.4) * 8.0);
                    let x = c as f32 * 3.0;
                    let bay = Bay { x, w: 3.0, y0: st as f32 * 3.0, nits: 8.0, rm, light };
                    match spill(&b, r, c, st, x, 6.9, Side::North, &bay) {
                        Some((at, foot, lumens, _)) => {
                            assert!(st == 0 && on && lumens > 0.0);
                            assert!(at.z < -6.0 && foot.z > at.z && foot.y < at.y, "{at} {foot}");
                            lit += 1;
                        }
                        None => none += 1,
                    }
                }
            }
            (lit, none)
        };
        let (lit, none) = count(&r);
        assert!(lit > 10 && none > lit, "{lit} spill, {none} not");
        r.windows = vec![3];
        assert_eq!(count(&r).0, 0, "a plain wall spills nothing");
    }
}
