//! The kit's laid floor (D634): slabs, pavers, cobbles, bricks or kerbs, a scan of laid units
//! at its true size drawn straight through StandardMaterial's own maps on the layout's
//! paving mesh, so its joints run true, with the weather laid over it per pixel as a decal
//! layer would be (laid.wesl): grime packed in every joint and moss growing out of them (the
//! scan's own occlusion marks the joints), hairline cracks, the detail grain (D362), and
//! standing water, darker and near-mirror smooth (Lagarde, "Water drop 2b: dynamic rain and
//! its effects", 2013). A biome's `ground.laid` says which scan and how weathered; where moss
//! and water lie are the layout's maps by name, carried per vertex (`Place::sites` here:
//! the second UV's x and y, the colour's alpha). A ruin's court and a city's pavement are
//! the same code.

use super::biome::{Laid, Mark, MarkKind, Mask};
use super::ground::STACK;
use crate::scene;
use bevy::asset::embedded_asset;
use bevy::image::{ImageArrayLayout, ImageLoaderSettings};
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;

pub type LaidMaterial = ExtendedMaterial<StandardMaterial, LaidExt>;

pub fn plugin(app: &mut App) {
    embedded_asset!(app, "laid.wesl");
    app.add_plugins(MaterialPlugin::<LaidMaterial>::default());
}

/// The source the shader reads for "everywhere" and "nowhere" (`source` in laid.wesl).
const ONE: f32 = 8.0;
const NONE: f32 = 9.0;

/// The floor's weathering as the shader reads it.
#[derive(Clone, Copy, Debug, Default, Reflect, ShaderType)]
pub struct Params {
    /// x the joints' grime factor, y the cracks' factor, z the grain's metres, w its strength.
    pub look: Vec4,
    /// x the moss's source, y the water's (`source`), z the moss scan's metres, w the
    /// wall feet's source.
    pub masks: Vec4,
    /// x the units' value spread, y their roughness spread, z the dirt, w the damp.
    pub vary: Vec4,
    /// Each course: x where it starts (v), y to w its first three joints (u; 2 none).
    pub courses: [Vec4; COURSES],
    /// Each course's joints four to seven.
    pub joints: [Vec4; COURSES],
    /// x the courses (0: the units are not told apart), y the marks, z the grit, w the
    /// joints' sand.
    pub shape: Vec4,
    /// The units' wear (biome.rs `Wear`): x the share split, y the tilt (radians), z the
    /// share broken, w the share gone.
    pub wear: Vec4,
    /// x how wide (m) the units' joints have opened (`Wear.gap`, 0: the scan's own), y how
    /// far each unit's width wanders (`Wear.uneven`), z how far in (m) its arris is worn
    /// round (`Wear.round`).
    pub seams: Vec4,
    /// Each mark's centre (x, z) and half size.
    pub mark_at: [Vec4; MARKS],
    /// Each mark's turn's cos and sin and its repeats' spacing.
    pub mark_turn: [Vec4; MARKS],
    /// How many repeats each side of a mark's middle, its kind, its amount.
    pub mark_rep: [Vec4; MARKS],
}

/// Most courses and joints a course a scan's units are told apart by (six: a street's
/// flags in a running bond, iso-kit j49).
pub const COURSES: usize = 6;
const JOINTS: usize = 7;
/// Most marks a laid floor carries (laid.wesl's arrays are as long): a city road's lines,
/// crossings, patches and stains (iso-kit j46: 16 left no room for its oil and trenches).
pub const MARKS: usize = 24;

/// How far a point of the floor (world x, z) lies outside a mark's nearest repeat, in
/// metres, negative inside. laid.wesl's `mark_edge` is the same.
pub fn mark_edge(m: &Mark, p: Vec2) -> f32 {
    let (s, c) = m.turn.to_radians().sin_cos();
    let d = p - Vec2::new(m.at.0, m.at.1);
    let mut q = Vec2::new(d.x * c + d.y * s, -d.x * s + d.y * c);
    let every = Vec2::new(m.every.0, m.every.1).max(Vec2::splat(1e-3));
    let reach = (Vec2::new(m.count.0 as f32, m.count.1 as f32) - 1.0) / 2.0;
    // An even count has no repeat in the middle: its repeats sit half a spacing off it.
    let half = reach.fract();
    q -= ((q / every - half).round() + half).clamp(-reach, reach) * every;
    let e = q.abs() - Vec2::new(m.size.0, m.size.1) / 2.0;
    e.x.max(e.y)
}

