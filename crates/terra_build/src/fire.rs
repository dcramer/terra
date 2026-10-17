//! Fire in what is built (D377): cubes of a kind that burns catch from flame, burn for as
//! long as their fuel lasts, give their heat to the cubes round them so the fire spreads,
//! char as they burn, and burn through; what they held then comes down by harm's own rule
//! (`loss`, D305). Stone does not burn, but blackens with soot where the fire licks it, and
//! falls when what holds it does.
//!
//! A cellular fire, as Far Cry 2's and Breath of the Wild's are (research/2026-10-15-dragons-
//! and-fire.md): each cube that is heating or burning is a cell with its heat and its fuel,
//! stepped on a fixed tick. A burning cube heats its neighbours, most above it (flame and
//! hot gas rise), less beside it, little below, more downwind; a cube catches once its heat
//! passes what its kind takes to ignite, a threshold a little different for each cube so a
//! fire's front is ragged, the same every time from the same start. A cube with no open face
//! has no air: it smoulders, slower and cooler. Heat not kept up cools away.
//!
//! What burns, and how, is its kind's ([`Burns`], its kit's `burns`, by its pattern unless its
//! material says otherwise). Burning harms a cube by its health over its fuel, so a burning
//! cube shows its damage as it goes (D305's cracks along the grain), and its last step
//! destroys it. The harm is said, not done: the game sends it as `Op::Damage`, as any harm,
//! so the base, its undo and every player agree (D28).
//!
//! Engine-free and deterministic: kept beside a base as its damage is, in sorted maps,
//! stepped by a fixed tick, with no clock or randomness but the cubes' own hashes.

use crate::{Base, Kind};
use glam::{IVec3, Vec3};
use std::collections::{BTreeMap, BTreeSet};

/// How a kind burns, as its kit's file says (`burns`): how much heat it takes to catch, how
/// long it burns, and how hot. Times are the game's, quicker than a real fire's (a log wall
/// burns for hours; here a minute), since a fire is watched, not waited for.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Burns {
    /// The heat it catches at: a second of a burning neighbour's full flame under it gives
    /// its heat (`heat`) times the share that reaches it; thatch catches at a touch, a log
    /// only after a while.
    pub catches: f32,
    /// How long it burns, in seconds, once caught, with air round it.
    pub fuel_s: f32,
    /// How hot it burns: the heat a second it gives the cube over it at its full flame.
    pub heat: f32,
}

/// The fixed tick a fire is stepped by, in seconds: four a second, as a fire's front moves
/// a cube in a second or two.
pub const TICK_S: f32 = 0.25;

/// What reaches each neighbour of a burning cube, a share of its heat, by where it is: over
/// it most, the flames' and hot gases' way; beside it less; under it least. Flame stands two
/// and three cubes over a fierce fire, so the cubes over those take some too.
fn reach(offset: IVec3) -> f32 {
    let side = offset.x.abs() + offset.z.abs();
    match offset.y {
        1 => [1.0, 0.55, 0.35][side as usize],
        2 if side == 0 => 0.5,
        2 if side == 1 => 0.2,
        3 if side == 0 => 0.22,
        0 => [0.0, 0.4, 0.2][side as usize],
        -1 => [0.1, 0.05, 0.02][side as usize],
        _ => 0.0,
    }
}

/// The cubes a burning cube heats, and the share each takes.
fn neighbours() -> &'static [(IVec3, f32)] {
    static ALL: std::sync::OnceLock<Vec<(IVec3, f32)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = Vec::new();
        for y in -1..=3 {
            for x in -1..=1 {
                for z in -1..=1 {
                    let offset = IVec3::new(x, y, z);
                    let share = reach(offset);
                    if share > 0.0 {
                        all.push((offset, share));
                    }
                }
            }
        }
        all
    })
}

