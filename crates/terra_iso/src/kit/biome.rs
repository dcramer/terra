//! A biome's file, `crates/terra_iso/biomes/<name>.ron` (D634): its layout, light, ground, scatter,
//! decals, water, weather, effects and life, and what the judging loop compares it with.
//! Unknown fields are refused, so a typo fails the run with its line rather than being
//! ignored (kit/mod.rs's `read` says the fix).
//!
//! Fields the kit does not draw yet are parsed and carried (empty or `None` in the three
//! biomes, whose layouts draw their own until the kit's job for it lands); each says which
//! job of the iso-kit line draws it. Nothing assumes a genre: a neon street's wet asphalt
//! and sodium lamps are the same layers, water and lights as a ruin's moss and braziers.

use super::light::{Colour, Overrides};
use serde::Deserialize;

#[derive(Deserialize, Debug, Clone, PartialEq, bevy::prelude::Resource)]
#[serde(deny_unknown_fields)]
pub struct Biome {
    /// What its stills are judged as, the kinds of reference they stand beside.
    pub kind: Vec<Kind>,
    /// Its references' folder, `~/.cache/terra-review/iso-kit/refs/<refs>/` (iso-kit j2).
    pub refs: String,
    pub layout: Layout,
    /// The street's road, kerbs, fronts, lamps, signs and props, when the layout is
    /// `Street` (kit/street.rs).
    #[serde(default)]
    pub street: Option<super::street::Street>,
    pub light: LightRef,
    /// The ground's layers and how they blend (kit/ground.rs draws it, iso-kit j6).
    #[serde(default)]
    pub ground: Ground,
    /// Props and plants strewn by rule (kit/scatter.rs, iso-kit j13).
    #[serde(default)]
    pub scatter: Vec<Scatter>,
    /// Flat marks laid on the ground (iso-kit j4).
    #[serde(default)]
    pub decals: Vec<Decal>,
    /// Standing and wet water (iso-kit j5).
    #[serde(default)]
    pub water: Option<Water>,
    /// Wind, rain, snow and mist (iso-kit j7).
    #[serde(default)]
    pub weather: Option<Weather>,
    /// Effect presets the scene plays (iso-kit j6).
    #[serde(default)]
    pub effects: Vec<Effect>,
    /// What lives and moves in it (iso-kit j7).
    #[serde(default)]
    pub life: Vec<Life>,
    /// Models' materials regraded at spawn: sheen, metal, colour (kit/finish.rs, iso-kit
    /// j60). None by default: a model looks as its file made it.
    #[serde(default)]
    pub finishes: Vec<super::finish::Finish>,
    /// The scene captured into a cubemap from one point and lit back as the view's
    /// environment map, for glass, paint and gloss to mirror (kit/reflect.rs, uc-reflect j1).
    /// None by default: no image-based light.
    #[serde(default)]
    pub reflect: Option<super::reflect::Reflections>,
    /// Play mode's roster: the foes about the hero (play.rs spawns them on a street; mob AI
    /// and encounters read them, never a list of their own). None by default.
    #[serde(default)]
    pub foes: Vec<Foe>,
}

/// The kind of a reference and a still.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Day,
    Dusk,
    Night,
    Rain,
    Snow,
    Water,
    Interior,
}

/// How the scene is built: the land's shape, where the fight stands and the camera looks.
/// For now each runs a biome's own modules (ruin/, wood/, marsh/); the kit's layouts replace
/// them as its jobs land (D634).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// A ruined court on flat paving between fallen walls (ruin/).
    Ruin,
    /// The glade's wood about a clearing by a pond (wood/ over the glade's modules).
    Forest,
    /// Mud banks between open channels (marsh/).
    Marsh,
    /// A kerbed road between pavements and building fronts, as the biome's `street` says
    /// (kit/street.rs): any town's, a city's at night or a wasteland's.
    Street,
}

