// The line from a third-person lens to the figure it follows, kept clear of leaves (D271, D274,
// D624): how much of a leaf at `world` stays drawn, 1 whole, 0 gone, for one shader to dither
// by. A leaf in front of the figure and within a circle round it on screen goes; as far from
// it as the figure's own depth, or beside it, it stays, so a low bough between the lens and the
// figure opens over the figure alone, never the whole tree, and the leaves round the figure
// still stand. Unreal third-person games cut theirs so in a masked material: a SphereMask
// node round the character's place on screen, for pixels nearer the camera than the
// character, fed to the opacity mask through the DitherTemporalAA material function, which
// TAA resolves (research/foliage-near-camera.md). The circle is a cone from the lens, so it
// is the same size on screen however far the leaf is; one dot product, one length and two
// ramps a fragment, dithered by the noise the rest of clear_view uses (Jimenez's).
//
// Both foliage passes import it, so the prepass's depth and the main pass's colour agree; the
// caller asks it only in the main camera's own view, so the sun's shadows do not move with it.
fn sight_keep(world: vec3<f32>, eye: vec3<f32>, subject: vec3<f32>) -> f32 {
    let line = subject - eye;
    let far = length(line);
    if far < 0.5 {
        return 1.0;
    }
    let ahead = line / far;
    let to = world - eye;
    let along = dot(to, ahead);
    if along <= 0.0 || along >= far {
        return 1.0;
    }
    // How far it is from the line, as if at the figure's depth: its circle on screen.
    let across = length(to - ahead * along) * far / along;
    let ring = smoothstep(SIGHT_M - SIGHT_BAND_M, SIGHT_M, across);
    // What stands at the figure, or a little before it, stays.
    let at_figure = smoothstep(far - SIGHT_FRONT_M, far - 0.5 * SIGHT_FRONT_M, along);
    return max(ring, at_figure);
}

// Leaves are whole from this far round the figure's chest, at its depth, and gone this much
// nearer: the figure is some 1.8 m tall with its chest 1.3 m up, so the circle holds it from
// the head to the knees.
const SIGHT_M: f32 = 0.9;
const SIGHT_BAND_M: f32 = 0.35;
// Leaves this close before the figure's chest are its own surroundings, kept, half of it a
// ramp; nearer the lens they go. A figure in a hazel had leaves within 0.35 m before its
// chest, so the cut reaches nearly to the body (research/foliage-near-camera.md).
const SIGHT_FRONT_M: f32 = 0.3;
