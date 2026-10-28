// How a hearth's light through a window flickers (crates/terra_folk/src/town/windows.rs), about
// its own brightness, at a time in seconds and a look's phase: a few slow waves and a quicker
// one, so it wavers rather than pulses. Worked out here from the clock rather than written into
// the windows' materials each frame, which re-specialised every pane, pool and halo of the town
// every frame (settlement round 3, D127). Its mean is `FLICKER_MEAN` in windows.rs.

fn flicker(t: f32, phase: f32) -> f32 {
    return 0.88 + 0.06 * sin(t * 5.3 + phase) + 0.04 * sin(t * 11.7 + phase * 2.3) + 0.02 * sin(t * 23.1 + phase * 4.1);
}