/// The light: a preset in `biomes/light/`, and the numbers this biome changes of it.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LightRef {
    pub preset: String,
    #[serde(default)]
    pub overrides: Overrides,
}

/// The ground (kit/ground.rs): its layers, the light and hue its floor drifts between over
/// tens of metres, and the scorched points (a camp fire's ash and char).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Ground {
    /// Bottom first; up to eight, height-blended where their masks overlap.
    pub layers: Vec<Layer>,
    /// The broad variation's darkest and brightest, as factors on the blended colour.
    pub broad: (f32, f32),
    /// Up to four scorched points, x and z.
    pub scorch: Vec<(f32, f32)>,
    /// A laid floor on the layout's paving (kit/laid.rs), if it has one.
    pub laid: Option<Laid>,
}

impl Default for Ground {
    fn default() -> Self {
        Ground {
            layers: Vec::new(),
            broad: (0.78, 1.15),
            scorch: Vec::new(),
            laid: None,
        }
    }
}

/// A laid floor (kit/laid.rs): slabs, pavers, cobbles, bricks or kerbs, a scan of laid
/// units (a folder of `assets/materials/` with its colour, normal and ARM maps) at its true
/// size, drawn straight so its joints run true, weathered in them: grime packed in every
/// joint (the scan's occlusion marks them), hairline cracks, the detail grain, moss out of
/// the joints and over the units, and standing water, each where its mask lays it.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Laid {
    pub material: String,
    /// Metres the scan covers.
    pub metres: f32,
    /// The factor grime puts on the joints' colour (1 clean, 0 black).
    #[serde(default = "joints")]
    pub joints: f32,
    /// Sand and soil washed into the joints over their grime, 0 (none: dark joints) to 1
    /// (filled): an old court's joints are earth, not shadow (judge 261107-0250-ruin).
    #[serde(default)]
    pub sand: f32,
    /// The factor on the hairline cracks' colour (1: none). The moss still grows along the
    /// crack web whatever it is (laid.wesl's `in_joints`): the ruin's judges keep that web
    /// (round 261107-1015, without it lost slight in both orders).
    #[serde(default = "one")]
    pub cracks: f32,
    /// The detail grain (D362): metres it repeats, how strongly it shows (0 none).
    #[serde(default)]
    pub grain: (f32, f32),
    /// Where moss grows out of the joints and over the units (none when `None`).
    #[serde(default)]
    pub moss: Option<Mask>,
    /// Where water stands on the floor (none when `None`).
    #[serde(default)]
    pub wet: Option<Mask>,
    /// The scan's courses top to bottom in its texture, up to four: where each starts (v, 0
    /// to 1) and the joints across it (u, up to seven), so each unit is told apart.
    #[serde(default)]
    pub courses: Vec<(f32, Vec<f32>)>,
    /// How far each unit's value and roughness wander from the scan's, as fractions.
    #[serde(default)]
    pub spread: (f32, f32),
    /// Soil blown into drifts at two scales, thickest in the joints and at `feet`, and grit
    /// strewn over all, 0 (none) to 1.
    #[serde(default)]
    pub dirt: f32,
    /// How strongly the dirt's grit shows, 0 (none) to 1 (loose soil's grit over stone,
    /// the default): a swept street's paving wants little (at 1 its specks read as confetti).
    #[serde(default = "one")]
    pub grit: f32,
    /// Broad patches of damp stone, darker and glossier, 0 (none) to 1.
    #[serde(default)]
    pub damp: f32,
    /// Where the floor meets walls: grime, dirt and moss heavier there (none when `None`).
    #[serde(default)]
    pub feet: Option<Mask>,
    /// What lies on the floor at set places, as a road carries it (kit/laid.rs `marks`):
    /// painted lines and crossings, wheel paths' and oil's stains, patched repairs.
    #[serde(default)]
    pub marks: Vec<Mark>,
    /// How worn the units are, each by its own hash (none by default): split, tilted,
    /// broken and gone (kit/laid.rs `Wear`).
    #[serde(default)]
    pub wear: Wear,
    /// A city floor's grime over its units, blotches, banks and spots (`Grime`, drawn by
    /// kit/laid.wesl); none by default.
    #[serde(default)]
    pub grime: Grime,
}

