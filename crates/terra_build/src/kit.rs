//! A kind of construction's kit (docs/building-kits.md, D136, D177, D290): how its walls are
//! laid, how big its pieces are, how each long member is shaped and varies (D200), its
//! materials and what building in it costs. Its file in `assets/data/kits/` is a material: a
//! few lines naming its pattern (`data/kits/patterns/`) and its reference pack
//! (`data/packs/`), its cost and weight, and what it overrides, resolved into a whole kit by
//! `layers` as it is read ([`Kit::read`]). A new material is a new file, not code.

use crate::Kind;
use crate::cost::{Amount, Bill, Rules, Terms};
use crate::layout::roles::Role;
use crate::layout::roofs::RoofPart;
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
    /// no roof's material (D241).
    pub roof_kind: Option<u16>,
    /// A block kind's surface: the radius its outside edges are rounded to, and how far
    /// its material's relief stands out (`surface.rs`).
    pub bevel: f32,
    pub relief: f32,
    /// How each long member is shaped, and how much one differs from the next (D200).
    pub timber: Timber,
    /// A roof kind's: how its roof is made, its frame, battens, covering and trims.
    pub roof: Roofing,
    /// Its materials' textures (D14): where each is, how many metres across it is, and how
    /// deep its parallax reads; a material with none is drawn in its plain colour.
    pub textures: BTreeMap<Material, Texture>,
    pub colours: KitColours,
    /// What its cubes cost to build (D139).
    pub cost: KitCost,
    /// How it stands up to harm and what is left when it falls (D193): its pattern's, unless
    /// its file says otherwise.
    pub toughness: KitToughness,
    /// How each of its materials breaks as its cubes lose their health (D305): by the
    /// structure its pattern lays (a block wall at its joints, timber along its grain, a
    /// covering a shingle at a time), as its pattern's file says; a material left out takes
    /// the shared crack and chip mask.
    pub damage: BTreeMap<Material, Breaks>,
    /// How each exposed edge of its cubes is dressed by its role (D215, `layout::roles`):
    /// its pieces, what each role takes, and its masks.
    pub dress: Dressing,
    /// How it meets other kinds where their cubes touch (D232): left out, as its pattern
    /// meets them ([`Meets::of`]).
    pub meets: Option<Meets>,
    /// What a block kind's surface's colour is multiplied by (sRGB): its earth or its daub
    /// warmed or lightened from its texture's, a grade a kit (docs/art-style.md).
    pub tint: [f32; 3],
    /// How a block kind's earth was packed (D236): its face bulging where it was pressed by
    /// hand and its lifts standing out as ridges, in its shape, not only its texture; left
    /// out, none.
    pub packed: Option<Packing>,
    /// A block kind framed in timber (D236, `layout::frames`): its cubes the infill, drawn as
    /// their surface, and a frame of its members laid over them by rule; left out, none.
    pub framing: Option<Framing>,
}

