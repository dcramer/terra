//! A kind of construction's kit (docs/building-kits.md, D136, D177): a file in
//! `assets/data/kits/` saying how its walls are laid, how big its pieces are, how each long
//! member is shaped and varies (D200), its materials and what building in it costs. A new
//! kind, a pine cabin or an oak one, round logs or hewn beams, is a new file, not code.

use crate::Kind;
use crate::cost::{Amount, Bill, Rules, Terms};
use crate::layout::roles::Role;
use crate::mesh::Material;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A kind of construction's kit: how its walls are laid, its sizes, how its timber is
/// shaped, and its colours and textures. Colours are as they should appear on screen
/// (sRGB). What a file leaves out is pine's, as `logs.ron` has it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Kit {
    /// The kind of construction it lays (D181): the number the cubes it lays carry.
    pub id: u16,
    /// How its walls are laid: as long members, or as blocks.
    pub pattern: Pattern,
    /// How thick a log is, or how wide a beam.
    pub diameter: f32,
    /// How far apart the courses are; less than the diameter, as logs are scribed to sit
    /// into one another. Must divide the storey.
    pub course: f32,
    /// How far a log runs past the corner cube it crosses at, beyond the grid. None, so
    /// that every piece stays inside its cubes and anything can be built against it.
    pub overhang: f32,
    /// The longest wall one log spans, the longest a tree gives (D174).
    pub longest: f32,
    /// How far short of the others' a corner log's end may stop, in metres, with a saddle
    /// notch; flush ends never do.
    pub ragged: f32,
    /// Whether there is daub in the groove between courses.
    pub chinking: bool,
    /// The boards round an opening: how wide and how far proud of the logs.
    pub frame: f32,
    /// A roof kind's: how far its covering is carried on past its cubes' open edges, drawn
    /// only (D192): down at its eaves, level past its verges (D175, D241).
    pub eaves: f32,
    /// A wall kind's: the roof kind a roof over its walls is laid in where the hammer holds
    /// no roof's material, and an old save's roof over them is turned into (D241).
    pub roof_kind: Option<u16>,
    /// A block kind's surface: the radius its outside edges are rounded to, and how far
    /// its material's relief stands out (`surface.rs`).
    pub bevel: f32,
    pub relief: f32,
    /// How each long member is shaped, and how much one differs from the next (D200); a
    /// roof kind's rafters are of it.
    pub timber: Timber,
    /// A roof kind's: how its roof is made, its frame, battens, covering and trims.
    pub roof: Roofing,
    /// Its materials' textures (D14): where each is, how many metres across it is, and how
    /// deep its parallax reads; a material with none is drawn in its plain colour.
    pub textures: BTreeMap<Material, Texture>,
    pub colours: KitColours,
    /// What its cubes cost to build (D139).
    pub cost: KitCost,
    /// How each exposed edge of its cubes is dressed by its role (D215, `layout::roles`):
    /// its pieces, what each role takes, and its masks.
    pub dress: Dressing,
    /// How it meets other kinds where their cubes touch (D232): left out, as its pattern
    /// meets them ([`Meets::of`]).
    pub meets: Option<Meets>,
    /// What a block kind's surface's colour is multiplied by (sRGB): its earth or its daub
    /// warmed or lightened from its texture's, a grade a kit (docs/art-style.md).
    pub tint: [f32; 3],
    /// A block kind framed in timber (D236, `layout::frames`): its cubes the infill, drawn as
    /// their surface, and a frame of its members laid over them by rule; left out, none.
    pub framing: Option<Framing>,
}

