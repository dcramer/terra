//! Snow lying on a day, from the climate's integrated past (climate.md, D314): what fell below
//! freezing less what warmth and sun melted since the fold began, at each height, latitude and
//! leaning of the ground, so peaks keep it into summer, slopes facing the pole longest, and
//! the low ridges are bare (docs/demos.md §1). Worked out ahead over a region's span, as a
//! fold takes most of a year of days, and blended between.
//!
//! The weather's fronts differ from place to place; a table folded at one place takes that
//! place's weather for the whole region, which on a region of a few kilometres is the same
//! weather (fronts are 3 km across and drift over it). What the ground's shelter does to drifting
//! snow (D323's catch) is still to come.

use terra_climate::{Fields, Place};

/// Snow by latitude, height over the sea and leaning toward the equator, on one day.
#[derive(Clone, Debug)]
pub struct Snow {
    pub day: i64,
    latitude: (f32, f32),
    height_m: (f32, f32),
    sunward: (f32, f32),
    steps: (usize, usize, usize),
    /// Snow depth, in metres, by latitude, then height, then leaning.
    table: Vec<f32>,
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
            let place = Place {
                at,
                height_m: value(height_m, j, steps.1),
                latitude: value(latitude, i, steps.0),
                sunward: value(sunward, k, steps.2),
            };
            // Snow too high for anything to ever melt it would pile up for ever; the fold
            // starts at the summer before, so it holds a winter's at most.
            terra_climate::past(fields, place, day).snow_depth_m()
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

    /// How deep the snow lies, in metres, at a latitude, a height over the sea and a leaning.
    pub fn depth_m(&self, latitude: f32, height_m: f32, sunward: f32) -> f32 {
        let index = |v: f32, range: (f32, f32), of: usize| {
            let t = ((v - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) * of as f32;
            let i = (t as usize).min(of.saturating_sub(1));
            (i, t - i as f32)
        };
        let (i, fi) = index(latitude, self.latitude, self.steps.0);
        let (j, fj) = index(height_m, self.height_m, self.steps.1);
        let (k, fk) = index(sunward, self.sunward, self.steps.2);
        let get = |a: usize, b: usize, c: usize| {
            self.table[(a.min(self.steps.0) * (self.steps.1 + 1) + b.min(self.steps.1))
                * (self.steps.2 + 1)
                + c.min(self.steps.2)]
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

    #[test]
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
            },
            day,
        )
        .snow_depth_m();
        assert!((snow.depth_m(latitude, 2500.0, 0.0) - exact).abs() < 0.02);
    }
}
