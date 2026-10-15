//! A material's pack checked as it is added (D349, docs/proposals/construction-002.md): the
//! mistakes a material made from a scan brings with it, which the drawing would otherwise
//! take without a word and show only later in the game as a material that "looks wrong".
//! Each is a problem in plain words saying what is wrong and how to put it right, so
//! `cargo xtask pack` refuses the material when it is made and `cargo xtask kits` (run by
//! `cargo xtask check`) holds every material to the same.
//!
//! - **Its height map**: the drawing displaces real geometry by it and breaks a material
//!   along its low places (`cells`, D305), so it must have range enough to read, not be
//!   upside down (its joints bright), not be noise, and agree with its colour map: a
//!   scan's height is high where its occlusion and colour are light, as stones stand out
//!   of dark joints.
//! - **That it tiles**: every face, member and covering repeats its texture, so a set that
//!   does not wrap draws a seam a copy apart.
//! - **Its real size**: a stone, a log, a course of slates each the size a pattern lays
//!   them, measured from the maps and the pack's numbers against the pattern's range.
//! - **Its relief and parallax depth** within what its pattern draws well.
//! - **Every role its pattern asks a pack for** given by the pack or by the pack it names
//!   under it (`pack:`), never left to a pattern's default or to nothing.
//! - **Roughness, metal and reflectance** a rough, non-metal building material has.
//!
//! It reads no files: it is given each texture set's maps, decoded, and what the material's
//! own pack chain gives, so it runs as a test and as a tool alike.

use crate::Material;
use crate::kit::{Kit, Pattern, Profile};
use std::collections::BTreeSet;

/// A map of a texture set, decoded: `width` by `height` texels, three bytes each (red,
/// green, blue), row by row from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

/// The side a map is brought down to before it is measured: its broad structure, which is
/// what each check reads, without the cost of every texel.
const SIDE: usize = 256;

/// One channel of a map, brought down to at most [`SIDE`] a side by averaging, 0 to 1.
#[derive(Clone, Debug)]
struct Grey {
    w: usize,
    h: usize,
    v: Vec<f32>,
}

/// Which of a picture's channels to read: its brightness, or one of its three.
#[derive(Clone, Copy)]
enum Channel {
    Luma,
    Red,
    Green,
    Blue,
}

impl Grey {
    fn of(picture: &Picture, channel: Channel) -> Option<Grey> {
        let (pw, ph) = (picture.width, picture.height);
        if pw == 0 || ph == 0 || picture.rgb.len() < pw * ph * 3 {
            return None;
        }
        let step = pw.max(ph).div_ceil(SIDE).max(1);
        let (w, h) = (pw.div_ceil(step), ph.div_ceil(step));
        let mut v = vec![0.0f32; w * h];
        let mut n = vec![0u32; w * h];
        for y in 0..ph {
            for x in 0..pw {
                let at = (y * pw + x) * 3;
                let [r, g, b] = [0, 1, 2].map(|c| f32::from(picture.rgb[at + c]) / 255.0);
                let value = match channel {
                    Channel::Luma => 0.2126 * r + 0.7152 * g + 0.0722 * b,
                    Channel::Red => r,
                    Channel::Green => g,
                    Channel::Blue => b,
                };
                let to = (y / step) * w + x / step;
                v[to] += value;
                n[to] += 1;
            }
        }
        for (value, count) in v.iter_mut().zip(n) {
            *value /= count.max(1) as f32;
        }
        Some(Grey { w, h, v })
    }

    fn at(&self, x: isize, y: isize) -> f32 {
        let x = x.rem_euclid(self.w as isize) as usize;
        let y = y.rem_euclid(self.h as isize) as usize;
        self.v[y * self.w + x]
    }

    fn mean(&self) -> f32 {
        self.v.iter().sum::<f32>() / self.v.len().max(1) as f32
    }

    fn deviation(&self) -> f32 {
        let mean = self.mean();
        libm::sqrtf(
            self.v.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>()
                / self.v.len().max(1) as f32,
        )
    }

    /// The value under which a share of its texels lie.
    fn quantile(&self, share: f32) -> f32 {
        let mut sorted = self.v.clone();
        sorted.sort_by(f32::total_cmp);
        sorted[((sorted.len() - 1) as f32 * share.clamp(0.0, 1.0)) as usize]
    }

    /// Averaged over a square `2r + 1` across, wrapping, as it tiles.
    fn blurred(&self, r: isize) -> Grey {
        let mut v = vec![0.0; self.v.len()];
        let n = ((2 * r + 1) * (2 * r + 1)) as f32;
        for y in 0..self.h as isize {
            for x in 0..self.w as isize {
                let mut sum = 0.0;
                for dy in -r..=r {
                    for dx in -r..=r {
                        sum += self.at(x + dx, y + dy);
                    }
                }
                v[y as usize * self.w + x as usize] = sum / n;
            }
        }
        Grey {
            w: self.w,
            h: self.h,
            v,
        }
    }

    /// How alike two maps of one size are: their correlation, -1 to 1 (0 if either is flat).
    fn correlation(&self, other: &Grey) -> f32 {
        if self.v.len() != other.v.len() {
            return 0.0;
        }
        let (ma, mb) = (self.mean(), other.mean());
        let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
        for (a, b) in self.v.iter().zip(&other.v) {
            ab += (a - ma) * (b - mb);
            aa += (a - ma) * (a - ma);
            bb += (b - mb) * (b - mb);
        }
        if aa < 1e-9 || bb < 1e-9 {
            return 0.0;
        }
        ab / libm::sqrtf(aa * bb)
    }

