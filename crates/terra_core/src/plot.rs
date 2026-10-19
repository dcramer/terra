//! A plot: the ground a building's floor is set on, as the world chooses a place for it
//! before anything is built (D407). Plain data, so the world fits its start to a base's
//! floor without naming building, and building's tests hold that its base fits.

/// Rectangles of floor on a grid, in metres east and south of the plot's corner.
#[derive(Clone, Copy, Debug)]
pub struct Plot {
    /// Each rectangle from its north-west corner to its south-east, as (east, south); they
    /// may overlap.
    pub floors: &'static [([f32; 2], [f32; 2])],
    /// How wide a square of the grid is, in metres.
    pub square_m: f32,
}

impl Plot {
    /// The start's plot: the example base's ground floor inside its walls, two rooms joined
    /// by a corridor, on building's half-metre grid. The start is chosen as ground this fits on, so a run
    /// with no building in it starts in the same place as one with.
    pub const START: Plot = Plot {
        floors: &[
            ([-5.5, -5.5], [8.5, 2.5]),
            ([8.5, -2.5], [15.5, -0.5]),
            ([15.5, -5.5], [20.5, 2.5]),
        ],
        square_m: 0.5,
    };

    /// The squares the floors cover, as (east, south) counts of squares from the corner;
    /// a square two rectangles share comes once for each.
    pub fn squares(&self) -> impl Iterator<Item = [i32; 2]> + '_ {
        let at = |metres: f32| (metres / self.square_m).round() as i32;
        self.floors.iter().flat_map(move |&(low, high)| {
            (at(low[0])..at(high[0]))
                .flat_map(move |east| (at(low[1])..at(high[1])).map(move |south| [east, south]))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plot_covers_its_floors_squares_and_no_more() {
        let plot = Plot {
            floors: &[([-1.0, 0.0], [0.0, 0.5]), ([0.0, 0.0], [0.5, 1.0])],
            square_m: 0.5,
        };
        let squares: Vec<_> = plot.squares().collect();
        assert_eq!(squares, [[-2, 0], [-1, 0], [0, 0], [0, 1]]);
    }
}
