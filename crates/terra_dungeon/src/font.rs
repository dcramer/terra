//! A 3 by 5 pixel font for the map sheet's labels: digits, capitals and a little punctuation.
//! Each glyph is five rows, top first, three bits a row, the left pixel the high bit.

const GLYPHS: &[(char, [u8; 5])] = &[
    ('0', [7, 5, 5, 5, 7]),
    ('1', [2, 6, 2, 2, 7]),
    ('2', [7, 1, 7, 4, 7]),
    ('3', [7, 1, 7, 1, 7]),
    ('4', [5, 5, 7, 1, 1]),
    ('5', [7, 4, 7, 1, 7]),
    ('6', [7, 4, 7, 5, 7]),
    ('7', [7, 1, 1, 1, 1]),
    ('8', [7, 5, 7, 5, 7]),
    ('9', [7, 5, 7, 1, 7]),
    ('A', [2, 5, 7, 5, 5]),
    ('B', [6, 5, 6, 5, 6]),
    ('C', [3, 4, 4, 4, 3]),
    ('D', [6, 5, 5, 5, 6]),
    ('E', [7, 4, 6, 4, 7]),
    ('F', [7, 4, 6, 4, 4]),
    ('G', [3, 4, 5, 5, 3]),
    ('H', [5, 5, 7, 5, 5]),
    ('I', [7, 2, 2, 2, 7]),
    ('J', [1, 1, 1, 5, 2]),
    ('K', [5, 5, 6, 5, 5]),
    ('L', [4, 4, 4, 4, 7]),
    ('M', [5, 7, 7, 5, 5]),
    ('N', [6, 5, 5, 5, 5]),
    ('O', [2, 5, 5, 5, 2]),
    ('P', [6, 5, 6, 4, 4]),
    ('Q', [2, 5, 5, 6, 3]),
    ('R', [6, 5, 6, 5, 5]),
    ('S', [3, 4, 2, 1, 6]),
    ('T', [7, 2, 2, 2, 2]),
    ('U', [5, 5, 5, 5, 7]),
    ('V', [5, 5, 5, 5, 2]),
    ('W', [5, 5, 7, 7, 5]),
    ('X', [5, 5, 2, 5, 5]),
    ('Y', [5, 5, 2, 2, 2]),
    ('Z', [7, 1, 2, 4, 7]),
    (':', [0, 2, 0, 2, 0]),
    ('.', [0, 0, 0, 0, 2]),
    (',', [0, 0, 0, 2, 4]),
    ('-', [0, 0, 7, 0, 0]),
    ('%', [5, 1, 2, 4, 5]),
    ('/', [1, 1, 2, 4, 4]),
    ('_', [0, 0, 0, 0, 7]),
    ('(', [1, 2, 2, 2, 1]),
    (')', [4, 2, 2, 2, 4]),
];

/// The glyph's rows, a blank for anything it lacks (lower case is drawn as capitals).
pub fn glyph(c: char) -> [u8; 5] {
    let c = c.to_ascii_uppercase();
    GLYPHS
        .iter()
        .find(|(g, _)| *g == c)
        .map_or([0; 5], |(_, rows)| *rows)
}

/// Whether pixel (x, y) of a glyph is lit, x from the left, y from the top.
pub fn lit(c: char, x: usize, y: usize) -> bool {
    x < 3 && y < 5 && glyph(c)[y] & (4 >> x) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_is_distinct_and_in_its_box() {
        for (i, (a, rows)) in GLYPHS.iter().enumerate() {
            assert!(rows.iter().all(|r| *r < 8), "{a}");
            for (b, other) in &GLYPHS[i + 1..] {
                assert_ne!(rows, other, "{a} and {b} look alike");
            }
        }
        assert!(lit('1', 1, 0) && !lit('1', 0, 0));
    }
}