    /// How much a step across each edge, where a copy meets the next, stands out from the
    /// steps between texels inside it: across its width and down its height. About 1 or
    /// under where it tiles; a seam stands well over the largest step inside.
    fn seams(&self) -> [f32; 2] {
        let step =
            |a: (isize, isize), b: (isize, isize)| (self.at(a.0, a.1) - self.at(b.0, b.1)).abs();
        let (w, h) = (self.w as isize, self.h as isize);
        // Each column's step to the next, and each row's, summed along it.
        let columns: Vec<f32> = (0..w)
            .map(|x| (0..h).map(|y| step((x, y), (x + 1, y))).sum::<f32>())
            .collect();
        let rows: Vec<f32> = (0..h)
            .map(|y| (0..w).map(|x| step((x, y), (x, y + 1))).sum::<f32>())
            .collect();
        let ratio = |steps: &[f32]| {
            let edge = steps[steps.len() - 1];
            let mut inside = steps[..steps.len() - 1].to_vec();
            inside.sort_by(f32::total_cmp);
            // Against nearly the largest step inside, so a texture whose joints, courses or
            // boards fall along its rows or columns, a large step wherever one falls, is
            // not taken for a seam where one falls on its edge.
            let large = inside[(inside.len() - 1) * 99 / 100].max(1e-4);
            edge / large
        };
        [ratio(&columns), ratio(&rows)]
    }

    /// How many islands its highest texels make, its lowest `share` being joints and a
    /// texel beside a joint joint too, as `cells::label` parts a material's stones; specks
    /// under a fiftieth of a percent of it left out.
    fn islands(&self, share: f32) -> usize {
        let cut = self.quantile(share);
        let (w, h) = (self.w as isize, self.h as isize);
        let solid: Vec<bool> = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .all(|&(dx, dy)| self.at(x + dx, y + dy) > cut)
            })
            .collect();
        let least = (self.v.len() / 5000).max(4);
        let mut seen = vec![false; solid.len()];
        let mut count = 0;
        let mut stack = Vec::new();
        for start in 0..solid.len() {
            if seen[start] || !solid[start] {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            let mut size = 0;
            while let Some(at) = stack.pop() {
                size += 1;
                let (x, y) = ((at % self.w) as isize, (at / self.w) as isize);
                for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    let next =
                        (y + dy).rem_euclid(h) as usize * self.w + (x + dx).rem_euclid(w) as usize;
                    if solid[next] && !seen[next] {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
            if size >= least {
                count += 1;
            }
        }
        count
    }
}

/// One channel of a map at its own texels: its top left corner, at most [`SIDE`] a side,
/// for what only its texels show (noise).
fn texels(picture: &Picture, channel: usize) -> Option<Grey> {
    let (w, h) = (picture.width.min(SIDE), picture.height.min(SIDE));
    if w < 8 || h < 8 || picture.rgb.len() < picture.width * picture.height * 3 {
        return None;
    }
    let v = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| f32::from(picture.rgb[(y * picture.width + x) * 3 + channel]) / 255.0)
        .collect();
    Some(Grey { w, h, v })
}

/// A height map brought down to at most `side` a side by the least of the texels under
/// each, as `cells::label` brings it down, so a joint a texel wide is kept.
fn least(picture: &Picture, side: usize) -> Option<Grey> {
    let (pw, ph) = (picture.width, picture.height);
    if pw == 0 || ph == 0 || picture.rgb.len() < pw * ph * 3 {
        return None;
    }
    let (w, h) = (side.min(pw), side.min(ph));
    let mut v = vec![1.0f32; w * h];
    for y in 0..ph {
        for x in 0..pw {
            let to = (y * h / ph) * w + x * w / pw;
            v[to] = v[to].min(f32::from(picture.rgb[(y * pw + x) * 3]) / 255.0);
        }
    }
    Some(Grey { w, h, v })
}

/// What the maps of a texture set measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Measured {
    /// Its maps' sizes in texels, colour then height (if it has one).
    pub size: [usize; 2],
    pub height_size: Option<[usize; 2]>,
    /// Its height's range, from the 2nd to the 98th percentile, 0 to 1.
    pub range: f32,
    /// How its height agrees with its colour: -1 to 1, high where its light parts stand
    /// out, as a scan's stones stand out of their dark joints. Its occlusion's correlation
    /// with its height or its colour's brightness's, whichever is the stronger.
    pub agrees: f32,
    /// How much of its height's variation is from one texel to the next, 0 to 1: about 0.9
    /// where it is all noise.
    pub noise: f32,
    /// The step where a copy meets the next, across and down, in its colour or its height,
    /// against nearly the largest inside it: about 1 or under where it tiles.
    pub seams: [f32; 2],
    /// Its mean roughness and metal, 0 to 1.
    pub roughness: f32,
    pub metal: f32,
    /// How many cells (stones, slates) the low places of its height part one copy of it
    /// into.
    pub cells: Option<usize>,
    /// Its colour's mean brightness, 0 to 1 as stored (sRGB).
    pub brightness: f32,
}

/// The share of a height map that is its joints, as damage parts it (`cells`, D305).
const JOINTS: f32 = 0.22;

/// Measures a texture set from its colour, occlusion-roughness-metal and height maps.
pub fn measure(colour: &Picture, arm: Option<&Picture>, height: Option<&Picture>) -> Measured {
    let mut out = Measured {
        size: [colour.width, colour.height],
        height_size: height.map(|h| [h.width, h.height]),
        ..Measured::default()
    };
    let Some(luma) = Grey::of(colour, Channel::Luma) else {
        return out;
    };
    out.brightness = luma.mean();
    let mut seams = luma.seams();
    let occlusion = arm.and_then(|arm| Grey::of(arm, Channel::Red));
    if let Some(arm) = arm {
        out.roughness = Grey::of(arm, Channel::Green).map_or(0.0, |g| g.mean());
        out.metal = Grey::of(arm, Channel::Blue).map_or(0.0, |g| g.mean());
    }
    if let Some(height) = height
        && let Some(h) = Grey::of(height, Channel::Red)
    {
        out.range = h.quantile(0.98) - h.quantile(0.02);
        if let Some(fine) = texels(height, 0) {
            let all = fine.deviation();
            let smooth = fine.blurred(1);
            let rest = libm::sqrtf(
                fine.v
                    .iter()
                    .zip(&smooth.v)
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum::<f32>()
                    / fine.v.len() as f32,
            );
            out.noise = if all > 1e-3 { rest / all } else { 0.0 };
        }
        // Each over a few texels, so a colour's own speckle does not swamp it; occlusion
        // only where it has something in it, as a made material's is often flat.
        let near = h.blurred(2);
        let with = |other: &Grey| {
            if other.w == h.w && other.h == h.h {
                near.correlation(&other.blurred(2))
            } else {
                0.0
            }
        };
        let by_colour = with(&luma);
        let by_occlusion = occlusion
            .as_ref()
            .filter(|ao| ao.deviation() > 0.02)
            .map_or(0.0, with);
        out.agrees = if by_occlusion.abs() > by_colour.abs() {
            by_occlusion
        } else {
            by_colour
        };
        let height_seams = h.seams();
        seams = [seams[0].max(height_seams[0]), seams[1].max(height_seams[1])];
        out.cells = least(height, crate::cells::CELLS as usize).map(|l| l.islands(JOINTS));
    }
    out.seams = seams;
    out
}