/// How quickly heat a cube is not given cools away, a second.
const COOLS: f32 = 0.1;
/// How much a cube's threshold and fuel differ from its kind's, either way.
const SPREAD: f32 = 0.3;
/// How much more heat goes downwind, for each metre a second of wind along the way to it,
/// and at most.
const DOWNWIND: f32 = 0.12;
const MOST_DOWNWIND: f32 = 1.5;
/// A smouldering cube's (no open face) share of its fuel burnt and heat given.
const SMOULDERS: f32 = 0.35;
/// How long a fire takes to grow to its full flame, and how long it dies down at the end,
/// as shares of its fuel.
const GROWS: f32 = 0.12;
const DIES: f32 = 0.2;
/// How much soot a second of full heat leaves on what does not burn, out of 255.
const SOOTS: f32 = 60.0;
/// How much faster a cube burns for each face it shares with a burning one.
const FEEDS: f32 = 0.25;
/// How much of its fuel burnt timber carrying stone has burnt when it gives way under it.
const CRUSHED: f32 = 0.4;
/// The heat a burning cube that comes down gives what it lands on, and the cubes beside
/// that, as a brand does; and how far it is looked for under it, in cubes.
const BRAND: f32 = 2.5;
const BRAND_SIDE: f32 = 1.0;
const BRAND_FALLS: i32 = 64;

/// A cube heating or burning.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cell {
    heat: f32,
    /// Burning: how long of its fuel is left and how long it had, in seconds of full air.
    burning: Option<(f32, f32)>,
}

/// A base's fire: every cube heating or burning, and how blackened each cube is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fire {
    cells: BTreeMap<[i32; 3], Cell>,
    /// How blackened each cube is, out of 255: charred timber, sooted stone.
    scorch: BTreeMap<[i32; 3], u8>,
    /// Burnt cubes harm has not yet been sent for: the share of its health each has lost to
    /// fire that is not yet harm, carried on from tick to tick so rounding loses nothing.
    owed: BTreeMap<[i32; 3], f32>,
    /// How many ticks it has been stepped.
    ticks: u64,
}

/// What a tick of the fire did: the harm to send, by amount, and the cubes that caught and
/// that burnt through.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stepped {
    /// Harm to send as `Op::Damage`: an amount and the cubes it harms, sorted.
    pub harm: Vec<(u16, Vec<[i32; 3]>)>,
    pub caught: Vec<[i32; 3]>,
    /// Cubes whose fuel is spent: destroyed by the harm sent with them.
    pub burnt: Vec<[i32; 3]>,
    /// Whether the scorch changed, for the drawing.
    pub scorched: bool,
}

/// How a cube is blackened, as it is drawn (`Base::scorch`): how far, in [`Scorch::LEVELS`]
/// steps, charred timber or sooted stone alike. Timber with char on it is burning, since a
/// fire burns what it catches through, so the drawing glows in the char of what burns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Scorch {
    pub level: u8,
}

impl Scorch {
    pub const LEVELS: u8 = 4;

    /// How blackened, 0 to 1.
    pub fn share(&self) -> f32 {
        f32::from(self.level.min(Self::LEVELS)) / f32::from(Self::LEVELS)
    }
}

/// How blackened a surface's corner is, 0 to 1, from the cubes round it a fire has reached:
/// its own cube and those beside it on its face's plane, each reaching a cube, so a char
/// spreads smoothly across a wall from cube to cube rather than in squares, as damage is
/// drawn (`loss::struck`).
pub fn charred(scorched: &BTreeMap<[i32; 3], Scorch>, at: Vec3, normal: Vec3) -> f32 {
    if scorched.is_empty() {
        return 0.0;
    }
    let inside = at - normal * 0.02;
    let cube = (inside / crate::CELL_M).floor().as_ivec3();
    let axis = normal.abs().max_position();
    let (a, b) = ((axis + 1) % 3, (axis + 2) % 3);
    let (mut black, mut weight) = (0.0f32, 0.0f32);
    for da in -1..=1 {
        for db in -1..=1 {
            let mut near = cube;
            near[a] += da;
            near[b] += db;
            let mut off = inside - (near.as_vec3() + 0.5) * crate::CELL_M;
            off[axis] = 0.0;
            let t = (off.length() / crate::CELL_M).min(1.0);
            let reach = 1.0 - t * t * (3.0 - 2.0 * t);
            if reach <= 0.0 {
                continue;
            }
            black += scorched.get(&near.to_array()).map_or(0.0, Scorch::share) * reach;
            weight += reach;
        }
    }
    if weight <= 0.0 {
        return 0.0;
    }
    (black / weight).clamp(0.0, 1.0)
}