/// How a block kind's earth was packed, in its surface's shape (D236): lumps where it was
/// pressed by hand, and each lift rounded out between the seams where the next was rammed
/// on it, the seams following its masks' `lift` and wandering as its strata do.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packing {
    /// How far its lumps stand out or sink, either way, in metres, and how far across one
    /// is.
    pub lumps: f32,
    pub lump: f32,
    /// How far each lift's middle stands out from the seams either side, in metres.
    pub ridge: f32,
    /// How far a whole lift stands out or back from the next, either way, in metres: each
    /// rammed a little proud or shy of the one under it, so a wall's arrises and its outline
    /// against the sky step from lift to lift.
    #[serde(default)]
    pub step: f32,
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
    /// The module of the kit's pieces each member is (D258), scaled across to its size, as
    /// its pattern names it; none by default.
    pub module: String,
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
            module: String::new(),
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
    /// Its pattern's pieces' file under `assets/`, as `tools/kits/pieces.py` makes it,
    /// shared by every material of the pattern; and a material's own, over it by name (D290).
    pub pieces: Option<String>,
    pub own: Option<String>,
    /// What each role is dressed with; a role not named is left as the cubes are.
    pub roles: BTreeMap<Role, Dress>,
    /// What its pieces' colour is multiplied by (sRGB): its dressed stone greyed towards the
    /// rough stone round it, one grade a kit (docs/art-style.md).
    pub tint: [f32; 3],
    pub masks: Masks,
    /// Its pieces, once read ([`Dressing::read`]).
    #[serde(skip)]
    pub made: Option<Arc<crate::dress::Pieces>>,
    /// Whether its modules are drawn at their far budget (D258): set for a section drawn a
    /// few metres off and further, never read from a file.
    #[serde(skip)]
    pub far: bool,
    /// Whether it is drawn far off, past where the far budget begins: its modules' copies
    /// drawn `modules::FAR_REPEAT` times their length. Set as `far` is.
    #[serde(skip)]
    pub far_off: bool,
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
    /// Corner stones up the edge, a stone a course, made from the material's own wall
    /// (`dress::corner`): dressed blocks cut from its own texture and height map, quoins
    /// long and short by turns, jamb stones long and short along the wall, standing `proud`
    /// of the wall's faces with their arrises bevelled by `bevel`, a `joint` round each, all
    /// in metres. `long` gives each variant's length along its first face and its second,
    /// and `short`, if any, the variants laid by turns with them (jamb stones); with none,
    /// the stones turn each course instead (quoins). Nothing modelled is shared: every
    /// material's corners match its wall (D215 as amended).
    Corners {
        long: Vec<[f32; 2]>,
        #[serde(default)]
        short: Vec<[f32; 2]>,
        proud: f32,
        bevel: f32,
        #[serde(default = "joint")]
        joint: f32,
    },
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
    /// Nothing: the role left as the cubes are, as a material's pack turns off a role its
    /// pattern dresses (the escape hatch, docs/proposals/construction-002.md).
    Off,
}

fn joints() -> bool {
    true
}

/// A corner stone's joint, all round it, in metres.
fn joint() -> f32 {
    0.007
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
    /// How much paler a block kind's wall is from a storey's height above the ground up, kept
    /// dry under the eaves and out of reach of the splash, its face weathered pale, as a
    /// fraction, rising to it over so many metres below; its foot meanwhile is darkened by
    /// the damp splashed up it (`dirt`).
    pub dry: f32,
    pub dry_reach: f32,
    /// How high a block kind's wall is darkened over its own foot, where the rain splashes
    /// up off its plinth and the ground, in metres, its top wandering, to `dirt_tint`; and
    /// how dark the streaks are that rain draws down it under a sill or an open top, as a
    /// fraction, and how far down they run, in metres.
    pub splash: f32,
    pub streaks: f32,
    pub streak_reach: f32,
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
            dry: 0.0,
            dry_reach: 1.2,
            splash: 0.0,
            streaks: 0.0,
            streak_reach: 1.5,
        }
    }
}

impl Default for Dressing {
    /// Nothing dressed.
    fn default() -> Self {
        Dressing {
            pieces: None,
            own: None,
            roles: BTreeMap::new(),
            tint: [1.0; 3],
            masks: Masks::default(),
            made: None,
            far: false,
            far_off: false,
        }
    }
}

impl Dressing {
    /// How many times its length each copy of its modules is drawn.
    pub fn repeat(&self) -> f32 {
        if self.far_off {
            crate::modules::FAR_REPEAT
        } else {
            1.0
        }
    }

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