/// The limits a material is held to, from the shipped scans' measures and the planted bad
/// ones' (`tests`): each a margin past the worst good one and short of the bad.
pub mod limits {
    /// A seam: the step where copies meet against nearly the largest inside (the shipped
    /// sets 0.3 to 2.4, a scan cropped so it does not tile 3.2 to 8.6).
    pub const SEAM: f32 = 2.8;
    /// Inverted: its height's agreement with its colour (the shipped -0.16 to 1, one
    /// flipped -0.86).
    pub const INVERTED: f32 = -0.25;
    /// Not its colour's: an agreement this near nothing either way (the shipped at least
    /// 0.12 either way, another scan's height 0.05).
    pub const UNRELATED: f32 = 0.08;
    /// Noise: the share of its height's variation from texel to texel (the shipped at most
    /// 0.19, random noise 0.94).
    pub const NOISE: f32 = 0.6;
    /// Weak: its height's range, of 0 to 1, where it shapes the surface (the shipped at
    /// least 0.26).
    pub const RANGE: f32 = 0.15;
    /// Stones and flags across, in metres, where its height parts it into at least
    /// [`STONY`] cells a copy; a plain face's copy (earth, plaster) otherwise.
    pub const STONES: [f32; 2] = [0.10, 0.60];
    pub const FLAGS: [f32; 2] = [0.15, 0.90];
    pub const PLAIN: [f32; 2] = [0.5, 5.0];
    pub const STONY: usize = 6;
    /// Logs across, in metres; and a bark laid once round a log, its texture's width
    /// against the log's girth.
    pub const LOGS: [f32; 2] = [0.20, 0.40];
    pub const GIRTH: [f32; 2] = [0.6, 1.6];
    /// A covering's course up the roof, in metres: a slate's or shingle's 15 to 40 cm,
    /// thatch's longer.
    pub const COURSES: [f32; 2] = [0.12, 0.70];
    /// Relief, in metres: a block face's, a log's bark, a covering's courses (and no more
    /// than half a course), a framed kind's core; parallax depth on a wall and on a floor.
    pub const BLOCK_RELIEF: [f32; 2] = [0.01, 0.10];
    pub const BARK_RELIEF: f32 = 0.04;
    pub const ROOF_RELIEF: [f32; 2] = [0.01, 0.08];
    pub const ROOF_THICK: [f32; 2] = [0.01, 0.35];
    pub const CORE_RELIEF: f32 = 0.06;
    pub const WALL_DEPTH: f32 = 0.06;
    pub const FLOOR_DEPTH: f32 = 0.03;
    /// A rough building material's: its mean roughness at least, its mean metal at most,
    /// and its reflectance (0.35 by default, 2%).
    pub const ROUGHNESS: f32 = 0.25;
    pub const METAL: f32 = 0.1;
    pub const REFLECTANCE: [f32; 2] = [0.05, 0.6];
    /// A detail texture's mean brightness as stored, grey about the middle so it brightens
    /// and darkens without tinting (made 0.5 by `tools/textures detail`); its size, how many
    /// metres a copy covers, a few times a metre; and its strength (D362).
    pub const DETAIL_GREY: [f32; 2] = [0.42, 0.58];
    pub const DETAIL_METRES: [f32; 2] = [0.1, 1.0];
    pub const DETAIL_STRENGTH: [f32; 2] = [0.0, 1.5];
}

use limits::*;

/// A pattern by the name its file has.
fn pattern_name(kit: &Kit) -> &'static str {
    match (kit.pattern, kit.framing.is_some()) {
        (Pattern::Masonry, true) => "frame",
        (Pattern::Masonry, false) => "block",
        (Pattern::Logs, _) => "coursed",
        (Pattern::Roof, _) => "roof",
    }
}

/// The roles a pattern asks every material's pack for, the pattern's own file giving none
/// of them a texture that suits every material: each with what it draws.
pub fn pack_roles(kit: &Kit) -> Vec<(Material, &'static str)> {
    match kit.pattern {
        Pattern::Masonry => vec![
            (Material::Masonry, "its walls' faces"),
            (Material::Floor, "its floors and its walls' tops"),
        ],
        Pattern::Logs => {
            let mut roles = vec![(Material::Log, "its members' faces")];
            if kit.chinking {
                roles.push((Material::Daub, "the daub between its courses"));
            }
            roles
        }
        Pattern::Roof => {
            let mut roles = vec![
                (Material::Covering, "its covering"),
                (Material::Log, "its rafters"),
            ];
            if kit
                .roof
                .parts
                .values()
                .any(|part| part.material == Material::Piece)
            {
                roles.push((Material::Piece, "its own ridge and hip pieces"));
            }
            roles
        }
    }
}

/// Checks a material, `kit` its whole merged file, `given` the roles its own lines and its
/// packs give (`layers::own_textures`), and `measured` a texture set's maps measured by its
/// folder under `assets/`, none if they cannot be read (which the kit checker says).
pub fn check(
    kit: &Kit,
    given: &BTreeSet<Material>,
    measured: &dyn Fn(&str) -> Option<Measured>,
) -> Vec<String> {
    let mut out = Vec::new();
    roles(kit, given, &mut out);
    numbers(kit, &mut out);
    for (&material, texture) in &kit.textures {
        if let Some(m) = measured(&texture.path) {
            maps(kit, material, texture, &m, &mut out);
        }
    }
    details(kit, measured, &mut out);
    out
}