/// An old floor's units, each told apart (`courses`) and worn its own way, as Substance
/// Designer's Tile Generator varies each tile by a random per cell (its size, luminance and
/// tilt) and texture bombing a cell's stamp (Glanville, GPU Gems ch. 20): the shares of
/// units split in two (and again, so the flags vary in size), broken across, and gone to
/// soil and moss, and how far a unit tilts (its normal, radians), so each catches the light
/// its own way. All 0: the scan's units as laid.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Wear {
    #[serde(default)]
    pub split: f32,
    #[serde(default)]
    pub tilt: f32,
    #[serde(default)]
    pub broken: f32,
    #[serde(default)]
    pub missing: f32,
    /// How wide (m) each unit's joints have opened, grimed or holding water (0: the scan's
    /// own, a hairline under a pixel at the camera); drawn at least a pixel wide and as
    /// dark as the share of the pixel it covers, so it neither vanishes nor shimmers.
    #[serde(default)]
    pub gap: f32,
    /// How far each unit's `gap` wanders from the rest, as a fraction (0: all alike), so an
    /// old floor's joints run wide and narrow, its edges chipped along them.
    #[serde(default)]
    pub uneven: f32,
    /// How far in (m) each piece's arris is worn round, shaded toward its joint (0: sharp).
    #[serde(default)]
    pub round: f32,
    /// How much of the floor is crazed by fatigue cracking, 0 (none) to 1: an old road
    /// worn through under its wheels, in stretches along it, a web of dark cracks round
    /// pieces a hand to a third of a metre across, each its own value and tilt (laid.wesl).
    #[serde(default)]
    pub crazed: f32,
}

/// A floor's grime (none by default), as a street's paving and road are in
/// photographs at night: dirt soaked into each unit along its joints, blotches of trodden
/// and spilt grime, some units far dirtier than the next, dark wet dirt banked at `feet`,
/// and spots (chewing gum on a pavement, oil drips on a road, lichen on an old court), each
/// its own size and tone; an abandoned court's earthier, with dead leaves drifted.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Grime {
    /// How far (m) dirt has soaked in from each unit's joints, darkest at the joint and
    /// patchy along it (0: none; needs `courses`).
    #[serde(default)]
    pub edges: f32,
    /// How dark the dirtiest blotches, units and the banks at `feet` fall, 0 (none) to 1.
    #[serde(default)]
    pub blotch: f32,
    /// Spots a square metre (0: none), up to 64 (a busy pavement has dozens).
    #[serde(default)]
    pub spots: f32,
    /// The largest spot's width (m); the rest down to a third of it.
    #[serde(default)]
    pub size: f32,
    /// How far its dirt is the soil's brown and the moss's dark green (an abandoned court's
    /// earth), 0 (the city's grey-black grime) to 1.
    #[serde(default)]
    pub earth: f32,
    /// How much of the stone lichen crusts, 0 (none) to 1: pale grey-green and ochre patches
    /// with crisp lobed edges, on some units and not others, never in a joint.
    #[serde(default)]
    pub lichen: f32,
    /// Dead leaves a square metre (0: none) where they drift: against the walls' feet
    /// (`feet`), in the broken corners and gone units, along the joints; up to 200.
    #[serde(default)]
    pub litter: f32,
    /// How much of the floor old tar covers, 0 (none) to 1: broad patches a metre to a few
    /// metres across, sealcoat and oil soaked in, dark and dry with crisp ragged edges, as
    /// on an old lot's asphalt (the wasteland's road read as one clean sheet without them;
    /// `damp`'s soft glossy patches read as wet in the desert, round 261107-1518).
    #[serde(default)]
    pub tar: f32,
}