/// How a corner's char rides on its layer number, which the shader rounds (D377): the layer
/// plus this share of its char, under a half, so it blends across a face as a texture place
/// does and leaves the layer as it was.
pub const CHAR_SHARE: f32 = 0.4;

/// A burning cube as it is drawn: where, how fiercely (0 to 1), and which of its faces are
/// open to the air, as bits in [`crate::support::FACES`]' order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flame {
    pub at: IVec3,
    pub fierce: f32,
    pub open: u8,
}

/// A number from 0 to 1 for a cube and a use of it, the same every time.
fn luck(at: [i32; 3], salt: u32) -> f32 {
    let h = terra_gen::hash::prospector32(
        (at[0] as u32).wrapping_mul(0x8da6_b343)
            ^ (at[1] as u32).wrapping_mul(0xd816_3841)
            ^ (at[2] as u32).wrapping_mul(0xcb1a_b31f)
            ^ salt.wrapping_mul(0x9e37_79b9),
    );
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// A cube's share of its kind's, `SPREAD` either way, by its luck.
fn varied(at: [i32; 3], salt: u32) -> f32 {
    1.0 + SPREAD * (2.0 * luck(at, salt) - 1.0)
}

impl Fire {
    /// Whether anything is heating or burning.
    pub fn is_out(&self) -> bool {
        self.cells.is_empty()
    }

    /// How many cubes are burning.
    pub fn burning_count(&self) -> usize {
        self.cells
            .values()
            .filter(|cell| cell.burning.is_some())
            .count()
    }

    /// Whether a cube is burning.
    pub fn is_burning(&self, at: IVec3) -> bool {
        self.cells
            .get(&at.to_array())
            .is_some_and(|cell| cell.burning.is_some())
    }

    /// How blackened each cube is, out of 255.
    pub fn scorch(&self) -> &BTreeMap<[i32; 3], u8> {
        &self.scorch
    }

    /// Heat put into a cube from outside, a breath of flame or a brand landing on it: so
    /// much as a burning neighbour's full flame gives in a second. A cube that does not
    /// burn is sooted by it; one already burning is not heated more.
    pub fn heat(
        &mut self,
        base: &Base,
        burns: &dyn Fn(Kind) -> Option<Burns>,
        at: IVec3,
        heat: f32,
    ) {
        let Some(cube) = base.cube(at).filter(|cube| cube.is_in()) else {
            return;
        };
        let key = at.to_array();
        if burns(cube.kind).is_some_and(|b| b.catches > 0.0) {
            let cell = self.cells.entry(key).or_insert(Cell {
                heat: 0.0,
                burning: None,
            });
            if cell.burning.is_none() {
                cell.heat += heat.max(0.0);
            }
        } else {
            let soot = self.scorch.entry(key).or_insert(0);
            *soot = (f32::from(*soot) + heat.max(0.0) * SOOTS).min(255.0) as u8;
        }
    }

    /// Sets a cube burning at once, if its kind burns, as a fire is lit.
    pub fn light(&mut self, base: &Base, burns: &dyn Fn(Kind) -> Option<Burns>, at: IVec3) {
        let Some(cube) = base.cube(at).filter(|cube| cube.is_in()) else {
            return;
        };
        let Some(kind) = burns(cube.kind).filter(|b| b.catches > 0.0) else {
            return;
        };
        let key = at.to_array();
        let fuel = kind.fuel_s * (1.0 + 0.5 * (varied(key, 2) - 1.0));
        let cell = self.cells.entry(key).or_insert(Cell {
            heat: 0.0,
            burning: None,
        });
        cell.burning.get_or_insert((fuel, fuel));
    }

    /// What came down in an act (its `Done::fell`), the base as it is after it: each cube of
    /// it that was burning lands as a brand on the first cube under it in its column and
    /// heats it and the cubes beside it, so a fire burning through a roof or a floor carries
    /// down to the floors under it, as burning debris does down a tower. Call it before the
    /// next [`Fire::step`], which forgets what fell.
    pub fn fell(
        &mut self,
        base: &Base,
        burns: &dyn Fn(Kind) -> Option<Burns>,
        fell: &[(IVec3, crate::Cube)],
    ) {
        let mut landed: BTreeMap<[i32; 3], f32> = BTreeMap::new();
        for &(at, _) in fell {
            if self
                .cells
                .remove(&at.to_array())
                .is_none_or(|cell| cell.burning.is_none())
            {
                continue;
            }
            self.owed.remove(&at.to_array());
            let Some(under) = (1..=BRAND_FALLS)
                .map(|down| at - IVec3::Y * down)
                .find(|&below| base.cube(below).is_some_and(|cube| cube.is_in()))
            else {
                continue;
            };
            *landed.entry(under.to_array()).or_default() += BRAND;
            for way in [IVec3::X, IVec3::NEG_X, IVec3::Z, IVec3::NEG_Z] {
                *landed.entry((under + way).to_array()).or_default() += BRAND_SIDE;
            }
        }
        for (key, heat) in landed {
            self.heat(base, burns, IVec3::from_array(key), heat);
        }
    }

    /// Tells the base how each cube the fire has reached is blackened, for its drawing: its
    /// sections are drawn again only where a cube's level or burning changes.
    pub fn show(&self, base: &mut Base) {
        for (&key, &soot) in &self.scorch {
            let at = IVec3::from_array(key);
            let level = (u32::from(soot) * u32::from(Scorch::LEVELS)).div_ceil(255) as u8;
            base.set_scorch(at, Scorch { level });
        }
    }

    /// Every burning cube as it is drawn, sorted.
    pub fn flames<'a>(&'a self, base: &'a Base) -> impl Iterator<Item = Flame> + 'a {
        self.cells.iter().filter_map(move |(&key, cell)| {
            let (left, had) = cell.burning?;
            let at = IVec3::from_array(key);
            Some(Flame {
                at,
                fierce: fierceness(left, had),
                open: open_faces(base, at),
            })
        })
    }

    /// A tick of the fire, [`TICK_S`] long: every burning cube burns some of its fuel and
    /// gives its heat round it, by its fierceness and the wind (metres a second in the
    /// base's own space); every cube heated past what it catches at catches; the rest cool.
    /// Cubes no longer in the base (fallen, taken out) are forgotten. Returns the harm to
    /// send: each burning cube's health over its fuel for the tick, and all that is left of
    /// it for one burnt through.
    pub fn step(
        &mut self,
        base: &Base,
        burns: &dyn Fn(Kind) -> Option<Burns>,
        health: &dyn Fn(Kind) -> u16,
        wind: Vec3,
    ) -> Stepped {
        let mut out = Stepped::default();
        // What is no longer there burns no more.
        self.cells.retain(|&key, _| {
            base.cube(IVec3::from_array(key))
                .is_some_and(|cube| cube.is_in())
        });
        self.owed.retain(|key, _| self.cells.contains_key(key));
        // Heat given this tick, by cube: gathered from every burning cube before any is
        // applied, so the order cubes are stepped in changes nothing.
        // Hashed, not sorted: each cube's heat is its own sum whatever the order, and this is the
        // fire's busiest map, tens of thousands of entries a tick at its height.
        let mut given: std::collections::HashMap<[i32; 3], f32> =
            std::collections::HashMap::with_capacity(self.cells.len() * 8);
        let mut harm: BTreeMap<u16, Vec<[i32; 3]>> = BTreeMap::new();
        let mut destroyed = BTreeSet::new();
        let lit: std::collections::HashSet<[i32; 3]> = self
            .cells
            .iter()
            .filter(|(_, cell)| cell.burning.is_some())
            .map(|(&key, _)| key)
            .collect();
        for (&key, cell) in &mut self.cells {
            let Some((left, had)) = cell.burning.as_mut() else {
                continue;
            };
            let at = IVec3::from_array(key);
            let Some(cube) = base.cube(at) else {
                continue;
            };
            let Some(kind) = burns(cube.kind) else {
                continue;
            };
            let open = open_faces(base, at);
            let air = if open == 0 { SMOULDERS } else { 1.0 };
            let fierce = fierceness(*left, *had) * air;
            // A fire feeds itself: a cube among burning ones burns faster, heated from round
            // it as well as by its own flame.
            let fed = crate::support::FACES
                .iter()
                .filter(|&&way| lit.contains(&(at + way).to_array()))
                .count() as f32;
            let burnt = (TICK_S * air * (1.0 + FEEDS * fed)).min(*left);
            *left -= burnt;
            for &(offset, share) in neighbours() {
                let to = at + offset;
                let way = offset.as_vec3().normalize();
                let downwind = (1.0 + DOWNWIND * wind.dot(way).max(0.0)).min(MOST_DOWNWIND);
                *given.entry(to.to_array()).or_default() +=
                    kind.heat * fierce * share * downwind * TICK_S;
            }
            let full = f32::from(health(cube.kind).max(1));
            // Charred through most of its section, timber carrying what does not burn, stone
            // or slate laid on it, has lost its strength and gives way under it.
            let laden = base
                .cube(at + IVec3::Y)
                .is_some_and(|over| over.is_in() && burns(over.kind).is_none());
            if *left <= 0.0 || (laden && *left <= *had * (1.0 - CRUSHED)) {
                destroyed.insert(key);
                harm.entry(u16::MAX).or_default().push(key);
            } else {
                // Its health over its fuel, owed until it makes a sixteenth of its health,
                // so cubes of a kind are harmed alike and sent together in few acts.
                let owed = self.owed.entry(key).or_default();
                *owed += full * burnt / *had;
                let quantum = (full / 16.0).floor().max(1.0);
                let whole = (*owed / quantum).floor() * quantum;
                if whole >= 1.0 {
                    *owed -= whole;
                    harm.entry(whole.min(f32::from(u16::MAX - 1)) as u16)
                        .or_default()
                        .push(key);
                }
            }
            // Charred by how much of it has burnt.
            let char = (255.0 * (1.0 - *left / *had).clamp(0.0, 1.0) * 0.7 + 255.0 * 0.3) as u8;
            let scorch = self.scorch.entry(key).or_insert(0);
            if char > *scorch {
                *scorch = char;
                out.scorched = true;
            }
        }
        for key in &destroyed {
            self.cells.remove(key);
            self.owed.remove(key);
        }
        out.burnt = destroyed.into_iter().collect();
        if self.ticks.is_multiple_of(HOT_GAS_EVERY) {
            self.hot_gas(base, burns, &mut given);
        }
        self.ticks += 1;
        // Heat given, taken by what burns, and soot on what does not.
        for (key, heat) in given {
            let at = IVec3::from_array(key);
            let Some(cube) = base.cube(at).filter(|cube| cube.is_in()) else {
                continue;
            };
            match burns(cube.kind).filter(|b| b.catches > 0.0) {
                Some(_) => {
                    let cell = self.cells.entry(key).or_insert(Cell {
                        heat: 0.0,
                        burning: None,
                    });
                    if cell.burning.is_none() {
                        cell.heat += heat;
                    }
                }
                None => {
                    let soot = self.scorch.entry(key).or_insert(0);
                    let now = (f32::from(*soot) + heat * SOOTS).min(255.0) as u8;
                    if now != *soot {
                        *soot = now;
                        out.scorched = true;
                    }
                }
            }
        }
        // What is hot enough catches; what is not cools; what is cold is forgotten.
        let mut cold = Vec::new();
        for (&key, cell) in &mut self.cells {
            if cell.burning.is_some() {
                continue;
            }
            let Some(kind) = base
                .cube(IVec3::from_array(key))
                .and_then(|cube| burns(cube.kind))
            else {
                cold.push(key);
                continue;
            };
            if cell.heat >= kind.catches * varied(key, 1) {
                let fuel = kind.fuel_s * (1.0 + 0.5 * (varied(key, 2) - 1.0));
                cell.burning = Some((fuel, fuel));
                out.caught.push(key);
            } else {
                cell.heat -= COOLS * TICK_S;
                if cell.heat <= 0.0 {
                    cold.push(key);
                }
            }
        }
        for key in cold {
            self.cells.remove(&key);
        }
        out.harm = harm.into_iter().collect();
        out
    }
}

impl Fire {
    /// The hot gas a fire fills the air round it with (a compartment fire's hot layer): the
    /// air within reach of each burning cube's open faces, flooded a few cubes deep, its heat
    /// the burning cubes round it share out over how much air there is, given to everything
    /// that burns facing it. In the open the air is wide and takes little; in a tower's storey
    /// it is shut in, and once enough of it burns everything round it catches at once, as a
    /// room flashes over.
    fn hot_gas(
        &self,
        base: &Base,
        burns: &dyn Fn(Kind) -> Option<Burns>,
        given: &mut std::collections::HashMap<[i32; 3], f32>,
    ) {
        use crate::support::FACES;
        let solid = |at: IVec3| base.cube(at).is_some_and(|cube| cube.is_in());
        let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
        for (&key, cell) in &self.cells {
            if cell.burning.is_none() || seen.contains(&key) {
                continue;
            }
            // Flood the air from this cube's open faces, gathering the burning cubes it meets.
            let start = IVec3::from_array(key);
            let mut air: BTreeMap<[i32; 3], i32> = BTreeMap::new();
            let mut queue = std::collections::VecDeque::new();
            for way in FACES {
                let to = start + way;
                if !solid(to) && air.insert(to.to_array(), 0).is_none() {
                    queue.push_back(to);
                }
            }
            let mut sources = BTreeSet::from([key]);
            let mut facing = BTreeSet::new();
            while let Some(at) = queue.pop_front() {
                let depth = air[&at.to_array()];
                for way in FACES {
                    let to = at + way;
                    let k = to.to_array();
                    if solid(to) {
                        if self.cells.get(&k).is_some_and(|c| c.burning.is_some()) {
                            sources.insert(k);
                        } else {
                            facing.insert(k);
                        }
                    } else if depth < GAS_REACH && air.len() < GAS_MOST && !air.contains_key(&k) {
                        air.insert(k, depth + 1);
                        queue.push_back(to);
                    }
                }
            }
            seen.extend(sources.iter().copied());
            let heat: f32 = sources
                .iter()
                .filter_map(|k| {
                    let (left, had) = self.cells[k].burning?;
                    let kind = burns(base.cube(IVec3::from_array(*k))?.kind)?;
                    Some(kind.heat * fierceness(left, had))
                })
                .sum();
            // Hot gas rises: under a roof or a floor it gathers, under the open sky it goes. How
            // shut in it is, the share of its air with something built over it within a
            // storey or so, looked at on every fourth cube of it.
            let looked: Vec<IVec3> = air
                .keys()
                .step_by(4)
                .map(|&k| IVec3::from_array(k))
                .collect();
            let covered = looked
                .iter()
                .filter(|&&at| (1..=GAS_COVER).any(|up| solid(at + IVec3::Y * up)))
                .count();
            let shut_in = covered as f32 / looked.len().max(1) as f32;
            let hot = HOT_GAS * shut_in * heat / (air.len().max(GAS_LEAST) as f32)
                * TICK_S
                * HOT_GAS_EVERY as f32;
            if hot <= 0.0 {
                continue;
            }
            for k in facing {
                *given.entry(k).or_default() += hot;
            }
        }
    }
}

/// How often the hot gas is worked out, in ticks; how deep its air is flooded from a fire,
/// in cubes, and at most how many cubes of it; the least air its heat is shared over; and
/// how much heat it gives.
const HOT_GAS_EVERY: u64 = 4;
const GAS_REACH: i32 = 8;
/// How far over the hot gas something built keeps it in, in cubes.
const GAS_COVER: i32 = 8;
const GAS_MOST: usize = 2500;
const GAS_LEAST: usize = 64;
const HOT_GAS: f32 = 30.0;

/// How fiercely a fire burns with so much of its fuel left of what it had: growing at
/// first, then full, then dying to embers.
fn fierceness(left: f32, had: f32) -> f32 {
    let through = 1.0 - (left / had.max(1e-6)).clamp(0.0, 1.0);
    let grown = (through / GROWS).min(1.0);
    let dying = ((1.0 - through) / DIES).min(1.0);
    (0.25 + 0.75 * grown).min(0.3 + 0.7 * dying)
}

/// Which of a cube's faces are open to the air: nothing in the cube there.
fn open_faces(base: &Base, at: IVec3) -> u8 {
    crate::support::FACES
        .iter()
        .enumerate()
        .filter(|&(_, &way)| !base.cube(at + way).is_some_and(|cube| cube.is_in()))
        .fold(0, |bits, (i, _)| bits | (1 << i))
}

#[cfg(test)]
mod tests;
