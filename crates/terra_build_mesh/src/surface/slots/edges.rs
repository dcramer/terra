//! Which edges a block surface's relief fades out at.

/// Whether the relief fades out at an edge, from the four cubes round it (in the order
/// (0, 0), (-1, 0), (0, -1), (-1, -1) across it), each full or not and drawn into this
/// surface or not. An inside edge: three of the four full, where the faces either side push
/// different ways. Or a seam: a face running on across the edge in one plane into a cube of
/// another kind that hides it, two full side by side, one of them drawn here; each kind's
/// relief pushing its own way would part their edges into a slit into what is behind (the
/// sun came through one between a mine's earth ceiling and a stone wall's foot standing in
/// it), so faded out there the two meet in the plane.
pub(super) fn fades(full: [bool; 4], member: [bool; 4]) -> bool {
    match full.iter().filter(|&&f| f).count() {
        3 => true,
        2 => [(0, 1), (0, 2), (1, 3), (2, 3)]
            .iter()
            .any(|&(i, j)| full[i] && full[j] && member[i] != member[j]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::fades;

    #[test]
    fn an_inside_edge_and_a_seam_with_another_kind_fade_and_a_flat_face_or_a_pinch_does_not() {
        let (t, f) = (true, false);
        // Three full: an inside edge, whoever draws them.
        assert!(fades([t, t, t, f], [t, f, f, f]));
        // Two side by side, one of another kind: a seam; both drawn here: a flat face.
        assert!(fades([t, t, f, f], [t, f, f, f]));
        assert!(fades([t, f, t, f], [f, f, t, f]));
        assert!(!fades([t, t, f, f], [t, t, f, f]));
        // Two diagonally across: a pinch, not a seam.
        assert!(!fades([t, f, f, t], [t, f, f, f]));
        // One full: an outside edge.
        assert!(!fades([t, f, f, f], [t, f, f, f]));
    }
}