/// A mark on a laid floor: a rectangle `size` metres (along its own x and z) centred `at`
/// (world x, z), turned `turn` degrees about up, repeated `count` times `every` metres
/// along its own x and z about its centre (a dashed line, a zebra's stripes); `amount` how
/// fresh its paint or how deep its stain (0 to 1).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub kind: MarkKind,
    pub at: (f32, f32),
    pub size: (f32, f32),
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub every: (f32, f32),
    #[serde(default = "once")]
    pub count: (u32, u32),
    #[serde(default = "one")]
    pub amount: f32,
}

/// What a mark is: road paint (white, yellow) worn in the wheel paths with the surface's
/// grain through it; a stain (tyres' rubber and dripped oil, darker and smoother, its edge
/// soft); a patch (a repair of fresh, darker surface, its edge sealed in a tar band).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    White,
    Yellow,
    Stain,
    Patch,
}

fn once() -> (u32, u32) {
    (1, 1)
}

fn joints() -> f32 {
    0.3
}

/// A ground layer: a scanned material (a layer of the `materials/ground` stack, by name) at
/// its true size, where its mask lays it, `amount` of the way (0 to 1). One without a
/// material only wets what lies under it (`wets`).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    #[serde(default)]
    pub material: Option<String>,
    pub mask: Mask,
    #[serde(default = "one")]
    pub amount: f32,
    /// Metres the scan covers, if not its true size (kit/ground.rs's `STACK`).
    #[serde(default)]
    pub metres: Option<f32>,
    /// A factor on the scan's colour: the scans are dark, photographed under cloud.
    #[serde(default = "white")]
    pub tint: (f32, f32, f32),
    /// Broken into drifts: (broad scale, fine scale, band from, band to); a band running
    /// down (0.65, 0.35) lays the drifts' inverse.
    #[serde(default)]
    pub breakup: Option<(f32, f32, f32, f32)>,
    /// How wet the ground reads where it lies, 0 to 1: darker and glossier.
    #[serde(default)]
    pub wets: f32,
    /// Projected from three sides (rock on the steep), not from above.
    #[serde(default)]
    pub triplanar: bool,
}

/// Where a layer lies: everywhere; by the layout's own map of that name (`Site("grass")`,
/// the place's authored weights); or by the kit's rules (kit/ground.rs): worn on paths,
/// mud low by water, wet nearer and lower, moss in shade and creases, snow on what faces
/// up, grime in hollows, steep where the land is.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Mask {
    Base,
    Site(String),
    Wear,
    Mud,
    Wet,
    Moss,
    Snow,
    Grime,
    Steep,
}

/// A scatter set (kit/scatter.rs): models (paths under `assets/`, one picked each) or the
/// kit's own plant clumps, strewn at up to `density` a square metre where `on` lies full
/// (everywhere when `None`) and its clumps gather; each scaled between `scale` evenly in
/// the logarithm (sizes several-fold apart), turned at random, leaned up to `tilt` degrees
/// and sunk `sink` metres; only `over` the water's face between its two heights (m) when
/// given. Sets with the same `seed` gather in the same clumps.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Scatter {
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub plant: Option<Plant>,
    pub density: f32,
    #[serde(default = "unit")]
    pub scale: (f32, f32),
    #[serde(default)]
    pub on: Option<Mask>,
    /// Noise at two scales (broad m, fine m) and how much it gathers the set, 0 (even) to 1
    /// (only in the clumps' hearts): never an even lawn.
    #[serde(default = "gather")]
    pub clump: (f32, f32, f32),
    #[serde(default)]
    pub tilt: f32,
    #[serde(default)]
    pub sink: f32,
    #[serde(default)]
    pub over: Option<(f32, f32)>,
    #[serde(default)]
    pub seed: u32,
}

fn gather() -> (f32, f32, f32) {
    (6.0, 1.5, 0.5)
}