    /// Reads its pieces' files, `read` giving a file under `assets/` as text: its
    /// pattern's, shared, and its own over them, a profile, corner set or module of a name
    /// its own having it in place of the pattern's.
    pub fn read(&mut self, read: &dyn Fn(&str) -> Result<String, String>) -> Result<(), String> {
        let file = |path: &str, shared: bool| -> Result<crate::dress::Pieces, String> {
            let text = read(path)?;
            let mut pieces: crate::dress::Pieces =
                ron::from_str(&text).map_err(|why| format!("{path}: {why}"))?;
            crate::modules::clean_all(&mut pieces);
            for module in pieces.modules.values_mut() {
                module.shared = shared;
            }
            Ok(pieces)
        };
        let mut made = match &self.pieces {
            Some(path) => Some(file(path, true)?),
            None => None,
        };
        if let Some(path) = &self.own {
            let own = file(path, false)?;
            let all = made.get_or_insert_with(Default::default);
            all.profiles.extend(own.profiles);
            all.modules.extend(own.modules);
        }
        self.made = made.map(Arc::new);
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
    /// How much darker a wall's log is toward the grooves where it meets the logs above
    /// and below, as the shade there is baked into a model's colour: the most, under it.
    pub groove: f32,
    /// How many knots a metre of a round log has, on average, where its branches were
    /// trimmed: each a low swell a few centimetres across, dark at its heart.
    pub knots: f32,
    /// Whether its face's texture is weathered where a log is exposed (`textures.ts blend
    /// --exposed`, D262): then laid once round each lying log, its middle on top and its
    /// first quarter to the weather, so its bark is lost on the log's top and weather face,
    /// and the log weathers paler toward its ends.
    pub exposed: bool,
    /// How much each member differs from the next.
    pub variation: Variation,
    /// A module of the kit's pieces (D251, D258) its wall's whole logs are drawn as in place
    /// of generated ones, if the developers choose hand-made logs; none, generated (D200).
    pub module: Option<String>,
}

/// How a roof kind's roofs are made (`layout::roofs`, `roofing`, D258): its covering the roof
/// cubes' surface in its `Covering` material, so thick and pushed out by its height, its rows
/// on level lines; each line its neighbours make and its frame, a module of its pieces.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Roofing {
    /// How many courses one copy of the covering's texture holds up the slope: with its
    /// size, how much of each course shows (its exposure).
    pub rows: f32,
    /// Where a course's lower edge falls on the texture, as a fraction of it down from its
    /// top, so the texture's rows sit on the courses.
    pub phase: f32,
    /// How thick the covering is over the roof, in metres: a few centimetres of shakes or
    /// slate, a foot of thatch.
    pub thick: f32,
    /// How much further its texture's height pushes it out, from its lowest row to its
    /// highest, in metres: each course's butt standing out as a step.
    pub relief: f32,
    /// What the covering's colour is multiplied by (sRGB).
    pub tint: [f32; 3],
    /// How thick its rafters are, and its battens' width and thickness, in metres: how far
    /// under the roof each is laid.
    pub rafter: f32,
    pub batten: (f32, f32),
    /// What each line of the roof and its frame is: a module of the kit's pieces.
    pub parts: BTreeMap<RoofPart, Part>,
}

/// A part of a roof or a frame (D258): the module of the kit's pieces laid along its line,
/// its material, its ends' material if they show another (a member's end grain), and what
/// its colour is multiplied by.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub module: String,
    pub material: Material,
    #[serde(default)]
    pub ends: Option<Material>,
    #[serde(default = "white")]
    pub tint: [f32; 3],
    /// For an eave: whether its piece lies on the roof's plane, as thatch's cut lip does,
    /// rather than hanging plumb, as a fascia does.
    #[serde(default)]
    pub on_roof: bool,
}

fn white() -> [f32; 3] {
    [1.0; 3]
}