/// How a framed kind's frame is laid and drawn (D236, `layout::frames`): wattle and daub,
/// and later plaster or brick nogging, are a block kind's cubes as the infill with a frame of
/// squared timber over them: posts up its corners, its openings' sides and every so many
/// cubes along a wall, a sill plate along its foot and a plate along its top, rails across
/// it at heights in each storey, and braces in the panels by its walls' ends. Every member
/// stands a little proud of the infill's face, so the panels read set back in the frame.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Framing {
    /// How wide a member shows on a wall's face, in metres: a post's width, a plate's or a
    /// rail's height.
    pub wide: f32,
    /// How far a member stands out of the infill's face, and how far into the wall it goes
    /// behind its own face, in metres.
    pub proud: f32,
    pub deep: f32,
    /// A corner post's side, square, in metres.
    pub corner: f32,
    /// A brace's width, in metres.
    pub brace: f32,
    /// How far apart its posts stand along a wall at most, in cubes: one at every multiple of
    /// it on the grid, but none within a metre of a wall's end or an opening.
    pub bay: i32,
    /// The heights of its rails over each storey's foot, in cubes. A rail in the lower half
    /// of a storey sits under its line, as a window's sill beam does, one in the upper half
    /// over it, as a head beam does, so rails run on into an opening's frame.
    pub rails: Vec<i32>,
    /// Which panels are braced.
    pub braces: Braces,
    /// Its members' material: squared timber.
    pub material: Material,
    /// What the infill is made on before it is finished, and is shown while a cube is part
    /// built or being cut out (`Piece::Wattle`): wattle under daub. None, nothing.
    pub core: Option<Material>,
    /// How far the core's own relief stands out, in metres.
    pub core_relief: f32,
}

/// Which of a frame's panels have a brace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Braces {
    None,
    /// The panels at each end of a wall, under its lowest rail and over its highest, each
    /// brace leaning in to the post at the end, as the corners of a frame are stiffened.
    #[default]
    Ends,
}

impl Default for Framing {
    /// Squared oak a little under 20 cm, a post every metre and a half, rails at one and
    /// two metres, braced at the ends, over wattle.
    fn default() -> Self {
        Framing {
            wide: 0.18,
            proud: 0.03,
            deep: 0.1,
            corner: 0.2,
            brace: 0.14,
            bay: 3,
            rails: vec![2, 4],
            braces: Braces::Ends,
            material: Material::Hewn,
            core: Some(Material::Wattle),
            core_relief: 0.03,
        }
    }
}

/// How a kind meets the others where their cubes touch (D179, D210, D232), as data, so a
/// new kit says how it meets the rest. Where two kinds meet, the heavier runs through: its
/// faces and edges at the joint are dressed as though the lighter were not there (a stone
/// wall's quoins up the joint beside logs), and the lighter ends against it with its `end`
/// trim; of two as heavy, each ends against the other. Where a lighter kind stands on it,
/// the heavier puts its `carries` trim along its top. `kinds` says, for a kind by its id,
/// where that differs from its weight's rule.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Meets {
    /// How heavy it reads where kinds meet: stone 3, timber 2, wattle 1.
    pub weight: u8,
    /// Its trim where it ends against a kind that runs through it.
    pub end: Joint,
    /// Its trim along its top where a kind it runs through stands on it.
    pub carries: Joint,
    pub kinds: BTreeMap<u16, Meet>,
}

impl Default for Meets {
    fn default() -> Self {
        Meets::of(Pattern::Logs)
    }
}

impl Meets {
    /// How a pattern meets the others unless its kit says otherwise: a block kind runs
    /// through a timber one, capping its top under it (D179, D210); a timber kind ends
    /// against anything with an end board.
    pub fn of(pattern: Pattern) -> Meets {
        match pattern {
            Pattern::Masonry => Meets {
                weight: 3,
                end: Joint::None,
                carries: Joint::Cap,
                kinds: BTreeMap::new(),
            },
            Pattern::Logs => Meets {
                weight: 2,
                end: Joint::Board,
                carries: Joint::None,
                kinds: BTreeMap::new(),
            },
            // A roof is drawn over what it rests on, which it neither caps nor ends against.
            Pattern::Roof => Meets {
                weight: 0,
                end: Joint::None,
                carries: Joint::None,
                kinds: BTreeMap::new(),
            },
        }
    }
}

/// How one kind meets another, where it differs from the weights' rule: whether it runs
/// through that kind, and its trims there.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Meet {
    pub through: Option<bool>,
    pub end: Option<Joint>,
    pub carries: Option<Joint>,
}

/// A trim at a joint between kinds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Joint {
    #[default]
    None,
    /// An end board up the joint, over a timber kind's sawn ends (`layout::trims`).
    Board,
    /// A cap along the top, dressed as the kit dresses [`Role::Meet`].
    Cap,
}