/// The kit's plant clumps (wood/lawn.rs's cards): a kind, tinted between `tint`'s colours
/// by broad noise and darkened at random, and a flower in some (0 to 1).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Plant {
    pub kind: PlantKind,
    pub tint: Vec<(f32, f32, f32)>,
    #[serde(default)]
    pub flowers: f32,
    /// How far each texel of the card is taken to its own grey before the tint, 0 (the
    /// atlas's green as it is) to 1: dead grass from a green card. A multiplied tint cannot
    /// do it, as the card's green blades and tan tips need opposite factors (the
    /// wasteland's straw came out olive with orange tips, judges to 261107-1917-2).
    #[serde(default)]
    pub grey: f32,
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantKind {
    Tuft,
    Fern,
    Leaves,
    /// Matted dead grass and reed: blades lodged flat in layers with a few still leaning
    /// up, no broad leaves beneath (a marsh's or a moor's thatch).
    Mat,
}

/// Decals: a texture (a path under `assets/`) laid `count` times, `size` metres across
/// between its two numbers, where `on` lays.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Decal {
    pub texture: String,
    pub count: u32,
    pub size: (f32, f32),
    #[serde(default)]
    pub on: Option<Mask>,
}

/// Water (kit/water.rs draws it, iso-kit j9): its kind and its face's height; its body,
/// absorbing by depth (Beer-Lambert) and scattering a colour back; its soft shore; what
/// floats at its edge; its mirror; its ripples, rain rings and the lights' streaks. Every
/// field but `kind` has a default: dark still water with nothing floating.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Water {
    pub kind: WaterKind,
    #[serde(default)]
    pub level: f32,
    /// The colour its body scatters back, lit as the ground is (none: near black).
    #[serde(default)]
    pub deep: Option<Colour>,
    /// How fast it absorbs red, green and blue light (per metre): a channel halves over
    /// ln 2 / it of depth.
    #[serde(default = "absorb")]
    pub absorb: (f32, f32, f32),
    /// The depth (m) over which its face fades in at the bank, its colour and its mirror.
    #[serde(default = "shore")]
    pub shore: f32,
    /// A thin film's depth (m), for puddles and wet asphalt (0: the depth under the face).
    #[serde(default)]
    pub film: f32,
    /// Foam or scum matting the margin (none by default, as at night).
    #[serde(default)]
    pub scum: Option<Scum>,
    /// Round leaves floating in the open shallows (lily pads, fallen leaves).
    #[serde(default)]
    pub pads: Option<Pads>,
    #[serde(default)]
    pub mirror: Mirror,
    #[serde(default)]
    pub ripples: Ripples,
    #[serde(default)]
    pub streaks: Streaks,
    /// Rain's rings: how many a square metre at once (0: none).
    #[serde(default)]
    pub rain: f32,
}

fn absorb() -> (f32, f32, f32) {
    (9.0, 6.0, 5.0)
}

fn shore() -> f32 {
    0.08
}

/// Foam or scum: its colour, how much of the margin it mats (0..1), how deep (m) it reaches
/// out to and its clumps' size (m).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Scum {
    pub colour: Colour,
    pub amount: f32,
    #[serde(default = "reach")]
    pub reach: f32,
    #[serde(default = "clump")]
    pub size: f32,
}

fn reach() -> f32 {
    0.2
}

fn clump() -> f32 {
    2.4
}

/// Floating leaves: how many a square metre of open shallows, a leaf's radius (m), its colour.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Pads {
    pub count: f32,
    pub radius: f32,
    pub colour: Colour,
}

/// The mirror (a planar reflection): `lift` over Fresnel's two percent (an art cheat, 1 is
/// physical), how far (m) a unit of the ripples' slope moves what it mirrors, and the sky
/// it shows where nothing stands over it (none: the light preset's sky). `smear` streaks
/// it along the view, a fraction of the frame's height each way, as rough wet asphalt
/// does (full on a thin film, a quarter on a deep puddle; 0: a clean mirror).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Mirror {
    pub lift: f32,
    pub distort: f32,
    pub sky: Option<Colour>,
    pub nits: Option<f32>,
    pub smear: f32,
}