impl Default for Roofing {
    /// Shakes, with no pieces: a test's roof is its covering alone.
    fn default() -> Self {
        Roofing {
            rows: 6.0,
            phase: 0.145,
            thick: 0.03,
            relief: 0.0,
            tint: [1.0, 0.97, 0.93],
            rafter: 0.14,
            batten: (0.06, 0.025),
            parts: BTreeMap::new(),
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
/// fraction either way, how much lighter or darker it is, how much warmer or cooler, and
/// how much it is mottled along its length.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Variation {
    pub thickness: f32,
    pub shade: f32,
    pub warmth: f32,
    /// How much lighter and darker it is in long patches along it, so no member is one
    /// flat colour and its texture's repeat is broken up.
    pub mottle: f32,
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

/// How a kind stands up to harm (D193, [`crate::cost::Toughness`]): how much damage a cube
/// takes before it is destroyed; the share of its cost a repair from nothing takes,
/// (numerator, denominator), a power of two no finer than a sixteenth, else the default half;
/// and how it breaks when it comes down (`fragments`, D305).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KitToughness {
    pub health: u16,
    pub repair: (u64, u64),
    pub breaks: crate::fragments::Breaks,
}

impl Default for KitToughness {
    /// Pine logs', as `patterns/coursed.ron` has it.
    fn default() -> Self {
        KitToughness {
            health: 400,
            repair: (1, 2),
            breaks: crate::fragments::Breaks::Lengths,
        }
    }
}

impl KitToughness {
    /// As the rules hold it, a share that is not a power of two no finer than a sixteenth,
    /// or more than the whole, taken as half.
    pub fn rules(&self) -> crate::cost::Toughness {
        let share = |(num, den): (u64, u64)| {
            let amount = Amount::new(num, den.max(1));
            let (_, den) = amount.parts();
            if den.is_power_of_two() && den <= 16 && amount <= Amount::whole(1) {
                amount
            } else {
                Amount::new(1, 2)
            }
        };
        crate::cost::Toughness {
            health: self.health.max(1),
            repair: share(self.repair),
        }
    }
}

/// How a kit lays its walls (D160: a few patterns in code, kits as data).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Pattern {
    /// Courses of long members, crossing at the corners (`layout::runs`).
    #[default]
    Logs,
    /// Blocks (`layout::blocks`): stone, plaster or earth, drawn as the cubes' own
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
            rules.toughness.insert(kit.kind(), kit.toughness.rules());
        }
        rules
    }

    /// The roof kind a roof over walls of a kind is laid in (D241): the kind itself if it is
    /// a roof's, else its kit's `roof_kind`, else the first roof kit's, else the first roof
    /// kind there can be.
    pub fn roof_over(&self, kind: Kind) -> Kind {
        self.roof_for(kind).unwrap_or(Kind::FIRST_ROOF)
    }

    /// The roof kind a roof over walls of a kind is laid in, if any kit has one: as
    /// [`Kits::roof_over`], which falls back on the first roof kind.
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
    /// For an atlas of pictures in a grid, such as a log's sawn ends, how many cells across
    /// and down it: each piece draws one, by its seed. One, the whole texture, if not given.
    #[serde(default = "one")]
    pub cells: u32,
    /// The texture set its face wears down to, a folder under `assets/` (a log's bark to
    /// its bare, silvered wood): shown where a piece's corners say it is worn, through its
    /// own height's low places first, as a material blends its layers by their heights.
    #[serde(default)]
    pub worn: Option<String>,
}

fn one() -> u32 {
    1
}

/// A rough natural material's reflectance (see [`Texture::reflectance`]).
pub fn matte() -> f32 {
    0.35
}

/// A kit's plain colours: `light`, the dust it throws up as it comes down
/// (`terra_game::build::loss`), and `chinking`, the daub between a coursed kit's courses,
/// which tints its texture. What a file leaves out is pine's.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KitColours {
    pub light: [f32; 3],
    pub chinking: [f32; 3],
}

impl Default for KitColours {
    fn default() -> Self {
        KitColours {
            light: [0.74, 0.56, 0.37],
            chinking: [0.66, 0.62, 0.58],
        }
    }
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
            tint: [1.0, 0.92, 0.8],
            end_tint: [0.58, 0.5, 0.42],
            groove: 0.4,
            knots: 0.8,
            exposed: true,
            variation: Variation::default(),
            module: None,
        }
    }
}

impl Default for Variation {
    fn default() -> Self {
        Variation {
            thickness: 0.06,
            shade: 0.16,
            warmth: 0.05,
            mottle: 0.1,
        }
    }
}