/// How a kit dresses its cubes' exposed edges by their roles (D215, docs/building-kits.md):
/// profiles swept along them and corner stones up them, from the kit's pieces, made in
/// Blender from their source under `art/kits/` (`tools/kits`); long members of its timber;
/// and the masks that ground and weather it. A new look is a kit file and its source.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Dressing {
    /// Its pieces' file under `assets/`, as `tools/kits/pieces.py` makes it.
    pub pieces: Option<String>,
    /// What each role is dressed with; a role not named is left as the cubes are.
    pub roles: BTreeMap<Role, Dress>,
    /// What its pieces' colour is multiplied by (sRGB): its dressed stone greyed towards the
    /// rough stone round it, one grade a kit (docs/art-style.md).
    pub tint: [f32; 3],
    pub masks: Masks,
    /// Its pieces, once read ([`Dressing::read`]).
    #[serde(skip)]
    pub made: Option<Arc<crate::dress::Pieces>>,
}

/// What a role is dressed with.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Dress {
    /// A profile of the kit's pieces swept along the edge, a stone a cube long with a joint
    /// between, or, without joints, as one stone however long; run on past each end that
    /// stops square by `extend` metres, as a lintel bears on the wall either side. Drawn in
    /// its `material`, the kit's `Dressed` stone unless it says; in timber (`Hewn`, `Log`)
    /// its texture runs along it, as a beam's grain does.
    Profile {
        name: String,
        #[serde(default)]
        extend: f32,
        #[serde(default = "joints")]
        joints: bool,
        #[serde(default)]
        material: Option<Material>,
    },
    /// Corner stones of one of the kit's sets up the edge, a stone a course.
    Corners { set: String },
    /// A long member of the kit's timber along the edge's run (D200): its section, how much
    /// thicker than a log, and its face's material.
    Member {
        profile: Profile,
        scale: f32,
        material: Material,
    },
    /// A block kind's post (`Role::Post`): a pillar of its stone so wide, in metres, a drum a
    /// cube, its base and its cap dressed as its foot and its crest are.
    Pillar { width: f32 },
}

fn joints() -> bool {
    true
}

/// How a kit's walls weather (research/role-pieces.md, candidate 2's masks): dirt and damp
/// rising unevenly from the ground, outside edges worn lighter, and timber greyed higher up
/// where the weather reaches it. Shades of what is drawn, as each corner's colour.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Masks {
    /// How high the dirt rises from the ground, in metres, and what it darkens to (sRGB).
    pub dirt: f32,
    pub dirt_tint: [f32; 3],
    /// How much lighter its outside edges are worn, as a fraction.
    pub wear: f32,
    /// How much greyer timber is three metres up, as a fraction.
    pub weather: f32,
    /// How much a roof's covering is greened by moss and lichen toward its eaves, where the
    /// rain runs off and lingers, as a fraction, and what they multiply its colour by
    /// (sRGB): greener and darker on shakes, paler on slate.
    pub moss: f32,
    pub moss_tint: [f32; 3],
    /// How much a block kind's lifts differ in shade, as a fraction either way, and how tall
    /// a lift is, in metres: rammed or cob earth laid a lift at a time, its layers wandering
    /// a little along the wall (D236).
    pub strata: f32,
    pub lift: f32,
}

impl Default for Masks {
    /// None: drawn as it is.
    fn default() -> Self {
        Masks {
            dirt: 0.0,
            dirt_tint: [1.0; 3],
            wear: 0.0,
            weather: 0.0,
            moss: 0.0,
            moss_tint: [1.0; 3],
            strata: 0.0,
            lift: 0.15,
        }
    }
}

impl Default for Dressing {
    /// Nothing dressed.
    fn default() -> Self {
        Dressing {
            pieces: None,
            roles: BTreeMap::new(),
            tint: [1.0; 3],
            masks: Masks::default(),
            made: None,
        }
    }
}