/// A material's detail textures (D362): each over a texture it has, grey about the middle,
/// a few times a metre and not overdone.
fn details(kit: &Kit, measured: &dyn Fn(&str) -> Option<Measured>, out: &mut Vec<String>) {
    let within =
        |value: f32, [low, high]: [f32; 2]| value.is_finite() && value >= low && value <= high;
    for (material, detail) in &kit.details {
        let mut fail =
            |what: String| out.push(format!("{material:?}'s detail ({}): {what}", detail.path));
        if !kit.textures.contains_key(material) {
            fail(
                "it has no texture to lay it over: take the detail out, or give the texture".into(),
            );
        }
        if !within(detail.metres, DETAIL_METRES) {
            fail(format!(
                "a copy {} m across, where detail is laid {} to {} m: larger and it is no finer than the texture under it, smaller and it is noise. Set its metres",
                detail.metres, DETAIL_METRES[0], DETAIL_METRES[1]
            ));
        }
        if !within(detail.strength, DETAIL_STRENGTH) {
            fail(format!(
                "strength {}, past the {} to {} it reads well at: it would swamp the texture under it. Set its strength",
                detail.strength, DETAIL_STRENGTH[0], DETAIL_STRENGTH[1]
            ));
        }
        if let Some(m) = measured(&detail.path)
            && !within(m.brightness, DETAIL_GREY)
        {
            fail(format!(
                "its colour is {:.2} on average, not grey about the middle ({} to {}): it would tint every material it is laid on. Make it with `node tools/textures/textures.ts detail`",
                m.brightness, DETAIL_GREY[0], DETAIL_GREY[1]
            ));
        }
    }
}

/// The fields a pattern's materials never read, and why: baggage that would make a material's
/// file say something it does not draw (research/building-code-health-2026-10-15.md, R8).
pub fn unread(kit: &Kit) -> &'static [(&'static str, &'static str)] {
    const COURSES: &str = "only a coursed kind lays daub between courses";
    match kit.pattern {
        Pattern::Roof => &[
            (
                "timber",
                "a roof lays no timber: its rafters are modules, tinted by its parts",
            ),
            ("chinking", COURSES),
            ("colours.chinking", COURSES),
            ("packed", "only a block kind is packed by hand"),
            ("framing", "only a frame kind has a frame"),
        ],
        Pattern::Masonry => &[("chinking", COURSES), ("colours.chinking", COURSES)],
        Pattern::Logs => &[
            ("packed", "only a block kind is packed by hand"),
            ("framing", "only a frame kind has a frame"),
        ],
    }
}

/// A material's fields its pattern never reads, `set` being every field its files set and
/// the file that sets it (`layers::fields_set`): each a problem naming the file.
pub fn fields(kit: &Kit, set: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (file, field) in set {
        if let Some((_, why)) = unread(kit).iter().find(|(name, _)| name == field) {
            out.push(format!(
                "{file} sets {field}, which the {} pattern never reads ({why}): take it out, so its file says only what it draws",
                pattern_name(kit)
            ));
        }
    }
    out
}

fn cm(metres: f32) -> String {
    format!("{:.0} cm", metres * 100.0)
}

fn roles(kit: &Kit, given: &BTreeSet<Material>, out: &mut Vec<String>) {
    let pattern = pattern_name(kit);
    for (role, what) in pack_roles(kit) {
        if given.contains(&role) {
            continue;
        }
        let fallback = match kit.textures.get(&role) {
            Some(texture) => format!(
                "the {pattern} pattern's default, {}, made for no material in particular",
                texture.path
            ),
            None => "nothing: left plain or not drawn".into(),
        };
        out.push(format!(
            "its pack gives no {role:?} texture, which the {pattern} pattern asks every pack for ({what}), so it would be drawn in {fallback}. Give one in its pack's textures, or lay the pack over one that has it with `pack: \"<pack>\"`"
        ));
    }
}

fn numbers(kit: &Kit, out: &mut Vec<String>) {
    let within =
        |value: f32, [low, high]: [f32; 2]| value.is_finite() && value >= low && value <= high;
    match kit.pattern {
        Pattern::Masonry => {
            if !within(kit.relief, BLOCK_RELIEF) {
                out.push(format!(
                    "relief {} stands its face out {}, where the {} pattern draws {} to {} well: under that its stones read flat, over it they float off the wall. Set its pack's relief",
                    kit.relief,
                    cm(kit.relief),
                    pattern_name(kit),
                    cm(BLOCK_RELIEF[0]),
                    cm(BLOCK_RELIEF[1])
                ));
            }
            if let Some(framing) = &kit.framing
                && framing.core_relief > CORE_RELIEF
            {
                out.push(format!(
                    "its core stands out {}, more than the {} under a frame's members: it would show through them. Set framing's core_relief",
                    cm(framing.core_relief),
                    cm(CORE_RELIEF)
                ));
            }
        }
        Pattern::Logs => {
            if !within(kit.diameter, LOGS) {
                out.push(format!(
                    "its logs are {} across, where the coursed pattern lays logs {} to {} (a cube being 50 cm): set its diameter",
                    cm(kit.diameter),
                    cm(LOGS[0]),
                    cm(LOGS[1])
                ));
            }
            if kit.timber.relief > BARK_RELIEF {
                out.push(format!(
                    "its bark stands out {}, more than the {} a log's face draws well: its outline would go lumpy. Set timber's relief",
                    cm(kit.timber.relief),
                    cm(BARK_RELIEF)
                ));
            }
            // A face laid once round each log (`timber.exposed`) is stretched to its girth.
            if kit.timber.profile == Profile::Round
                && kit.timber.exposed
                && let Some(texture) = kit.textures.get(&Material::Log)
            {
                let girth = core::f32::consts::PI * kit.diameter;
                let stretch = texture.metres[0] / girth;
                if !within(stretch, GIRTH) {
                    out.push(format!(
                        "its Log texture ({}) is {} m round, but it is laid once round each log, {:.2} m round: its bark would be drawn {:.1} times its size. Set its metres to ({:.2}, {})",
                        texture.path,
                        texture.metres[0],
                        girth,
                        girth / texture.metres[0],
                        girth,
                        texture.metres[1]
                    ));
                }
            }
        }
        Pattern::Roof => {
            let course = kit
                .textures
                .get(&Material::Covering)
                .map(|t| t.metres[1] / kit.roof.rows.max(1e-3));
            if let Some(course) = course
                && !within(course, COURSES)
            {
                out.push(format!(
                    "its covering's courses are {} up the roof ({} rows a copy), where a slate's or shingle's is 15 to 40 cm and thatch's up to {}: set its rows, or its Covering texture's metres",
                    cm(course),
                    kit.roof.rows,
                    cm(COURSES[1])
                ));
            }
            if !within(kit.roof.relief, ROOF_RELIEF) {
                out.push(format!(
                    "its courses stand out {}, where the roof pattern draws {} to {} well: set its roof's relief",
                    cm(kit.roof.relief),
                    cm(ROOF_RELIEF[0]),
                    cm(ROOF_RELIEF[1])
                ));
            } else if let Some(course) = course
                && kit.roof.relief > course * 0.5
            {
                out.push(format!(
                    "its courses stand out {}, more than half a course ({}): each would read as a step, not a slate or shingle. Set its roof's relief",
                    cm(kit.roof.relief),
                    cm(course)
                ));
            }
            if !within(kit.roof.thick, ROOF_THICK) {
                out.push(format!(
                    "its covering is {} thick, where the roof pattern lays {} (slate) to {} (thatch): set its roof's thick",
                    cm(kit.roof.thick),
                    cm(ROOF_THICK[0]),
                    cm(ROOF_THICK[1])
                ));
            }
        }
    }
    for (material, texture) in &kit.textures {
        if !within(texture.reflectance, REFLECTANCE) {
            out.push(format!(
                "{material:?} ({}): reflectance {} is not a rough building material's ({} to {}; 0.35 unless said): it would sheen as if wet or polished, or go dead. Set its reflectance",
                texture.path, texture.reflectance, REFLECTANCE[0], REFLECTANCE[1]
            ));
        }
        let depth = texture.depth * texture.metres[0];
        let most = if *material == Material::Floor {
            FLOOR_DEPTH
        } else {
            WALL_DEPTH
        };
        if depth > most {
            out.push(format!(
                "{material:?} ({}): its parallax reads {} deep (depth {} of {} m), more than the {} it draws without swimming: set its depth to {:.3} or less",
                texture.path,
                cm(depth),
                texture.depth,
                texture.metres[0],
                cm(most),
                most / texture.metres[0].max(1e-3)
            ));
        }
    }
}