impl Default for Mirror {
    fn default() -> Self {
        Mirror {
            lift: 1.0,
            distort: 0.1,
            sky: None,
            nits: None,
            smear: 0.0,
        }
    }
}

/// The ripples: two scrolling normal maps, `fine` and `broad` the metres a tile spans,
/// `steep` their slope, `calm` how much stiller the margin is (0..1).
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Ripples {
    pub fine: f32,
    pub broad: f32,
    pub steep: f32,
    pub calm: f32,
}

impl Default for Ripples {
    fn default() -> Self {
        Ripples {
            fine: 1.3,
            broad: 5.5,
            steep: 0.25,
            calm: 0.7,
        }
    }
}

/// Each light's and the key's highlight off the rippled face, a streak toward the eye (an
/// anisotropic Beckmann lobe): its spread `along` the line to the eye and `across` it; the
/// glitter's facet `cell` (m) and its share; the lights' `gain`; the key's (the moon's)
/// `key` gain, the `calm` share of the ripples' tilt it sees taken away and its soft `peak`
/// (nits, 0: none); a glint drawn `dash` times as long across the streak as along it.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct Streaks {
    pub along: f32,
    pub across: f32,
    pub cell: f32,
    pub glitter: f32,
    pub gain: f32,
    pub key: f32,
    pub calm: f32,
    pub peak: f32,
    pub dash: f32,
}

impl Default for Streaks {
    fn default() -> Self {
        Streaks {
            along: 0.1,
            across: 0.025,
            cell: 0.09,
            glitter: 0.3,
            gain: 1.0,
            key: 0.4,
            calm: 0.5,
            peak: 160.0,
            dash: 3.0,
        }
    }
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    Pond,
    River,
    Puddles,
    /// A film over a hard floor (wet stone or asphalt), no depth.
    Wet,
}

/// Weather: the wind (x, z, metres a second), and rain, snow and mist from 0 to 1. Rain
/// falls as `drops` say and mist lies as its `layer` says (kit/weather.rs, iso-kit j19).
#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
#[serde(deny_unknown_fields, default)]
pub struct Weather {
    pub wind: (f32, f32),
    pub rain: f32,
    pub snow: f32,
    pub mist: f32,
    pub drops: super::weather::Drops,
    pub layer: super::weather::Layer,
}

/// An effect preset by name (iso-kit j6 names them), where it plays if not the fight's, and
/// how bright its lights are against the preset's (a preset tuned in the ruin's day floods
/// a night street's walls).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Effect {
    pub preset: String,
    #[serde(default)]
    pub at: Option<(f32, f32)>,
    #[serde(default = "one")]
    pub light: f32,
}

/// A kind of life (iso-kit j7 names them: insects, birds, beasts, drones) and how many.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Life {
    pub kind: String,
    pub count: u32,
}

/// One of play mode's foes (`--play`): a model of `assets/models/iso/` standing `at` (x, z) m
/// from the hero's start, facing him (play.rs). A new foe is a line of a biome's file, never
/// a list in code (D136).
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Foe {
    pub model: String,
    pub at: (f32, f32),
}

fn one() -> f32 {
    1.0
}

fn white() -> (f32, f32, f32) {
    (1.0, 1.0, 1.0)
}

fn unit() -> (f32, f32) {
    (1.0, 1.0)
}

impl Biome {
    /// `biomes/<name>.ron`.
    pub fn load(name: &str) -> Result<Biome, String> {
        let path = super::dir().join(format!("{name}.ron"));
        super::read(&path, "biome.rs")
    }