/// How a material breaks, by the structure of what it is (D305): drawn by the texture
/// arrays' shader from each corner's lost health, the same for every material of a pattern.
/// Where it has cells (stones, shingles), they are the islands its height map's lowest
/// `share` of texels, its joints, part.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Breaks {
    /// Laid stones in mortar: the joints open first and darken, then the stones' edges chip
    /// back to their fresh face, then whole stones fall out, leaving their hollows `depth`
    /// metres deep in the wall's surface. No crack runs through a stone.
    Joints { share: f32, depth: f32 },
    /// Timber: splits run along the grain, `splits` of them round or across a member's
    /// face, opening and lengthening as more is lost, and chunks gouge out of it showing
    /// the fresh wood; a log or beam, harmed past half, loses whole chunks of it `chunks`
    /// metres deep at most, out of its outline, and its sawn ends splinter (none if 0).
    Grain {
        splits: f32,
        #[serde(default)]
        chunks: f32,
    },
    /// A member's sawn end: checks open out from its heart and its rim splinters.
    Ends,
    /// A covering of shingles, slates or shakes: one cracks, then whole ones go missing,
    /// what is under them (its battens and rafters) showing through.
    Shingles { share: f32 },
    /// Daub and mud: crumbling in patches, darker and rougher, bits of it gone.
    Crumbles,
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
            toughness: KitToughness::default(),
            damage: BTreeMap::new(),
            dress: Dressing::default(),
            colours: KitColours::default(),
            meets: None,
            framing: None,
            packed: None,
            tint: [1.0; 3],
        }
    }
}

/// Where the materials' files are, under `assets/`: one a material, each over its pattern's
/// file and its pack (D290, `layers`).
pub const MATERIALS: &str = "data/kits";

impl Kit {
    /// A material's file read as the game reads it (D290): resolved over its pattern and its
    /// pack (`layers::resolve`), checked as a kit, and its dressed pieces read. `read` gives
    /// a file under `assets/` as text; a mistake names the file.
    pub fn read(path: &str, read: &dyn Fn(&str) -> Result<String, String>) -> Result<Kit, String> {
        let text = read(path)?;
        let whole = crate::layers::resolve(&text, read).map_err(|why| format!("{path}: {why}"))?;
        let mut kit: Kit = ron::from_str(&whole).map_err(|why| format!("{path}: {why}"))?;
        kit.dress
            .read(read)
            .map_err(|why| format!("{path}: {why}"))?;
        Ok(kit)
    }

