//! Snow lying on a day, from the climate's integrated past (climate.md, D314): what fell below
//! freezing less what warmth and sun melted since the fold began, at each height, latitude and
//! leaning of the ground, so peaks keep it into summer, slopes facing the pole longest, and
//! the low ridges are bare (docs/demos.md §1). Worked out ahead over a region's span, as a
//! fold takes most of a year of days, and blended between.
//!
//! The weather's fronts differ from place to place; a table folded at one place takes that
//! place's weather for the whole region, which on a region of a few kilometres is the same
//! weather (fronts are tens of kilometres across, D574). The ground's shelter from the wind bends
//! it too (D323): a table is folded at a scoured rise's catch, open ground's and a lee hollow's
//! (`drift`), and a place's snow is blended between them by its own.

use terra_climate::{Fields, Place};

/// The catches a table is folded at: a scoured rise's, open ground's and a lee hollow's.
const CATCHES: [f32; 3] = [crate::drift::SCOURED_CATCH, 1.0, crate::drift::LEE_CATCH];

/// Snow by latitude, height over the sea, leaning toward the equator and catch, on one day.
#[derive(Clone, Debug)]
pub struct Snow {
    pub day: i64,
    latitude: (f32, f32),
    height_m: (f32, f32),
    sunward: (f32, f32),
    steps: (usize, usize, usize),
    /// Snow depth, in metres, by latitude, then height, then leaning, each at the three
    /// catches.
    table: Vec<[f32; 3]>,
}

impl Snow {
    /// The snow on a day over latitudes and heights, as it lies where `at` (a point in the
    /// world) is, so for the region round it.
    pub fn on(
        fields: &Fields,
        day: i64,
        at: glam::Vec3,
        latitude: (f32, f32),
        height_m: (f32, f32),
    ) -> Self {
        let sunward = (-0.9, 0.9);
        let steps = (
            (((latitude.1 - latitude.0).to_degrees() / 0.25).ceil() as usize).max(1),
            (((height_m.1 - height_m.0) / 50.0).ceil() as usize).max(1),
            6,
        );
        let points: Vec<(usize, usize, usize)> = (0..=steps.0)
            .flat_map(|i| (0..=steps.1).flat_map(move |j| (0..=steps.2).map(move |k| (i, j, k))))
            .collect();
        let value = |range: (f32, f32), step: usize, of: usize| {
            range.0 + (range.1 - range.0) * step as f32 / of as f32
        };
        let table = crate::maps::par_map(&points, |&(i, j, k)| {
            // Snow too high for anything to ever melt it would pile up for ever; the fold
            // starts at the summer before, so it holds a winter's at most.
            CATCHES.map(|catch| {
                let place = Place {
                    at,
                    height_m: value(height_m, j, steps.1),
                    latitude: value(latitude, i, steps.0),
                    sunward: value(sunward, k, steps.2),
                    catch,
                };
                terra_climate::past(fields, place, day).snow_depth_m()
            })
        });
        Self {
            day,
            latitude,
            height_m,
            sunward,
            steps,
            table,
        }
    }

    /// How deep the snow lies, in metres, at a latitude, a height over the sea and a leaning,
    /// on open ground.
    pub fn depth_m(&self, latitude: f32, height_m: f32, sunward: f32) -> f32 {
        self.depth_caught_m(latitude, height_m, sunward, 1.0)
    }