impl Dressing {
    /// The long member a role is dressed with, if any: its section, how much thicker than a
    /// log, and its face's material.
    pub fn member(&self, role: Role) -> Option<(Profile, f32, Material)> {
        match self.roles.get(&role)? {
            Dress::Member {
                profile,
                scale,
                material,
            } => Some((*profile, *scale, *material)),
            _ => None,
        }
    }

    /// Reads its pieces' file, `read` giving a file under `assets/` as text.
    pub fn read(&mut self, read: &dyn Fn(&str) -> Result<String, String>) -> Result<(), String> {
        let Some(path) = &self.pieces else {
            return Ok(());
        };
        let text = read(path)?;
        let pieces: crate::dress::Pieces =
            ron::from_str(&text).map_err(|why| format!("{path}: {why}"))?;
        self.made = Some(Arc::new(pieces));
        Ok(())
    }
}

/// How a long member is made (D200): a profile swept along the piece the layout gives,
/// tapered, bent and uneven as a tree grows, its face displaced by its material's height
/// and its cut ends showing their grain. Every number varies a little by the piece's seed,
/// so no two logs in a wall are the same.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Timber {
    /// Its section: a round log in its bark, or a beam squared by the axe.
    pub profile: Profile,
    /// How its ends finish at a corner.
    pub notch: Notch,
    /// How many points round its section.
    pub sides: u32,
    /// How far apart its rings are along it, in metres.
    pub step: f32,
    /// How much thicker its butt is than its middle, and its top thinner, as a fraction.
    pub taper: f32,
    /// How far it bows from straight over its length, in metres, at most.
    pub bend: f32,
    /// How uneven its section is, and how much it swells here and there along it, as
    /// fractions of its radius.
    pub uneven: f32,
    pub swell: f32,
    /// How far its face's material stands out from it, from its deepest to its proudest,
    /// in metres: the bark's height map, read on the CPU.
    pub relief: f32,
    /// The daub in the groove under it: how far out from the wall's middle its face is,
    /// as a fraction of the member's half width, and how tall it is, in metres; a hewn
    /// beam's fills the gap between it and the course below.
    pub daub: (f32, f32),
    /// What its face's and its ends' colours are multiplied by (sRGB).
    pub tint: [f32; 3],
    pub end_tint: [f32; 3],
    /// How much each member differs from the next.
    pub variation: Variation,
}

/// How a roof kind's roofs are made (`layout::roofs`, docs/building-kits.md): a frame of
/// rafters in its timber, battens across them, a covering of courses (shakes, slates) or a
/// thick coat (thatch) in its `Covering` material, boards along its eaves and verges, caps
/// over its ridges and hips, and a mortar fillet where it meets a wall.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Roofing {
    /// How many courses one copy of the covering's texture holds up the slope: with its
    /// size, how much of each course shows (its exposure).
    pub rows: f32,
    /// Where a course's lower edge falls on the texture, as a fraction of it down from its
    /// top, so the texture's rows sit on the courses' steps.
    pub phase: f32,
    /// How far each course's lower edge stands out from the one under it, in metres.
    pub butt: f32,
    /// How thick the covering is over its battens, in metres: a few centimetres of shakes
    /// or slate, a foot of thatch.
    pub thick: f32,
    /// How much a course's butt varies along it, as a fraction, so no row is ruled.
    pub uneven: f32,
    /// What the covering's colour is multiplied by (sRGB).
    pub tint: [f32; 3],
    /// Its rafters: their section, and how thick they are, in metres.
    pub rafter: (Profile, f32),
    /// Its battens, one under each course: how wide and how thick.
    pub batten: (f32, f32),
    /// The boards along its eaves (fascia) and verges (bargeboards): how deep and thick.
    pub board: (f32, f32),
    /// What its boards' colour is multiplied by (linear): their wood weathered to a silver
    /// grey-brown, lighter than the bare texture, so a roof's edges read against its
    /// covering and the walls, not as black lines.
    pub board_tint: [f32; 3],
    /// What caps its ridges and hips, and how wide it is either side of them.
    pub ridge: Ridge,
    pub cap: f32,
    /// How big the fillet of mortar is where it meets a wall standing above it.
    pub fillet: f32,
}