/// How a texture set's height is used: how far it stands a surface out, in metres, and
/// whether that shapes what is drawn (a face, bark, a covering's courses, a core, parallax)
/// or only parts it for damage and lifts a floor's tops a little.
fn height_use(kit: &Kit, material: Material, texture: &crate::kit::Texture) -> Option<(f32, bool)> {
    let shaping = match (kit.pattern, material) {
        (Pattern::Masonry, Material::Masonry) => kit.relief,
        (Pattern::Roof, Material::Covering) => kit.roof.relief,
        (Pattern::Logs, Material::Log) if kit.timber.profile == Profile::Round => kit.timber.relief,
        _ => 0.0,
    };
    let cored = kit
        .framing
        .as_ref()
        .filter(|f| f.core == Some(material))
        .map_or(0.0, |f| f.core_relief);
    let parallax = texture.depth * texture.metres[0];
    let shaped = shaping.max(cored).max(parallax);
    if shaped > 0.0 {
        return Some((shaped, true));
    }
    let topped = kit.pattern == Pattern::Masonry && material == Material::Floor && kit.relief > 0.0;
    let parted = matches!(
        kit.damage.get(&material),
        Some(crate::kit::Breaks::Joints { .. } | crate::kit::Breaks::Shingles { .. })
    );
    (topped || parted).then_some((0.0, false))
}

