//! How open the ground round a point is against its own plane (D434): 1 on a plane at any
//! slope and on a crest, falling toward 0 in a gully's bed or the corner under a cliff
//! band. A far chunk's mesh is too coarse to shade its gullies and benches, and its sky
//! light came in alike on every one of them: the "smooth plaster" of a far wall at dusk.
//!
//! The terrain's cavity or occlusion mask, as World Machine and Gaea bake from a
//! heightfield ("Occlusion", "Curvature") and as Battlefield 3 put its terrain's occlusion
//! into the colour it draws far off (Widmark, "Terrain in Battlefield 3", GDC 2012). Ours
//! asks the field itself, so it holds relief finer than the mesh that carries it.

use crate::field::Field;
use glam::Vec3;

/// The probes' climb above the ground's plane: 25 degrees, as its cosine and sine.
const CLIMB: (f32, f32) = (0.906_307_8, 0.422_618_26);
/// Six ways round the normal, as cosines and sines of multiples of 60 degrees.
const AROUND: [(f32, f32); 6] = [
    (1.0, 0.0),
    (0.5, 0.866_025_4),
    (-0.5, 0.866_025_4),
    (-1.0, 0.0),
    (-0.5, -0.866_025_4),
    (0.5, -0.866_025_4),
];

/// How open the ground at `point`, facing `normal`, is within each of `reaches_m`: the mean,
/// over probes climbing 25 degrees off its plane six ways round, of how high above the
/// ground each stands against how high it would over the plane, capped at 1.
///
/// A field's distance is measured straight up from the centre, so over a plane tilted from
/// the level a probe stands higher by the tilt's secant: scaled back by the normal's share
/// of the up, a plane gives 1 at every slope, and only the ground's bending shows.
pub fn openness<F: Field>(
    field: &F,
    memo: &mut F::Memo,
    point: Vec3,
    normal: Vec3,
    reaches_m: &[f32],
) -> f32 {
    let up = point.normalize_or(Vec3::Y);
    let level = normal.dot(up).max(0.2);
    let across = normal.any_orthonormal_vector();
    let along = normal.cross(across);
    let mut open = 0.0;
    let mut asked = 0.0;
    for &(c, s) in &AROUND {
        let way = (across * c + along * s) * CLIMB.0 + normal * CLIMB.1;
        for &reach in reaches_m {
            let height = field.distance_in(point + way * reach, memo);
            open += (height * level / (reach * CLIMB.1)).clamp(0.0, 1.0);
            asked += 1.0;
        }
    }
    if asked > 0.0 { open / asked } else { 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sample;
    use crate::field::material;

    /// A world this big, so a few hundred metres of it stand level to well under a degree,
    /// and small enough that its points keep centimetres.
    const RADIUS_M: f32 = 100_000.0;

    /// Ground at the top of a round world, its height a function of the level ways across.
    struct Ground(fn(f32, f32) -> f32);

    impl Field for Ground {
        type Memo = ();

        fn distance_in(&self, point: Vec3, _: &mut ()) -> f32 {
            point.length() - (RADIUS_M + (self.0)(point.x, point.z))
        }

        fn sample_in(&self, point: Vec3, memo: &mut ()) -> Sample {
            Sample {
                distance: self.distance_in(point, memo),
                material: material::STONE,
            }
        }
    }

    /// The point on the ground over (x, z), and its normal, from the height's slope there.
    fn on(ground: &Ground, x: f32, z: f32) -> (Vec3, Vec3) {
        let d = 0.01;
        let h = |x, z| (ground.0)(x, z);
        let gx = (h(x + d, z) - h(x - d, z)) / (2.0 * d);
        let gz = (h(x, z + d) - h(x, z - d)) / (2.0 * d);
        let point = Vec3::new(x, 0.0, z).with_y(RADIUS_M + h(x, z));
        (point, Vec3::new(-gx, 1.0, -gz).normalize())
    }

    const REACHES: [f32; 2] = [14.0, 42.0];

    fn open_at(ground: &Ground, x: f32, z: f32) -> f32 {
        let (point, normal) = on(ground, x, z);
        openness(ground, &mut (), point, normal, &REACHES)
    }

    #[test]
    fn a_plane_is_open_at_every_slope() {
        // Level, 30 degrees and 70, the steepest a wall stands: the probes' secant scaling
        // keeps a steep plane from reading as shut in, as a sky-seen measure would.
        let level = Ground(|_, _| 0.0);
        let thirty = Ground(|x, _| x * 0.577);
        let seventy = Ground(|x, z| x * 2.747 + z * 0.1);
        for (name, ground) in [("level", &level), ("30", &thirty), ("70", &seventy)] {
            let open = open_at(ground, 3.0, -2.0);
            assert!(
                (open - 1.0).abs() < 0.03,
                "{name}: a plane read {open:.3} open"
            );
        }
    }

    #[test]
    fn a_gully_bed_is_shut_in_and_its_ribs_are_open() {
        // Gullies 60 m apart cut 15 m deep down a 35-degree wall, as a range's middle set.
        let wall = Ground(|x, z| {
            x * 0.7 - 15.0 * (0.5 + 0.5 * libm::cosf(z * std::f32::consts::TAU / 60.0))
        });
        let bed = open_at(&wall, 0.0, 0.0);
        let side = open_at(&wall, 0.0, 15.0);
        let rib = open_at(&wall, 0.0, 30.0);
        assert!(bed < 0.75, "the gully's bed read {bed:.3} open");
        assert!(rib > 0.97, "the rib between read {rib:.3} open");
        assert!(
            bed < side && side < rib,
            "bed {bed:.3}, side {side:.3}, rib {rib:.3}"
        );
    }

    #[test]
    fn the_foot_of_a_cliff_band_is_shut_in_and_the_bench_lip_is_open() {
        // A bench under a riser 40 m high: its back corner is shut in, its open middle
        // 60 m out is not.
        let banded = Ground(|x, _| if x < 0.0 { 0.0 } else { (x * 4.0).min(40.0) });
        let corner = open_at(&banded, -2.0, 0.0);
        let lip = open_at(&banded, -60.0, 0.0);
        assert!(corner < 0.8, "the riser's foot read {corner:.3} open");
        assert!(lip > 0.97, "the bench's open middle read {lip:.3}");
    }
}