    /// Its light: the preset with its overrides.
    pub fn light(&self) -> Result<super::light::Light, String> {
        Ok(super::light::Light::load(&self.light.preset)?.with(&self.light.overrides))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every biome in biomes/ parses and its light preset loads, and the three the spike's
    /// scenes alias are there.
    #[test]
    fn every_biome_parses() {
        let mut names = Vec::new();
        for f in std::fs::read_dir(crate::kit::dir()).unwrap() {
            let p = f.unwrap().path();
            if p.extension().is_some_and(|e| e == "ron") {
                let name = p.file_stem().unwrap().to_str().unwrap().to_string();
                let b = Biome::load(&name).unwrap_or_else(|e| panic!("{e}"));
                b.light().unwrap_or_else(|e| panic!("{name}'s light: {e}"));
                names.push(name);
            }
        }
        for want in ["ruin", "forest", "marsh"] {
            assert!(names.iter().any(|n| n == want), "no biome {want}");
        }
    }

    const FULL: &str = r#"(
        kind: [Night, Rain, Water],
        refs: "neon-street",
        layout: Ruin,
        light: (preset: "city-night-rain", overrides: (lux: 30.0, curve: AgX)),
        ground: (layers: [
            (material: "asphalt", mask: Base),
            (material: "grime", mask: Grime, amount: 0.6), (mask: Wet, wets: 0.8),
        ], laid: (material: "concrete_pavement", metres: 1.8, wet: Site("gutter"))),
        scatter: [(models: ["models/bin.glb"], density: 0.05, scale: (0.9, 1.1), on: Some(Wear))],
        decals: [(texture: "decals/oil.png", count: 12, size: (0.5, 1.5))],
        water: Some((kind: Wet, deep: Some(Srgb(0.02, 0.02, 0.03)), film: 0.002, rain: 0.6, mirror: (lift: 2.0, nits: 20.0))),
        weather: (wind: (1.0, 0.5), rain: 0.8, mist: 0.3, drops: (fall: 8.0, bright: 0.03),
            layer: (height: 0.5, water: 2.0, colour: Srgb(0.6, 0.65, 0.7))),
        effects: [(preset: "steam", at: Some((2.0, -1.0)))],
        life: [(kind: "drone", count: 2)],
    )"#;

    /// A biome may name every field the schema has (a non-fantasy one here), and its
    /// overrides change its preset's numbers.
    #[test]
    fn a_full_biome_parses() {
        let b: Biome = crate::kit::parse(FULL, "full.ron", "biome.rs").unwrap();
        assert_eq!(b.ground.layers[1].mask, Mask::Grime);
        assert_eq!(b.weather.as_ref().unwrap().snow, 0.0);
        let lit = b.light().unwrap();
        assert_eq!(lit.key.lux, 30.0);
        assert_eq!(lit.curve, super::super::light::Curve::AgX);
    }

    /// A misspelt field fails with the file, its line, the field and the fix.
    #[test]
    fn a_bad_field_says_where_and_how_to_fix_it() {
        let bad = FULL.replace("weather:", "wether:");
        let e = crate::kit::parse::<Biome>(&bad, "biomes/bad.ron", "biome.rs").unwrap_err();
        for want in [
            "biomes/bad.ron:13:",
            "wether",
            "weather",
            "Fix:",
            "kit/biome.rs",
        ] {
            assert!(e.contains(want), "the error lacks {want:?}: {e}");
        }
        let missing = FULL.replace("refs: \"neon-street\",", "");
        let e = crate::kit::parse::<Biome>(&missing, "m.ron", "biome.rs").unwrap_err();
        assert!(e.contains("refs") && e.contains("add the field"), "{e}");
        let variant = FULL.replace("mask: Grime", "mask: Grim");
        let e = crate::kit::parse::<Biome>(&variant, "v.ron", "biome.rs").unwrap_err();
        assert!(
            e.contains("Grim") && e.contains("one of those names"),
            "{e}"
        );
    }
}