fn maps(
    kit: &Kit,
    material: Material,
    texture: &crate::kit::Texture,
    m: &Measured,
    out: &mut Vec<String>,
) {
    let mut fail = |what: String| out.push(format!("{material:?} ({}): {what}", texture.path));
    // Atlases of pieces and of sawn ends are pictures side by side, not a surface that
    // repeats; only their surface is checked.
    let atlas = matches!(
        material,
        Material::Piece | Material::Trim | Material::LogEnd
    ) || texture.cells > 1;
    if m.roughness < ROUGHNESS {
        fail(format!(
            "its roughness reads glossy (a mean of {:.2}): a building material is rough. A gloss or smoothness map taken for roughness? Invert the green of its arm.jpg",
            m.roughness
        ));
    }
    if m.metal > METAL {
        fail(format!(
            "it reads as metal (a mean of {:.2} in the blue of its arm.jpg): no material of the {} pattern is. Make its arm.jpg's blue black",
            m.metal,
            pattern_name(kit)
        ));
    }
    if !atlas {
        for (axis, seam, along) in [
            ("across", m.seams[0], texture.metres[0]),
            ("down", m.seams[1], texture.metres[1]),
        ] {
            if seam > SEAM {
                fail(format!(
                    "it does not tile {axis}: where one copy meets the next the step is {seam:.1} times the largest inside it, a seam every {along} m. Make it tileable (a scan that tiles, or textures.ts material --tile) and pack it again"
                ));
            }
        }
    }
    let Some((relief, shapes)) = height_use(kit, material, texture) else {
        return;
    };
    if atlas {
        return;
    }
    if let Some([hw, hh]) = m.height_size {
        let [cw, ch] = m.size;
        let aspect = |w: usize, h: usize| w as f32 / h.max(1) as f32;
        if (aspect(hw, hh) - aspect(cw, ch)).abs() > 0.02 {
            fail(format!(
                "its height map is {hw} by {hh} but its colour map {cw} by {ch}: they are not one picture. Make them from the same source"
            ));
            return;
        }
    } else {
        return;
    }
    if m.agrees < INVERTED {
        fail(format!(
            "height map looks inverted: its joints are bright and its faces dark (it agrees {:.2} with its colour), so joints would stand proud and stones sink. Flip it with --invert-height",
            m.agrees
        ));
    } else if m.agrees.abs() < UNRELATED {
        fail(format!(
            "its height map does not match its colour map (they agree {:.2}): the relief would not be where the picture's stones, joints or courses are. Use the height made with the scan's colour",
            m.agrees
        ));
    }
    if m.noise > NOISE {
        fail(format!(
            "its height map is mostly noise ({:.0}% of it from one texel to the next): displaced, it reads as grit and breaks into specks. Use the scan's own height map, not one worked out from grain",
            m.noise * 100.0
        ));
    }
    if shapes && m.range < RANGE {
        fail(format!(
            "its height map is weak: it spans {:.0}% of its range, so the {} of relief asked for shows as {}. Stretch it with --normalise-height",
            m.range * 100.0,
            cm(relief),
            cm(relief * m.range)
        ));
    }
    // Its real size, from the cells its height parts it into: a block's stones and a floor's
    // flags; a face with no stones (earth, plaster) by the size of a copy of it.
    let Some(cells) = m.cells else {
        return;
    };
    if kit.pattern != Pattern::Masonry || !matches!(material, Material::Masonry | Material::Floor) {
        return;
    }
    let area = texture.metres[0] * texture.metres[1];
    let each = libm::sqrtf(area / cells.max(1) as f32);
    let (range, what, measured) = match (material, cells >= STONY) {
        (Material::Masonry, true) => (STONES, "stones are", each),
        (Material::Floor, true) => (FLAGS, "flags are", each),
        (Material::Masonry, false) => (PLAIN, "copies, having no stones, are", texture.metres[0]),
        _ => return,
    };
    if measured < range[0] || measured > range[1] {
        let aim = (range[0] + range[1]) * 0.5;
        let metres = texture.metres[0] * aim / measured;
        fail(format!(
            "texture scale is off: its {what} {} across at {} m a copy, where the {} pattern lays them {} to {}, so it is laid {}. Set its metres to about {metres:.1}, the scan's real size",
            cm(measured),
            texture.metres[0],
            pattern_name(kit),
            cm(range[0]),
            cm(range[1]),
            if measured > range[1] {
                "too large"
            } else {
                "too small"
            },
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn assets() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// A shipped map, decoded as the kit checker decodes it.
    fn load(path: &Path) -> Option<Picture> {
        use zune_core::bytestream::ZCursor;
        use zune_core::colorspace::ColorSpace;
        use zune_core::options::DecoderOptions;
        let bytes = std::fs::read(path).ok()?;
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut decoder =
            zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(&bytes[..]), options);
        let rgb = decoder.decode().ok()?;
        let (width, height) = decoder.dimensions()?;
        Some(Picture { width, height, rgb })
    }

    fn set(folder: &str) -> [Option<Picture>; 3] {
        let dir = assets().join(folder);
        ["color.jpg", "arm.jpg", "height.jpg"].map(|map| load(&dir.join(map)))
    }

    /// The shipped materials, by file, with the roles each one's own lines and packs give.
    fn shipped() -> Vec<(String, Kit, BTreeSet<Material>)> {
        let assets = assets();
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        Kit::read_folder(&assets)
            .unwrap()
            .into_iter()
            .map(|(name, kit)| {
                let text = read(&format!("{}/{name}.ron", crate::kit::MATERIALS)).unwrap();
                let given = crate::layers::own_textures(&text, &read)
                    .unwrap()
                    .iter()
                    .map(|role| ron::from_str::<Material>(role).unwrap())
                    .collect();
                (name, kit, given)
            })
            .collect()
    }

    fn kit(name: &str) -> (Kit, BTreeSet<Material>) {
        shipped()
            .into_iter()
            .find(|(n, ..)| n == name)
            .map(|(_, kit, given)| (kit, given))
            .unwrap()
    }

    /// A picture whose red, green and blue are `f` of each texel, 0 to 1.
    fn picture(side: usize, f: impl Fn(usize, usize) -> [f32; 3]) -> Picture {
        let mut rgb = Vec::with_capacity(side * side * 3);
        for y in 0..side {
            for x in 0..side {
                rgb.extend(f(x, y).map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8));
            }
        }
        Picture {
            width: side,
            height: side,
            rgb,
        }
    }

    /// A made stone wall that tiles, known from how it is made rather than measured: 8
    /// courses of 8 stones, each 32 texels square, each course shifted along by its own
    /// amount, joints 3 texels wide; each stone pillowed up from its joints. How high a
    /// texel is, 0 in a joint to 1 at a stone's middle.
    fn stones(x: usize, y: usize) -> f32 {
        let (course, long, joint) = (32usize, 32usize, 3usize);
        let (x, y) = (x % SIDE, y % SIDE);
        let shift = (y / course * 13) % long;
        let along = (x + shift) % long;
        let up = y % course;
        let edge = along.min(long - 1 - along).min(up.min(course - 1 - up));
        if edge < joint {
            0.0
        } else {
            (((edge - joint) as f32) / 6.0).min(1.0) * 0.7 + 0.3
        }
    }

    const SIDE: usize = 256;

    /// The made wall's maps: its height, its colour, and its roughness and metal.
    fn wall(
        height: impl Fn(usize, usize) -> f32,
        colour: impl Fn(usize, usize) -> f32,
        rough: f32,
        metal: f32,
    ) -> Measured {
        let colour = picture(SIDE, |x, y| [colour(x, y); 3]);
        let arm = picture(SIDE, |x, y| [0.4 + 0.6 * stones(x, y), rough, metal]);
        let height = picture(SIDE, |x, y| [height(x, y); 3]);
        measure(&colour, Some(&arm), Some(&height))
    }

    /// Light where it stands out, as a scan's stones are against their joints.
    fn lit(x: usize, y: usize) -> f32 {
        0.2 + 0.5 * stones(x, y)
    }

    fn good() -> Measured {
        wall(stones, lit, 0.8, 0.0)
    }

    /// The stone material with its walls' and floors' maps measured as `m`, at so many
    /// metres a copy, checked: its problems, one a line. The pattern's own sets are not
    /// read.
    fn stone_with(m: &Measured, metres: f32) -> String {
        let (mut kit, given) = kit("stone");
        for role in [Material::Masonry, Material::Floor] {
            let texture = kit.textures.get_mut(&role).unwrap();
            texture.path = "materials/made".into();
            texture.metres = [metres, metres];
            texture.depth = 0.0;
        }
        check(&kit, &given, &|path| {
            (path == "materials/made").then(|| m.clone())
        })
        .join("\n")
    }

    #[test]
    fn a_made_wall_that_is_right_passes() {
        let m = good();
        assert!(m.agrees > 0.5 && m.range > 0.5 && m.noise < 0.3, "{m:?}");
        assert!(m.seams[0] < SEAM && m.seams[1] < SEAM, "{m:?}");
        // 8 courses of 8 stones a copy.
        assert_eq!(m.cells, Some(64), "{m:?}");
        // At 2.4 m a copy, its stones are 30 cm.
        assert_eq!(stone_with(&m, 2.4), "");
    }

    #[test]
    fn an_inverted_height_map_is_refused_saying_how_to_flip_it() {
        let m = wall(|x, y| 1.0 - stones(x, y), lit, 0.8, 0.0);
        let said = stone_with(&m, 2.4);
        assert!(
            said.contains("height map looks inverted: its joints are bright")
                && said.contains("Flip it with --invert-height"),
            "{said}"
        );
    }

    #[test]
    fn a_texture_that_does_not_tile_is_refused() {
        // The same wall lit more toward one side and the bottom, as a photograph often is,
        // so its edges do not meet.
        let ramp = |x: usize, y: usize| (x + y) as f32 / (2 * SIDE) as f32;
        let m = wall(
            |x, y| stones(x, y) * 0.3 + ramp(x, y) * 0.7,
            |x, y| lit(x, y) * 0.5 + ramp(x, y) * 0.5,
            0.8,
            0.0,
        );
        let said = stone_with(&m, 2.4);
        for axis in ["across", "down"] {
            assert!(said.contains(&format!("it does not tile {axis}")), "{said}");
        }
    }

    #[test]
    fn a_texture_laid_far_from_its_real_size_is_refused() {
        let m = good();
        // Stones 1.25 m across, and 5 cm.
        let large = stone_with(&m, 10.0);
        assert!(
            large.contains("texture scale is off: its stones are 125 cm across")
                && large.contains("so it is laid too large. Set its metres to about 2.8"),
            "{large}"
        );
        assert!(stone_with(&m, 0.4).contains("laid too small"));
        // Logs far thicker than the coursed pattern lays, their bark stretched round them.
        let (mut logs, given) = kit("logs");
        logs.diameter = 0.8;
        let said = check(&logs, &given, &|_| None).join("\n");
        assert!(said.contains("its logs are 80 cm across"), "{said}");
        assert!(
            said.contains("its bark would be drawn 2.5 times its size"),
            "{said}"
        );
        // Slates as long as a door.
        let (mut slate, given) = kit("slate");
        slate.roof.rows = 1.0;
        let said = check(&slate, &given, &|_| None).join("\n");
        assert!(
            said.contains("its covering's courses are 240 cm up the roof"),
            "{said}"
        );
    }

    #[test]
    fn a_weak_noisy_or_unrelated_height_map_is_refused() {
        let weak = wall(|x, y| 0.45 + 0.05 * stones(x, y), lit, 0.8, 0.0);
        let said = stone_with(&weak, 2.4);
        assert!(
            said.contains("its height map is weak: it spans 5% of its range")
                && said.contains("--normalise-height"),
            "{said}"
        );
        // Noise hashed from each texel, the same every run.
        let noise = |x: usize, y: usize| {
            let mut h = (x as u32).wrapping_mul(0x9E37_79B1) ^ (y as u32).wrapping_mul(0x85EB_CA77);
            h ^= h >> 15;
            h = h.wrapping_mul(0x2C1B_3C6D);
            h ^= h >> 12;
            (h % 1000) as f32 / 1000.0
        };
        let said = stone_with(&wall(noise, lit, 0.8, 0.0), 2.4);
        assert!(said.contains("its height map is mostly noise"), "{said}");
        // Another scan's height under this one's colour: smooth swells that tile, with
        // nothing of its stones in them.
        let swells = |x: usize, y: usize| {
            let turn = core::f32::consts::TAU / SIDE as f32;
            let (x, y) = (x as f32 * turn, y as f32 * turn);
            0.5 + 0.25 * libm::sinf(3.0 * x + 1.0) * libm::cosf(2.0 * y)
                + 0.2 * libm::sinf(5.0 * (x + y))
        };
        let other = wall(swells, lit, 0.8, 0.0);
        let said = stone_with(&other, 2.4);
        assert!(said.contains("does not match its colour map"), "{said}");
    }

    #[test]
    fn glossy_or_metal_maps_and_a_shine_are_refused() {
        let glossy = wall(stones, lit, 0.1, 0.0);
        assert!(stone_with(&glossy, 2.4).contains("its roughness reads glossy"));
        let metal = wall(stones, lit, 0.8, 0.9);
        assert!(stone_with(&metal, 2.4).contains("it reads as metal"));
        let (mut stone, given) = kit("stone");
        stone
            .textures
            .get_mut(&Material::Masonry)
            .unwrap()
            .reflectance = 0.95;
        let said = check(&stone, &given, &|_| None).join("\n");
        assert!(
            said.contains("reflectance 0.95 is not a rough building material's"),
            "{said}"
        );
    }

    #[test]
    fn relief_and_parallax_past_what_a_pattern_draws_are_refused() {
        let (mut stone, given) = kit("stone");
        stone.relief = 0.3;
        stone.textures.get_mut(&Material::Floor).unwrap().depth = 0.1;
        let said = check(&stone, &given, &|_| None).join("\n");
        assert!(
            said.contains("relief 0.3 stands its face out 30 cm"),
            "{said}"
        );
        assert!(said.contains("its parallax reads 18 cm deep"), "{said}");
        let (mut thatch, given) = kit("thatch");
        thatch.roof.relief = 0.5;
        let said = check(&thatch, &given, &|_| None).join("\n");
        assert!(said.contains("its courses stand out 50 cm"), "{said}");
    }

    #[test]
    fn a_detail_that_tints_is_overdone_or_has_nothing_under_it_is_refused() {
        // Stone's own: grey, laid over its walls, passes.
        let (mut stone, given) = kit("stone");
        let grey = |path: &str| {
            path.contains("detail").then(|| Measured {
                brightness: if path.contains("tinted") { 0.8 } else { 0.5 },
                ..good()
            })
        };
        let detail = stone
            .details
            .get(&Material::Masonry)
            .cloned()
            .expect("stone's walls have detail");
        assert!(
            !check(&stone, &given, &grey)
                .iter()
                .any(|s| s.contains("detail"))
        );
        stone.details.insert(
            Material::Masonry,
            crate::kit::Detail {
                path: "materials/detail_tinted".into(),
                strength: 3.0,
                ..detail.clone()
            },
        );
        stone.details.insert(Material::Covering, detail);
        let said = check(&stone, &given, &grey).join("\n");
        assert!(said.contains("not grey about the middle"), "{said}");
        assert!(said.contains("strength 3, past"), "{said}");
        assert!(
            said.contains("Covering's detail (materials/detail_stone): it has no texture"),
            "{said}"
        );
    }

    #[test]
    fn a_role_its_pack_leaves_to_the_patterns_default_is_refused() {
        // Fieldstone's pack without its floors: the block pattern has none of its own.
        let (mut stone, mut given) = kit("stone");
        given.remove(&Material::Floor);
        stone.textures.remove(&Material::Floor);
        let said = check(&stone, &given, &|_| None).join("\n");
        assert!(
            said.contains(
                "its pack gives no Floor texture, which the block pattern asks every pack for"
            ) && said.contains("drawn in nothing"),
            "{said}"
        );
        // A wood whose pack gives no face, falling back to its pattern's hewn timber.
        let (mut logs, mut given) = kit("logs");
        given.remove(&Material::Log);
        let hewn = logs.textures[&Material::Hewn].clone();
        logs.textures.insert(Material::Log, hewn);
        let said = check(&logs, &given, &|_| None).join("\n");
        assert!(
            said.contains("no Log texture") && said.contains("pattern's default"),
            "{said}"
        );
        // Lime-wash gives its floors by the pack it is laid over, which counts.
        let (_, given) = kit("limewash");
        assert!(given.contains(&Material::Floor), "{given:?}");
    }

    #[test]
    fn a_field_its_pattern_never_reads_is_refused_naming_its_file() {
        let assets = assets();
        let read = |path: &str| {
            std::fs::read_to_string(assets.join(path)).map_err(|why| format!("{path}: {why}"))
        };
        let own = "data/kits/thatch.ron";
        let set = crate::layers::fields_set(own, &read(own).unwrap(), &read).unwrap();
        // Its pattern's, its pack's and its own fields, each by the file that sets it.
        for (file, field) in [
            ("data/kits/patterns/roof.ron", "eaves"),
            ("data/packs/thatch.ron", "roof.rows"),
            (own, "id"),
        ] {
            assert!(
                set.iter().any(|(f, n)| f == file && n == field),
                "{file} {field}: {set:?}"
            );
        }
        let (thatch, _) = kit("thatch");
        assert!(fields(&thatch, &set).is_empty());
        // Timber written into a roof's pack, as its packs once had it, and daub into a wall's.
        let mut baggage = set.clone();
        baggage.push(("data/packs/slate.ron".into(), "timber".into()));
        let said = fields(&thatch, &baggage).join("\n");
        assert!(
            said.contains("data/packs/slate.ron sets timber, which the roof pattern never reads (a roof lays no timber"),
            "{said}"
        );
        let (stone, _) = kit("stone");
        let said = fields(
            &stone,
            &[(
                "data/packs/fieldstone.ron".into(),
                "colours.chinking".into(),
            )],
        );
        assert_eq!(said.len(), 1, "{said:?}");
        // And a colour the kits no longer have does not read at all.
        assert!(ron::from_str::<crate::kit::KitColours>("(dark: (0.4, 0.38, 0.35))").is_err());
    }

    /// Every shipped material passes, its real maps read; and a real scan planted wrong
    /// (flipped, cropped so it does not tile, laid at the wrong size) fails as the made ones
    /// do.
    #[test]
    fn every_shipped_material_passes_and_a_real_scan_planted_wrong_fails() {
        let mut measures: BTreeMap<String, Option<Measured>> = BTreeMap::new();
        for (name, kit, given) in shipped() {
            for texture in kit.textures.values() {
                measures.entry(texture.path.clone()).or_insert_with(|| {
                    let [colour, arm, height] = set(&texture.path);
                    colour.map(|colour| measure(&colour, arm.as_ref(), height.as_ref()))
                });
            }
            let said = check(&kit, &given, &|path| measures.get(path).cloned().flatten());
            assert!(said.is_empty(), "{name}: {said:?}");
            let file = format!("{}/{name}.ron", crate::kit::MATERIALS);
            let read = |path: &str| {
                std::fs::read_to_string(assets().join(path)).map_err(|why| format!("{path}: {why}"))
            };
            let set = crate::layers::fields_set(&file, &read(&file).unwrap(), &read).unwrap();
            assert_eq!(fields(&kit, &set), Vec::<String>::new(), "{name}");
        }
        let [colour, arm, height] = set("materials/stone_wall");
        let (colour, height) = (colour.unwrap(), height.unwrap());
        let right = measure(&colour, arm.as_ref(), Some(&height));
        assert_eq!(stone_with(&right, 2.0), "");
        assert!(stone_with(&right, 9.0).contains("laid too large"));
        let flipped = Picture {
            rgb: height.rgb.iter().map(|v| 255 - v).collect(),
            ..height.clone()
        };
        let said = stone_with(&measure(&colour, arm.as_ref(), Some(&flipped)), 2.0);
        assert!(said.contains("height map looks inverted"), "{said}");
        // Its middle two thirds, which no longer meet themselves at their edges.
        let crop = |p: &Picture| {
            let side = p.width * 2 / 3;
            let rgb = (0..side)
                .flat_map(|y| (0..side).map(move |x| (x, y)))
                .flat_map(|(x, y)| {
                    let at = ((y + p.height / 6) * p.width + x + p.width / 6) * 3;
                    p.rgb[at..at + 3].to_vec()
                })
                .collect();
            Picture {
                width: side,
                height: side,
                rgb,
            }
        };
        let cropped = measure(
            &crop(&colour),
            arm.as_ref().map(crop).as_ref(),
            Some(&crop(&height)),
        );
        let said = stone_with(&cropped, 2.0);
        assert!(said.contains("it does not tile"), "{said}");
    }
}