/// What caps a roof's ridges and hips.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Ridge {
    /// Two boards meeting over it, one on each slope.
    #[default]
    Boards,
    /// A rounded roll of the covering itself: ridge tiles, or a thatch's ridge.
    Roll,
}

impl Default for Roofing {
    /// Split pine shakes over round poles, as `logs.ron` has it.
    fn default() -> Self {
        Roofing {
            rows: 6.0,
            phase: 0.145,
            butt: 0.03,
            thick: 0.025,
            uneven: 0.35,
            tint: [1.0, 0.97, 0.93],
            rafter: (Profile::Round, 0.14),
            batten: (0.06, 0.025),
            board: (0.22, 0.035),
            board_tint: [1.42, 1.36, 1.26],
            ridge: Ridge::Boards,
            cap: 0.16,
            fillet: 0.1,
        }
    }
}

impl Roofing {
    /// How far a course shows along the slope, in metres, for a covering texture so many
    /// metres down.
    pub fn exposure(&self, metres: f32) -> f32 {
        metres / self.rows.max(0.1)
    }
}

/// How much one member differs from the next, each by its seed: its thickness as a
/// fraction either way, how much lighter or darker it is, and how much warmer or cooler.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Variation {
    pub thickness: f32,
    pub shade: f32,
    pub warmth: f32,
}

/// A long member's section.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Profile {
    /// A round log in its bark, a little flattened where it sits on the one below.
    #[default]
    Round,
    /// A beam hewn square, its edges rounded where the axe met the tree's round, its faces
    /// scalloped by the adze, a band of daub showing between courses.
    Hewn,
}

/// How a member's ends finish where walls cross.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Notch {
    /// Saddle-notched round logs: the ends stack at the corner, each sawn a little short
    /// or long of the next, chamfered by the axe.
    #[default]
    Saddle,
    /// Dovetailed square timber: every end flush with the corner's face, sawn square.
    Flush,
}

/// What a kind costs to build (D139, D194): how many steps fill a cube, and what each
/// step takes, by the items' names, in exact fractions (numerator, denominator); a roof
/// kind's four steps are its frame, its battens, its covering and its trims (D241). What
/// is left out is free.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KitCost {
    pub steps: u8,
    pub step: Vec<(String, u64, u64)>,
}

impl KitCost {
    fn bill(amounts: &[(String, u64, u64)]) -> Bill {
        let mut bill = Bill::default();
        for (material, num, den) in amounts {
            bill.add(&Bill::of(material, Amount::new(*num, (*den).max(1))));
        }
        bill
    }
}

/// How a kit lays its walls (D160: a few patterns in code, kits as data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    /// Courses of long members, crossing at the corners (`layout::runs`).
    #[default]
    Logs,
    /// Blocks (`layout::blocks`): stone, brick, plaster or earth, drawn as the cubes' own
    /// surface, rounded and displaced by the kit's `Masonry` material (D197). Where it
    /// meets logs, it runs through and the logs end flush against it (D179).
    Masonry,
    /// A roof's covering (`layout::roofs`, D241): its cubes, cut to slopes and corners, drawn
    /// as a roof over them, ridges, hips, valleys, eaves and verges from their neighbours.
    /// Its kind is a roof's ([`Kind::is_roof`]).
    Roof,
}

/// Every kit a base is laid by, one for each kind of construction.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kits(pub Vec<Kit>);

impl Kits {
    /// The kind footings and door steps are laid in (D194, D206): the first block kind by its
    /// id that is not framed, stone.
    pub fn footing(&self) -> Option<&Kit> {
        self.0
            .iter()
            .filter(|kit| kit.pattern == Pattern::Masonry && kit.framing.is_none())
            .min_by_key(|kit| kit.id)
    }

    /// The kit that lays a kind, if any does.
    pub fn of(&self, kind: Kind) -> Option<&Kit> {
        self.0.iter().find(|kit| kit.kind() == kind)
    }

    /// What building in each kit's kind costs; props are not the kits' to say.
    pub fn rules(&self) -> Rules {
        let mut rules = Rules::default();
        for kit in &self.0 {
            let terms = Terms::new(kit.cost.steps.max(1), KitCost::bill(&kit.cost.step));
            rules.kinds.insert(kit.kind(), terms);
        }
        rules
    }