/// A mark's three rows of the uniform.
fn mark_rows(m: &Mark) -> [Vec4; 3] {
    let (s, c) = m.turn.to_radians().sin_cos();
    let kind = match m.kind {
        MarkKind::White => 0.0,
        MarkKind::Yellow => 1.0,
        MarkKind::Stain => 2.0,
        MarkKind::Patch => 3.0,
    };
    [
        Vec4::new(m.at.0, m.at.1, m.size.0 / 2.0, m.size.1 / 2.0),
        Vec4::new(c, s, m.every.0.max(1e-3), m.every.1.max(1e-3)),
        Vec4::new(
            (m.count.0 as f32 - 1.0) / 2.0,
            (m.count.1 as f32 - 1.0) / 2.0,
            kind,
            m.amount,
        ),
    ]
}

/// Which unit of a laid scan a point of its texture lies in: the tile (the scan's repeat),
/// the course and the unit along it, a unit cut by the tile's edge counted in the tile it
/// starts in. laid.wesl's `unit` is the same.
pub fn unit(uv: Vec2, courses: &[(f32, Vec<f32>)]) -> (i32, i32, usize, usize) {
    let mut tile = uv.floor();
    let f = uv - tile;
    let n = courses.len();
    let mut r = None;
    for (k, c) in courses.iter().enumerate() {
        if f.y >= c.0 {
            r = Some(k);
        }
    }
    let r = r.unwrap_or_else(|| {
        tile.y -= 1.0;
        n - 1
    });
    let joints = &courses[r].1;
    let mut c = joints.iter().filter(|j| f.x >= **j).count();
    if c == 0 {
        tile.x -= 1.0;
        c = joints.len();
    }
    (tile.x as i32, tile.y as i32, r, c)
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct LaidExt {
    #[uniform(106)]
    pub params: Params,
    /// The game's ground stack (materials/ground), for its moss.
    #[texture(100, dimension = "2d_array")]
    #[sampler(101)]
    pub stack: Handle<Image>,
    /// The scan's own occlusion, which marks its joints.
    #[texture(102)]
    #[sampler(103)]
    pub arm: Handle<Image>,
    /// The stone detail texture (D362), grey about the middle.
    #[texture(104)]
    #[sampler(105)]
    pub detail: Handle<Image>,
}

impl MaterialExtension for LaidExt {
    fn fragment_shader() -> ShaderRef {
        "embedded://iso/kit/laid.wesl".into()
    }
}

/// The layout's paving: the names of the maps its mesh carries, in the second UV's x and y
/// and the colour's alpha (the colour's rgb is the layout's own grime on the scan).
pub struct Place<'a> {
    pub sites: [&'a str; 3],
}

/// The shader's source for a laid floor's mask, or what is wrong and the fix (D556).
fn source(mask: Option<&Mask>, place: &Place) -> Result<f32, String> {
    match mask {
        None => Ok(NONE),
        Some(Mask::Base) => Ok(ONE),
        Some(Mask::Site(name)) => place
            .sites
            .iter()
            .position(|s| s == name)
            .map(|k| k as f32)
            .ok_or_else(|| {
                format!(
                    "the layout's paving has no map {name:?}; its maps are {}. Fix: name one \
                     of those in Site(...), or Base (kit/laid.rs, D634)",
                    place.sites.join(", ")
                )
            }),
        Some(other) => Err(format!(
            "a laid floor's mask is the layout's map by name or Base, not {other:?}: the \
             kit's rule masks are baked from a land the paving does not have. Fix: give the \
             layout's paving that map (kit/laid.rs `Place`) and name it in Site(...) (D634)"
        )),
    }
}

/// A laid floor as the shader's uniform, or what is wrong with it and the fix.
pub fn params(laid: &Laid, place: &Place) -> Result<Params, String> {
    let moss = source(laid.moss.as_ref(), place).map_err(|e| format!("laid moss: {e}"))?;
    let wet = source(laid.wet.as_ref(), place).map_err(|e| format!("laid wet: {e}"))?;
    let feet = source(laid.feet.as_ref(), place).map_err(|e| format!("laid feet: {e}"))?;
    let moss_m = STACK
        .iter()
        .find(|(n, _)| *n == "moss_004")
        .map_or(1.1, |s| s.1);
    let mut p = Params {
        look: Vec4::new(
            laid.joints,
            laid.cracks,
            laid.grain.0.max(1e-3),
            laid.grain.1,
        ),
        masks: Vec4::new(moss, wet, moss_m, feet),
        vary: Vec4::new(laid.spread.0, laid.spread.1, laid.dirt, laid.damp),
        shape: Vec4::new(
            laid.courses.len() as f32,
            laid.marks.len() as f32,
            laid.grit,
            laid.sand,
        ),
        wear: Vec4::new(
            laid.wear.split,
            laid.wear.tilt,
            laid.wear.broken,
            laid.wear.missing,
        ),
        seams: Vec4::new(laid.wear.gap, laid.wear.uneven, laid.wear.round, 0.0),
        ..default()
    };
    let bad_mark = |m: &Mark| {
        m.size.0 <= 0.0
            || m.size.1 <= 0.0
            || m.count.0 == 0
            || m.count.1 == 0
            || (m.count.0 > 1 && m.every.0 < m.size.0)
            || (m.count.1 > 1 && m.every.1 < m.size.1)
            || !(0.0..=1.0).contains(&m.amount)
    };
    if laid.marks.len() > MARKS || laid.marks.iter().any(bad_mark) {
        return Err(format!(
            "laid marks: the kit draws at most {MARKS} marks a floor (this has {}), each a \
             size over 0, a count of at least 1 along each axis, repeats spaced at least \
             its size apart and an amount 0 to 1. Fix: merge marks into repeats (`every`, \
             `count`) or mend the one named: {:?} (kit/laid.rs, D634)",
            laid.marks.len(),
            laid.marks.iter().find(|m| bad_mark(m))
        ));
    }
    for (k, m) in laid.marks.iter().enumerate() {
        [p.mark_at[k], p.mark_turn[k], p.mark_rep[k]] = mark_rows(m);
    }
    let rising = |v: &[f32]| v.windows(2).all(|w| w[0] < w[1]);
    let starts: Vec<f32> = laid.courses.iter().map(|c| c.0).collect();
    if laid.courses.len() > COURSES
        || laid
            .courses
            .iter()
            .any(|c| c.1.len() > JOINTS || !rising(&c.1))
        || !rising(&starts)
        || laid
            .courses
            .iter()
            .flat_map(|c| c.1.iter().chain([&c.0]))
            .any(|x| !(0.0..1.0).contains(x))
    {
        return Err(format!(
            "laid courses {:?}: the kit tells apart at most {COURSES} courses of up to \
             {JOINTS} joints, each start and joint in 0 to 1, rising. Fix: measure them off \
             the scan's colour map, top to bottom and left to right (kit/laid.rs, D634)",
            laid.courses
        ));
    }
    for (k, (v, joints)) in laid.courses.iter().enumerate() {
        let j = |i: usize| joints.get(i).copied().unwrap_or(2.0);
        p.courses[k] = Vec4::new(*v, j(0), j(1), j(2));
        p.joints[k] = Vec4::new(j(3), j(4), j(5), j(6));
    }
    Ok(p)
}

/// The floor's material: the scan through StandardMaterial, the weather over it. A bad
/// floor panics with what to fix.
pub fn material(asset: &AssetServer, laid: &Laid, place: &Place) -> LaidMaterial {
    let params = params(laid, place).unwrap_or_else(|e| panic!("{e}"));
    let linear = |file: String| -> Handle<Image> {
        asset
            .load_builder()
            .with_settings(|s: &mut ImageLoaderSettings| s.is_srgb = false)
            .load(file)
    };
    ExtendedMaterial {
        base: scene::material(asset, &laid.material, Color::WHITE),
        extension: LaidExt {
            params,
            stack: asset
                .load_builder()
                .with_settings(|s: &mut ImageLoaderSettings| {
                    s.array_layout = Some(ImageArrayLayout::RowCount {
                        rows: STACK.len() as u32,
                    });
                })
                .load("materials/ground/color.jpg"),
            arm: linear(format!("materials/{}/arm.jpg", laid.material)),
            detail: linear("materials/detail_stone/color.jpg".into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLACE: Place = Place {
        sites: ["mossy", "puddled", "walls"],
    };

    fn laid(moss: Option<Mask>, wet: Option<Mask>) -> Laid {
        Laid {
            material: "large_grey_tiles".into(),
            metres: 3.0,
            joints: 0.3,
            sand: 0.0,
            cracks: 0.25,
            grain: (0.5, 0.6),
            moss,
            wet,
            courses: vec![(0.0, vec![0.5]), (0.5, vec![0.25, 0.75])],
            spread: (0.2, 0.3),
            dirt: 0.5,
            grit: 1.0,
            damp: 0.5,
            feet: Some(Mask::Site("walls".into())),
            marks: vec![],
            wear: Default::default(),
        }
    }

    fn mark(size: (f32, f32), every: (f32, f32), count: (u32, u32), turn: f32) -> Mark {
        Mark {
            kind: MarkKind::White,
            at: (10.0, -2.0),
            size,
            turn,
            every,
            count,
            amount: 1.0,
        }
    }

    /// A dashed line's dashes lie where it says and its gaps are bare; an even count
    /// leaves the middle bare; a turn turns it; past the last repeat is bare.
    #[test]
    fn marks_lie_where_they_say() {
        let at = |m: &Mark, x: f32, z: f32| mark_edge(m, Vec2::new(10.0 + x, -2.0 + z)) < 0.0;
        // Three 3 m dashes every 9 m along x, 0.1 m wide.
        let dash = mark((3.0, 0.1), (9.0, 0.0), (3, 1), 0.0);
        for x in [-9.0, -10.4, 0.0, 1.4, 9.0, 10.4] {
            assert!(at(&dash, x, 0.0), "a dash covers x {x}");
        }
        for x in [-4.5, 2.0, 4.5, 7.0, 12.0, 18.0, -18.0] {
            assert!(!at(&dash, x, 0.0), "x {x} is a gap or past the line");
        }
        assert!(!at(&dash, 0.0, 0.08), "the line is 0.1 m wide");
        // A zebra of four 0.5 m stripes every metre along z: none at the middle.
        let zebra = mark((3.0, 0.5), (0.0, 1.0), (1, 4), 0.0);
        for z in [-1.5, -0.5, 0.5, 1.5] {
            assert!(at(&zebra, 0.0, z), "a stripe at z {z}");
        }
        for z in [0.0, -1.0, 1.0, 2.5] {
            assert!(
                !at(&zebra, 0.0, z),
                "z {z} lies between the stripes or past them"
            );
        }
        // Turned a right angle, a long thin mark runs along z.
        let turned = mark((4.0, 0.1), (0.0, 0.0), (1, 1), 90.0);
        assert!(at(&turned, 0.0, 1.8) && !at(&turned, 1.8, 0.0));
        // The distance outside grows with the distance.
        let one = mark((2.0, 2.0), (0.0, 0.0), (1, 1), 0.0);
        assert!((mark_edge(&one, Vec2::new(13.0, -2.0)) - 2.0).abs() < 1e-4);
    }

    #[test]
    fn bad_marks_say_the_fix() {
        let mut l = laid(None, None);
        l.marks = vec![mark((3.0, 0.1), (2.0, 0.0), (3, 1), 0.0)];
        let e = params(&l, &PLACE).unwrap_err();
        assert!(e.contains("Fix:") && e.contains("marks"), "{e}");
        l.marks = vec![mark((3.0, 0.1), (9.0, 0.0), (3, 1), 0.0); MARKS + 1];
        assert!(params(&l, &PLACE).is_err());
        l.marks.truncate(MARKS);
        let p = params(&l, &PLACE).unwrap();
        assert_eq!(p.shape.y, MARKS as f32);
        assert_eq!(p.mark_rep[MARKS - 1].z, 0.0, "the last mark is white");
        assert_eq!(p.mark_at[MARKS - 1].z, 1.5, "its half length");
    }

    /// A running bond of two courses: points on one unit share it, across a joint or a
    /// course they do not, and a unit cut by the scan's edge is one unit on both sides.
    #[test]
    fn units_are_told_apart_by_their_joints() {
        let c = laid(None, None).courses;
        let u = |x: f32, y: f32| unit(Vec2::new(x, y), &c);
        assert_eq!(u(0.1, 0.1), u(0.4, 0.4), "one unit");
        assert_ne!(u(0.4, 0.1), u(0.6, 0.1), "across a joint");
        assert_ne!(u(0.4, 0.4), u(0.4, 0.6), "across a course");
        // The first course's second unit runs from u 0.5 over the edge to the next tile's
        // 0.5; the second course's last from 0.75 to the next tile's 0.25.
        assert_eq!(u(0.9, 0.2), u(1.2, 0.2), "cut by the scan's edge");
        assert_eq!(u(0.9, 0.7), u(1.1, 0.7));
        assert_ne!(u(0.9, 0.7), u(1.3, 0.7));
        // A scan whose first course starts below its top edge: above it is the last
        // course of the tile above.
        let low = vec![(0.1, vec![0.5]), (0.6, vec![0.3])];
        assert_eq!(
            unit(Vec2::new(0.4, 0.05), &low),
            unit(Vec2::new(0.4, -0.2), &low)
        );
    }

    /// Courses out of order or too many fail naming the fix.
    #[test]
    fn bad_courses_say_the_fix() {
        let mut l = laid(None, None);
        l.courses = vec![(0.5, vec![0.2]), (0.1, vec![0.3])];
        let e = params(&l, &PLACE).unwrap_err();
        assert!(e.contains("rising") && e.contains("Fix:"), "{e}");
        l.courses = vec![(0.0, vec![0.7, 0.2])];
        assert!(params(&l, &PLACE).is_err());
        l.courses = vec![(0.0, vec![]); COURSES + 1];
        assert!(params(&l, &PLACE).is_err());
    }

    /// A mask names the paving's map it reads, Base reads everywhere, none nowhere.
    #[test]
    fn masks_read_the_pavings_maps_by_name() {
        let p = params(
            &laid(Some(Mask::Site("walls".into())), Some(Mask::Base)),
            &PLACE,
        )
        .unwrap();
        assert_eq!(p.masks.x, 2.0);
        assert_eq!(p.masks.y, ONE);
        let p = params(&laid(None, Some(Mask::Site("puddled".into()))), &PLACE).unwrap();
        assert_eq!((p.masks.x, p.masks.y), (NONE, 1.0));
    }

    /// A map the paving lacks, or a rule mask it has no land for, fails with the fix.
    #[test]
    fn a_bad_mask_says_the_fix() {
        let e = params(&laid(Some(Mask::Site("lichen".into())), None), &PLACE).unwrap_err();
        assert!(
            e.contains("lichen") && e.contains("mossy") && e.contains("Fix:"),
            "{e}"
        );
        let e = params(&laid(None, Some(Mask::Wet)), &PLACE).unwrap_err();
        assert!(e.contains("Wet") && e.contains("Fix:"), "{e}");
    }

    /// The ruin's floor resolves on the court's paving.
    #[test]
    fn the_ruins_floor_resolves() {
        let b = super::super::biome::Biome::load("ruin").unwrap();
        let l = b.ground.laid.expect("the ruin lays its court");
        let p = params(&l, &PLACE).unwrap();
        // Its flags worn (split, tilted, broken, gone), each a share or an angle the shader
        // reads in that order; a floor that names no wear lays the scan's units as they are.
        assert!(p.wear.x > 0.0 && p.wear.y > 0.0 && p.wear.z > 0.0 && p.wear.w > 0.0);
        assert!(p.wear.x <= 1.0 && p.wear.z <= 1.0 && p.wear.w <= 1.0 && p.wear.y < 0.2);
        assert_eq!(params(&laid(None, None), &PLACE).unwrap().wear, Vec4::ZERO);
        // Its joints opened unevenly and its arrises worn round, in the order the shader
        // reads them (gap, uneven, round), a share and centimetres.
        assert!(p.seams.x > 0.005 && p.seams.x < 0.05, "{}", p.seams.x);
        assert!(p.seams.y > 0.0 && p.seams.y < 1.0, "{}", p.seams.y);
        assert!(p.seams.z > 0.005 && p.seams.z < 0.1, "{}", p.seams.z);
    }

    /// The street's flags (six courses of a running bond, iso-kit j49) are each told apart:
    /// two points a flag's width apart along a course lie on different flags, and the
    /// joints open to a width the shader reads; a floor naming no gap keeps the scan's.
    #[test]
    fn the_streets_flags_are_told_apart() {
        let b = super::super::biome::Biome::load("neon-street").unwrap();
        let l = b.ground.laid.expect("the street lays its pavements");
        let p = params(&l, &PLACE).unwrap();
        assert_eq!(p.shape.x as usize, l.courses.len());
        assert!(l.courses.len() > 4, "the scan's six courses");
        let a = unit(Vec2::new(0.25, 0.08), &l.courses);
        let b2 = unit(Vec2::new(0.6, 0.08), &l.courses);
        let c = unit(Vec2::new(0.25, 0.25), &l.courses);
        assert_ne!(a, b2, "along a course");
        assert_ne!(a, c, "across courses");
        assert!(p.seams.x > 0.005 && p.seams.x < 0.05, "{}", p.seams.x);
        assert_eq!(params(&laid(None, None), &PLACE).unwrap().seams, Vec4::ZERO);
    }
}