    /// Every material's file under `assets/` (`MATERIALS`), by its name (its file's stem), in
    /// order of name; `list` gives the names of the files in a folder under `assets/`.
    pub fn read_all(
        list: &dyn Fn(&str) -> Result<Vec<String>, String>,
        read: &dyn Fn(&str) -> Result<String, String>,
    ) -> Result<Vec<(String, Kit)>, String> {
        let mut names: Vec<String> = list(MATERIALS)?
            .into_iter()
            .filter_map(|file| file.strip_suffix(".ron").map(str::to_string))
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|name| {
                Ok((
                    name.clone(),
                    Kit::read(&format!("{MATERIALS}/{name}.ron"), read)?,
                ))
            })
            .collect()
    }

    /// Every material's file in an `assets` folder on disk, as [`Kit::read_all`] reads them.
    pub fn read_folder(assets: &std::path::Path) -> Result<Vec<(String, Kit)>, String> {
        let list = |folder: &str| {
            std::fs::read_dir(assets.join(folder))
                .map_err(|why| format!("{folder}: {why}"))
                .map(|entries| {
                    entries
                        .flatten()
                        .filter(|entry| entry.path().is_file())
                        .map(|entry| entry.file_name().to_string_lossy().to_string())
                        .collect()
                })
        };
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        Kit::read_all(&list, &read)
    }

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

    /// How deep a material of it reads by parallax, as a fraction of its texture's width:
    /// as its texture says, or a round log's bark by its timber's relief, which is drawn by
    /// its maps and not its points (D284).
    pub fn depth(&self, material: Material) -> f32 {
        let Some(texture) = self.textures.get(&material) else {
            return 0.0;
        };
        let bark = material == Material::Log
            && self.pattern == Pattern::Logs
            && self.timber.profile == Profile::Round;
        if texture.depth > 0.0 || !bark {
            return texture.depth;
        }
        self.timber.relief / texture.metres[0].max(1e-3)
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
        let read = |path: &str| {
            std::fs::read_to_string(assets().join(path)).map_err(|why| format!("{path}: {why}"))
        };
        let mut kit = Kit::read(&format!("{MATERIALS}/{name}"), &read).unwrap();
        kit.dress.made = None;
        kit
    }

    /// Every Rust file of a crate's source, outside its tests.
    fn sources(crate_dir: &std::path::Path) -> Vec<(std::path::PathBuf, String)> {
        let mut out = Vec::new();
        let mut folders = vec![crate_dir.join("src")];
        while let Some(folder) = folders.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                if path.is_dir() {
                    if name != "tests" {
                        folders.push(path);
                    }
                } else if name.ends_with(".rs") && name != "tests.rs" {
                    let text = std::fs::read_to_string(&path).unwrap();
                    let code = text.split("#[cfg(test)]").next().unwrap_or("").to_string();
                    out.push((path, code));
                }
            }
        }
        out
    }

    #[test]
    fn no_game_or_layout_code_names_a_material_or_its_number() {
        // D290: kits are cleanly designed so none is customised by the game. What a
        // material is and does comes from its pattern, its pack and its file; the code that
        // lays and draws them, and the game's, names none (as a string: a material's or a
        // pack's file name) and branches on no material's number (a kind written as a
        // number, but for the two that mark the ranges, nothing and the first roof).
        let mut names: Vec<String> = Vec::new();
        for folder in [MATERIALS, crate::layers::PACKS] {
            for entry in std::fs::read_dir(assets().join(folder)).unwrap().flatten() {
                if let Some(stem) = entry.file_name().to_string_lossy().strip_suffix(".ron") {
                    names.push(stem.to_string());
                }
            }
        }
        assert!(names.len() >= 10, "{names:?}");
        // And no piece by its name, so none is nudged, scaled or fixed up by code: a piece
        // lands right by its contract (D303), held by the kit checker.
        let pieces = assets().join("data/kits/pieces");
        for entry in std::fs::read_dir(&pieces).unwrap().flatten() {
            let text = std::fs::read_to_string(entry.path()).unwrap();
            for line in text.lines() {
                let line = line.trim();
                if let Some(name) = line
                    .strip_prefix('"')
                    .and_then(|rest| rest.strip_suffix("\": ("))
                {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        names.dedup();
        assert!(names.iter().any(|n| n == "ridge"), "{names:?}");
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut found = Vec::new();
        for krate in [root.to_path_buf(), root.join("../terra_game")] {
            for (path, code) in sources(&krate) {
                for (n, line) in code.lines().enumerate() {
                    let line = line.trim();
                    if line.starts_with("//") {
                        continue;
                    }
                    for name in &names {
                        if line.contains(&format!("\"{name}\""))
                            || line.contains(&format!("\"{name}.ron\""))
                        {
                            found.push(format!(
                                "{}:{}: names {name}: {line}",
                                path.display(),
                                n + 1
                            ));
                        }
                    }
                    // A material's id compared with a number.
                    let by_id = ["id == ", "id != ", "id() == ", "id() != "]
                        .iter()
                        .any(|op| {
                            line.match_indices(op).any(|(at, _)| {
                                line[at + op.len()..]
                                    .chars()
                                    .next()
                                    .is_some_and(|c| c.is_ascii_digit())
                            })
                        });
                    if by_id {
                        found.push(format!(
                            "{}:{}: a material by its id: {line}",
                            path.display(),
                            n + 1
                        ));
                    }
                    let numbered = line.match_indices("Kind(").any(|(at, _)| {
                        let before = line[..at].chars().last();
                        let after = line[at + 5..].chars().next();
                        !before.is_some_and(|c| c.is_alphanumeric() || c == '_')
                            && after.is_some_and(|c| c.is_ascii_digit())
                    });
                    // The ranges kinds are numbered in (`Kind::PLAIN`, `FIRST_ROOF`, `LAST`).
                    let ranges = path.ends_with("cubes.rs") && line.starts_with("pub const");
                    if numbered && !ranges {
                        found.push(format!(
                            "{}:{}: a kind by number: {line}",
                            path.display(),
                            n + 1
                        ));
                    }
                }
            }
        }
        assert!(
            found.is_empty(),
            "code customised by material:\n{}",
            found.join("\n")
        );
    }

    #[test]
    fn a_materials_toughness_is_its_patterns_unless_it_says_otherwise() {
        // D305: each pattern says how a cube of it stands up to harm, and a material may
        // say otherwise; a repair's share is half unless said.
        let health = |name: &str| shipped(name).toughness.rules().health;
        let stone = shipped("stone.ron").toughness.rules();
        assert_eq!(stone.repair, Amount::new(1, 2));
        assert!(health("stone.ron") > health("logs.ron"));
        assert!(health("logs.ron") > health("wattle.ron"));
        assert!(health("wattle.ron") > health("thatch.ron"));
        // How each breaks: by its pattern, earth as its own file says.
        use crate::fragments::Breaks;
        let breaks = |name: &str| shipped(name).toughness.breaks;
        assert_eq!(breaks("stone.ron"), Breaks::Chunks);
        assert_eq!(breaks("logs.ron"), Breaks::Lengths);
        assert_eq!(breaks("wattle.ron"), Breaks::Bits);
        assert_eq!(breaks("thatch.ron"), Breaks::Bits);
        assert_eq!(breaks("earth.ron"), Breaks::Crumbs);
        // A share that is not a power of two, or more than the whole, is taken as half.
        let odd = KitToughness {
            health: 0,
            repair: (5, 4),
            ..KitToughness::default()
        }
        .rules();
        assert_eq!((odd.health, odd.repair), (1, Amount::new(1, 2)));
        let third = KitToughness {
            health: 10,
            repair: (1, 3),
            ..KitToughness::default()
        };
        assert_eq!(third.rules().repair, Amount::new(1, 2));
    }

    #[test]
    fn every_material_is_a_small_file_over_its_pattern_and_pack() {
        // D290: a material fits on one screen, its pattern and its pack saying the rest.
        let folder = assets().join(MATERIALS);
        let mut seen = 0;
        for entry in std::fs::read_dir(&folder).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let lines = text
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
                .count();
            assert!(lines <= 20, "{}: {lines} lines", path.display());
            assert!(
                text.contains("pattern: \"") && text.contains("pack: \""),
                "{}",
                path.display()
            );
            seen += 1;
        }
        assert!(seen >= 9, "{seen}");
    }

    #[test]
    fn the_default_kit_is_pine_as_its_file_has_it() {
        // What a kit's file leaves out is the default's, which is to be logs.ron's but
        // for what it lays with: its textures and cost; and for how it is dressed,
        // its ends run past the corners and its role members, so tests lay plain logs
        // inside their cubes; and how it meets other kinds, which the default leaves to
        // its pattern, so a test's stone kit meets as stone, and the file says as logs; and
        // the roof over it, which the file names; and how it breaks, which its pattern says.
        let file = shipped("logs.ron");
        assert_eq!(file.meets, Some(Meets::of(Pattern::Logs)));
        let pine = Kit {
            textures: BTreeMap::new(),
            cost: KitCost::default(),
            overhang: 0.0,
            dress: Dressing::default(),
            meets: None,
            roof_kind: None,
            damage: BTreeMap::new(),
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