    /// The roof kind a roof over walls of a kind is laid in (D241): the kind itself if it is
    /// a roof's, else its kit's `roof_kind`, else the first roof kit's.
    pub fn roof_for(&self, kind: Kind) -> Option<Kind> {
        if kind.is_roof() {
            return Some(kind);
        }
        self.of(kind)
            .and_then(|kit| kit.roof_kind)
            .map(Kind)
            .or_else(|| {
                self.0
                    .iter()
                    .find(|kit| kit.pattern == Pattern::Roof)
                    .map(Kit::kind)
            })
    }

    /// How a kind meets the others: its kit's table, or its pattern's.
    pub fn meets(&self, kind: Kind) -> Meets {
        self.of(kind).map_or_else(
            || Meets::of(Pattern::Logs),
            |kit| kit.meets.clone().unwrap_or_else(|| Meets::of(kit.pattern)),
        )
    }

    /// What one kind's table says of another, if anything.
    fn meet(&self, kind: Kind, other: Kind) -> Meet {
        self.meets(kind)
            .kinds
            .get(&other.0)
            .cloned()
            .unwrap_or_default()
    }

    /// Whether `kind` runs through `other` where they meet (D232): as either's table says
    /// of the other, else the heavier does.
    pub fn through(&self, kind: Kind, other: Kind) -> bool {
        if kind == other {
            return false;
        }
        if let Some(through) = self.meet(kind, other).through {
            return through;
        }
        if let Some(through) = self.meet(other, kind).through {
            return !through;
        }
        self.meets(kind).weight > self.meets(other).weight
    }

    /// The trim `kind` puts where it ends against `other`, if `other` runs through it or
    /// neither does.
    pub fn end(&self, kind: Kind, other: Kind) -> Joint {
        if kind == other || self.through(kind, other) {
            return Joint::None;
        }
        self.meet(kind, other).end.unwrap_or(self.meets(kind).end)
    }

    /// The trim `kind` puts along its top where `other` stands on it, if it runs through
    /// `other`.
    pub fn carries(&self, kind: Kind, other: Kind) -> Joint {
        if !self.through(kind, other) {
            return Joint::None;
        }
        self.meet(kind, other)
            .carries
            .unwrap_or(self.meets(kind).carries)
    }

    /// Whether a kind goes in a cube at a time, as blocks do, rather than a course across
    /// every cube of a part, as long members do (D194).
    pub fn one_at_a_time(&self, kind: Kind) -> bool {
        self.of(kind)
            .is_some_and(|kit| kit.pattern == Pattern::Masonry)
    }
}

/// A material's textures: a folder under `assets` with `color.jpg`, `normal.jpg` and
/// `arm.jpg` (occlusion, roughness, metal), and `height.jpg` for parallax or relief.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Texture {
    pub path: String,
    /// How many metres across and down one copy of it covers.
    pub metres: [f32; 2],
    /// How deep its parallax reads, as a fraction of its size; none if not given.
    #[serde(default)]
    pub depth: f32,
    /// How much light it reflects as a mirror would, head on, as Bevy and Filament say it:
    /// 0.5 is 4%, a generic dielectric; 0.35, 2%, which the kits' bark, stone, plaster and
    /// weathered wood are given unless they say otherwise. At 4% the painted sky's blue
    /// sheened every kit's material in shade and washed roofs pale at a slant.
    #[serde(default = "matte")]
    pub reflectance: f32,
    /// For a texture of boards side by side, such as planks, where they lie across it: the
    /// first board's edge and how far apart they are, as fractions of its width, so a board
    /// made on its own is drawn from one board of it, not across a seam.
    #[serde(default)]
    pub strips: Option<[f32; 2]>,
}

/// A rough natural material's reflectance (see [`Texture::reflectance`]).
pub fn matte() -> f32 {
    0.35
}

/// The plain colours of what a kit has no texture for yet: a floor, a flat roof and their
/// sides, and the daub between courses, which tints its texture.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KitColours {
    pub light: [f32; 3],
    pub dark: [f32; 3],
    pub roof: [f32; 3],
    pub chinking: [f32; 3],
}

