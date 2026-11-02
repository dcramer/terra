// A world's look in its plants' colours (`Tweaks::foliage_hue`, the parting buffer's `look`):
// one turn for every plant shader (foliage, grass, the far forest), so a realm's leaves,
// blades and distant crowns turn alike and whoever walks through keeps his own colours.

// A colour turned round the hue circle by so many radians, about the grey axis, keeping its
// brightness and greys (Rodrigues' rotation, as a colour grade's hue turn is), none below 0.
fn hue_turned(c: vec3<f32>, turn: f32) -> vec3<f32> {
    if turn == 0.0 {
        return c;
    }
    let k = vec3<f32>(0.57735027);
    let s = sin(turn);
    let co = cos(turn);
    return max(c * co + cross(k, c) * s + k * dot(k, c) * (1.0 - co), vec3<f32>(0.0));
}
