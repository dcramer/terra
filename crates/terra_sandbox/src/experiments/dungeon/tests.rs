use super::*;
use terra_core::PlanetSpec;

/// The ground over the digs is asked so far apart, in metres.
const STEP_M: f32 = 1.0;

/// Whether a point lies inside one of the stamp's dig boxes at its frame, deeper than `by`.
fn in_a_dig(stamp: &Stamp, frame: &Transform, point: Vec3, by: f32) -> bool {
    let local = frame.rotation.inverse() * (point - frame.translation);
    stamp.digs.iter().any(|dig| {
        let off = (local - dig.centre).abs();
        off.cmplt(dig.half - Vec3::splat(by)).all()
    })
}

/// Nothing is planted inside a dungeon: over every demo's dig boxes, wherever the forest's
/// placements ask the ground (a point on the start's level, as the lush forest's trees,
/// floor and grass do, its foot sunk as deep as a tree's), what the planting rule lets grow
/// (`!Planet::dug`) stands outside every box; and the stair's opening to the sky, where the
/// ground found is the dungeon's floor, is met and refused, so the rule is tested.
#[test]
fn no_plant_stands_inside_a_dungeons_digs() {
    let demos: Demos = terra_world::data::read(FILE);
    let themes = Theme::all(&terra_world::data::assets()).unwrap();
    // A demo a core: digging a dungeon's boxes measures what each took out, seconds apiece.
    terra_gen::par::map(&demos.demos, |demo| {
        let theme = themes.iter().find(|t| t.name == demo.theme).unwrap();
        let dungeon = generate(theme, demo.seed).unwrap();
        let stamp = stamp::stamp(&dungeon, theme, &|_| Default::default());
        let mut planet = Planet::new(PlanetSpec {
            seed: Seed(1),
            radius_m: 2000.0,
        });
        let start = Start::choose(&planet, true);
        let frame = site(&planet, &start, &stamp, demos.near, demos.within);
        dig_in(&mut planet, &stamp, &frame);
        let (mut low, mut high) = (Vec3::MAX, Vec3::MIN);
        for dig in &stamp.digs {
            low = low.min(dig.centre - dig.half);
            high = high.max(dig.centre + dig.half);
        }
        let rows: Vec<f32> = (0..)
            .map(|i| low.x - 1.0 + i as f32 * STEP_M)
            .take_while(|&x| x <= high.x + 1.0)
            .collect();
        let counts: Vec<(usize, usize)> = rows
            .iter()
            .map(|&x| {
                let (mut grown, mut refused) = (0, 0);
                let mut z = low.z - 1.0;
                while z <= high.z + 1.0 {
                    let local = start.local(frame.transform_point(Vec3::new(x, 0.0, z)));
                    let ground = planet.ground_at(start.at(local.x, local.z));
                    let up = ground.normalize();
                    // Where its leaves and stems are.
                    let body = ground + up * 0.3;
                    for sink in [0.0, 0.3] {
                        let foot = ground - up * sink;
                        if planet.dug(foot) {
                            refused += usize::from(in_a_dig(&stamp, &frame, body, 0.0));
                            continue;
                        }
                        grown += 1;
                        assert!(
                            !in_a_dig(&stamp, &frame, foot, 0.02)
                                && !in_a_dig(&stamp, &frame, body, 0.02),
                            "{}: a plant's ground at {x:.1},{z:.1} (sunk {sink}) is inside a dig",
                            demo.experiment
                        );
                    }
                    z += STEP_M;
                }
                (grown, refused)
            })
            .collect();
        let grown: usize = counts.iter().map(|c| c.0).sum();
        let refused: usize = counts.iter().map(|c| c.1).sum();
        assert!(grown > 500, "{}: {grown} grown", demo.experiment);
        assert!(
            refused > 10,
            "{}: the stair's opening met {refused} times",
            demo.experiment
        );
    });
}

/// The eye opens as in a room all the way down a dungeon's stair, not only once in a room's
/// box: the mine's first view, at the stair's foot, was dark for a second and a half as the
/// eye was held to the outdoor band (mine round 11). Over every demo's tour, each eye under
/// the ground's level is underground, and the first, on the surface, is not.
#[test]
fn a_tour_is_underground_from_the_top_of_the_stair_down() {
    let demos: Demos = terra_world::data::read(FILE);
    let themes = Theme::all(&terra_world::data::assets()).unwrap();
    terra_gen::par::map(&demos.demos, |demo| {
        let theme = themes.iter().find(|t| t.name == demo.theme).unwrap();
        let dungeon = generate(theme, demo.seed).unwrap();
        let stamp = stamp::stamp(&dungeon, theme, &|_| Default::default());
        let interior = interior::Interior::new(&stamp, Transform::IDENTITY);
        let name = &demo.experiment;
        assert!(
            !interior.underground(stamp.view[0]),
            "{name}: the tour's first eye is on the surface"
        );
        let below: Vec<_> = stamp.view.iter().filter(|e| e.y < stamp.start.y).collect();
        assert!(
            below.len() * 2 > stamp.view.len(),
            "{name}: the tour goes down"
        );
        let out: Vec<_> = below
            .iter()
            .filter(|&&&eye| !interior.underground(eye))
            .collect();
        assert!(
            out.is_empty(),
            "{name}: {} of {} eyes under the ground are not underground, the first at {:?}",
            out.len(),
            below.len(),
            out.first()
        );
    });
}