impl Default for Timber {
    /// Pine in its bark, as `logs.ron` has it.
    fn default() -> Self {
        Timber {
            profile: Profile::Round,
            notch: Notch::Saddle,
            sides: 18,
            step: 0.08,
            taper: 0.07,
            bend: 0.03,
            uneven: 0.035,
            swell: 0.02,
            relief: 0.022,
            daub: (0.7, 0.05),
            tint: [1.0, 0.95, 0.89],
            end_tint: [0.86, 0.82, 0.78],
            variation: Variation::default(),
        }
    }
}

impl Default for Variation {
    fn default() -> Self {
        Variation {
            thickness: 0.06,
            shade: 0.16,
            warmth: 0.05,
        }
    }
}

impl Default for Kit {
    /// Pine, as `assets/data/kits/logs.ron` has it; for tests.
    fn default() -> Self {
        Kit {
            id: 1,
            pattern: Pattern::Logs,
            diameter: 0.3,
            course: 0.25,
            overhang: 0.0,
            longest: 4.0,
            ragged: 0.04,
            chinking: true,
            frame: 0.08,
            eaves: 0.5,
            roof_kind: None,
            bevel: 0.04,
            relief: 0.05,
            timber: Timber::default(),
            roof: Roofing::default(),
            textures: BTreeMap::new(),
            cost: KitCost::default(),
            dress: Dressing::default(),
            colours: KitColours {
                light: [0.74, 0.56, 0.37],
                dark: [0.5, 0.34, 0.22],
                roof: [0.47, 0.42, 0.37],
                chinking: [0.8, 0.75, 0.66],
            },
            meets: None,
            framing: None,
            tint: [1.0; 3],
        }
    }
}

impl Kit {
    /// How many courses fill a storey, and whether they fill it exactly.
    pub fn courses(&self) -> (i32, bool) {
        let count = (crate::STOREY_M / self.course).round();
        (
            count as i32,
            (count * self.course - crate::STOREY_M).abs() < 1e-3,
        )
    }

    /// The kind of construction it lays.
    pub fn kind(&self) -> Kind {
        Kind(self.id)
    }

    pub fn radius(&self) -> f32 {
        self.diameter * 0.5
    }

    /// How far short of the corner's face an end may stop: by up to `ragged` when saddle
    /// notched, never when flush.
    pub fn ragged(&self) -> f32 {
        match self.timber.notch {
            Notch::Saddle => self.ragged,
            Notch::Flush => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn assets() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    fn shipped(name: &str) -> Kit {
        let path = assets().join("data/kits").join(name);
        let text = std::fs::read_to_string(&path).unwrap();
        ron::from_str(&text).unwrap_or_else(|why| panic!("{}: {why}", path.display()))
    }

    #[test]
    fn the_default_kit_is_pine_as_its_file_has_it() {
        // What a kit's file leaves out is the default's, which is to be logs.ron's but
        // for what it lays with: its textures and cost; and for how it is dressed,
        // its ends run past the corners and its role members, so tests lay plain logs
        // inside their cubes; and how it meets other kinds, which the default leaves to
        // its pattern, so a test's stone kit meets as stone, and the file says as logs; and
        // the roof over it, which the file names.
        let file = shipped("logs.ron");
        assert_eq!(file.meets, Some(Meets::of(Pattern::Logs)));
        let pine = Kit {
            textures: BTreeMap::new(),
            cost: KitCost::default(),
            overhang: 0.0,
            dress: Dressing::default(),
            meets: None,
            roof_kind: None,
            ..file
        };
        assert_eq!(pine, Kit::default());
    }

    #[test]
    fn a_hewn_kit_is_a_file_and_lays_flush() {
        let hewn: Kit = ron::from_str(
            "(id: 9, timber: (profile: Hewn, notch: Flush, relief: 0.0), \
             textures: { Log: (path: \"materials/rough_wood\", metres: (0.6, 1.4)) })",
        )
        .unwrap();
        assert_eq!(hewn.timber.profile, Profile::Hewn);
        assert_eq!(hewn.ragged(), 0.0);
        assert!(Kit::default().ragged() > 0.0);
    }
}