    /// How deep the snow lies, in metres, where the ground catches so much of what falls
    /// (`Place::catch`, D323).
    pub fn depth_caught_m(&self, latitude: f32, height_m: f32, sunward: f32, catch: f32) -> f32 {
        let index = |v: f32, range: (f32, f32), of: usize| {
            let t = ((v - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) * of as f32;
            let i = (t as usize).min(of.saturating_sub(1));
            (i, t - i as f32)
        };
        let (i, fi) = index(latitude, self.latitude, self.steps.0);
        let (j, fj) = index(height_m, self.height_m, self.steps.1);
        let (k, fk) = index(sunward, self.sunward, self.steps.2);
        // Between the catches the table was folded at, the snow is blended as the catch is.
        let catch = catch.clamp(CATCHES[0], CATCHES[2]);
        let (m, fm) = if catch < CATCHES[1] {
            (0, (catch - CATCHES[0]) / (CATCHES[1] - CATCHES[0]))
        } else {
            (1, (catch - CATCHES[1]) / (CATCHES[2] - CATCHES[1]))
        };
        let get = |a: usize, b: usize, c: usize| {
            let at = self.table[(a.min(self.steps.0) * (self.steps.1 + 1) + b.min(self.steps.1))
                * (self.steps.2 + 1)
                + c.min(self.steps.2)];
            at[m] + (at[m + 1] - at[m]) * fm
        };
        let lerp = |x: f32, y: f32, t: f32| x + (y - x) * t;
        let along_k = |a, b| lerp(get(a, b, k), get(a, b, k + 1), fk);
        let along_j = |a| lerp(along_k(a, j), along_k(a, j + 1), fj);
        lerp(along_j(i), along_j(i + 1), fi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test] // Slow: minutes.
    fn early_summer_snow_lies_on_the_heights_longest_facing_the_pole() {
        let climate = crate::start_climate().unwrap();
        let fields = Fields::new(climate.clone(), terra_gen::Seed(1));
        let day = terra_climate::Date::parse("05-25")
            .unwrap()
            .day(&climate.year);
        let at = Vec3::Y * 30_000.0;
        let latitude = 42f32.to_radians();
        let snow = Snow::on(
            &fields,
            day,
            at,
            (latitude - 0.02, latitude + 0.02),
            (0.0, 3200.0),
        );
        // Low ground is bare, high ground white, and colder faces hold more.
        assert_eq!(snow.depth_m(latitude, 500.0, 0.0), 0.0);
        assert!(snow.depth_m(latitude, 3000.0, 0.0) > 0.2);
        let mut found_line = false;
        for h in (0..32).map(|k| k as f32 * 100.0) {
            let level = snow.depth_m(latitude, h, 0.0);
            let shaded = snow.depth_m(latitude, h, -0.5);
            let sunny = snow.depth_m(latitude, h, 0.5);
            assert!(
                shaded >= level - 1e-3 && level >= sunny - 1e-3,
                "{h}: {sunny} {level} {shaded}"
            );
            found_line |= shaded > 0.05 && sunny < 0.05;
        }
        assert!(
            found_line,
            "somewhere a shaded slope keeps snow a sunny one has lost"
        );
        // The table is the fold's own at its points.
        let exact = terra_climate::past(
            &fields,
            Place {
                at,
                height_m: 2500.0,
                latitude,
                sunward: 0.0,
                catch: 1.0,
            },
            day,
        )
        .snow_depth_m();
        assert!((snow.depth_m(latitude, 2500.0, 0.0) - exact).abs() < 0.02);
    }

    #[test] // Slow: minutes.
    fn a_lee_hollow_keeps_its_snow_longer_than_open_ground_and_a_rise_less() {
        let climate = crate::start_climate().unwrap();
        let fields = Fields::new(climate.clone(), terra_gen::Seed(1));
        let day = terra_climate::Date::parse("06-01")
            .unwrap()
            .day(&climate.year);
        let at = Vec3::Y * 30_000.0;
        let latitude = 50f32.to_radians();
        let snow = Snow::on(
            &fields,
            day,
            at,
            (latitude - 0.02, latitude + 0.02),
            (0.0, 3000.0),
        );
        let mut patch = false;
        for h in (0..30).map(|k| k as f32 * 100.0) {
            let scoured = snow.depth_caught_m(latitude, h, 0.0, 0.3);
            let open = snow.depth_m(latitude, h, 0.0);
            let lee = snow.depth_caught_m(latitude, h, 0.0, 2.0);
            assert!(
                lee >= open - 1e-3 && open >= scoured - 1e-3,
                "{h}: {scoured} {open} {lee}"
            );
            // Somewhere the lee keeps a patch where open ground is bare.
            patch |= lee > 0.2 && open < 0.05;
        }
        assert!(patch, "no height where only the lee keeps snow");
        // At a catch it was folded at, the table is the fold's own.
        let exact = terra_climate::past(
            &fields,
            Place {
                at,
                height_m: 1500.0,
                latitude,
                sunward: 0.0,
                catch: crate::drift::LEE_CATCH,
            },
            day,
        )
        .snow_depth_m();
        let table = snow.depth_caught_m(latitude, 1500.0, 0.0, crate::drift::LEE_CATCH);
        assert!((table - exact).abs() < 0.02, "{table} {exact}");
    }
}
