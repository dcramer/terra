//! A room shut off from the sky, seen from inside it (D485, D486, D488): no sky outshines its
//! fires, so they burn and the eye is set as by night, whatever the hour; and a little light
//! fills it everywhere, as the fires' light bounced round its stone does, so the stretch
//! between two torches is dim but never black.
//!
//! How shut off the air round the eye is follows from how much of the sky reaches it
//! (`scene::IndoorSky`, 1 outdoors): a house's room, its door open on the air, keeps the day's
//! light and exposure; a dungeon's corridor four levels down none.
//!
//! Shipped games do it so: Bethesda's interior cells have no sky and an ambient colour of
//! their own; Unreal's interiors take a low fill (a dim sky light or ambient cubemap) and the
//! eye adaptation's range, not the sky's exposure, sets how they read.

use crate::planet::MainCamera;
use crate::scene::IndoorSky;
use bevy::prelude::*;
use terra_gen::smoothstep;

/// The share of the sky's light in the air round the eye under which a room counts as shut
/// off, wholly under the first and not at all over the second: a house's room takes some 0.6
/// (D486's `INDOORS`), a dungeon's stair foot a tenth or two, its deep rooms none.
const SHUT: [f32; 2] = [0.05, 0.35];
/// The eye's key in a room shut off from the sky, EV at ISO 100, whatever the hour. At noon's
/// 11.8 with the fires at their day light a dungeon read only the torches' orange and black
/// between (round 8); at the deep night's 9 with them at their night light its rooms were lit
/// orange boxes (the keep's goal mean sRGB 0.24, round 9); at 10.8, 0.18.
const INDOOR_EV: f32 = 10.8;
/// The light filling a room shut off from the sky everywhere, cd/m²: what of its fires' light
/// the probes do not carry, bounced round its stone. Without it a corridor between torches
/// was black (the mine film's 22.5 s, the crypt's 2.5 s, round 8's review); at 16 and warm
/// grey every surface took one even orange-buff cast with no dark between the torches (round
/// 11, all three reviewers). At 8 and cool it was not seen at all (round 13's reviewers: the
/// shade read ochre): 7 stops under a mid-grey on stone, its blue at the 8-bit floor, so the
/// shadowed fifth of the probe stills read redder than the torch-lit tenth (linear R/B 33
/// against 9). At 140 (round 15) the shade reads cooler than the torch-lit (R/B 4.5 against 7).
const FILL: f32 = 140.0;
/// Its colour: a cool, moonless grey, as shipped dungeons light their darks (Diablo IV's and
/// Elden Ring's catacombs: warm pools at the fires, cool shadow between), so the eye reads a
/// torch's warmth against them rather than one tint over everything. Green enough that the
/// torches' red spill over it reads grey, not violet: (0.60, 0.72, 1.0) left the shade at a
/// linear G/B of 0.3-0.4 (round 15's sweep). Near grey: at (0.62, 0.78, 1.0), linear blue
/// three times its red, whatever the torches missed went sky blue, a mine's ceiling slab and
/// the lintels a blue band over the frame, a grey rock a blue-white lump (round 17's
/// reviewers: "a ceiling slab that reads as sky"); a neutral fill took the blue off them
/// (round 18's stills, the sky's own light at nought changing nothing).
const FILL_COLOUR: Color = Color::srgb(0.78, 0.84, 0.95);

pub fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, fill);
}

/// How shut off from the sky the air round the eye is, from 0 (outdoors, or a room open
/// enough to the sky) to 1, for so much of the sky's light reaching it.
pub fn shut(air_lit: f32) -> f32 {
    1.0 - smoothstep(SHUT[0], SHUT[1], air_lit)
}

/// The eye's key for the hour's `key` with so much of the sky reaching the air round it.
pub fn key(key: f32, air_lit: f32) -> f32 {
    key + (INDOOR_EV - key) * shut(air_lit)
}

/// How far into the night the fires burn, for the sky's `night`: by night in a room shut off
/// from the sky, where nothing outshines them.
pub fn fires_night(night: f32, air_lit: f32) -> f32 {
    night.max(shut(air_lit))
}

/// How bright the room's fill is round an eye with so much of the sky reaching it, cd/m².
pub fn fill_brightness(air_lit: f32) -> f32 {
    FILL * shut(air_lit)
}

/// Gives the camera the room's fill while it is shut off from the sky, and takes it away
/// outside, where the world's own ambient (none, `skylight`) is left as it was.
fn fill(
    mut commands: Commands,
    air: Option<Res<IndoorSky>>,
    camera: Query<(Entity, Option<&AmbientLight>), With<MainCamera>>,
) {
    let brightness = fill_brightness(air.map_or(1.0, |air| air.0));
    for (entity, had) in &camera {
        match had {
            _ if brightness <= 0.0 => {
                if had.is_some() {
                    commands.entity(entity).remove::<AmbientLight>();
                }
            }
            Some(light) if (light.brightness - brightness).abs() < 1e-3 => {}
            _ => {
                commands.entity(entity).insert(AmbientLight {
                    color: FILL_COLOUR,
                    brightness,
                    affects_lightmapped_meshes: true,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_room_shut_off_from_the_sky_is_seen_the_same_by_day_as_by_night() {
        // Noon's key and midnight's (daylight::keys) give one key in a dungeon's deep room.
        assert_eq!(key(11.8, 0.0), key(9.0, 0.0));
        // And its fires burn as by night at noon.
        assert_eq!(fires_night(0.0, 0.0), 1.0);
        // Outdoors, and in a house's room (D486: 0.6 of the sky), nothing changes.
        for air_lit in [1.0, 0.6] {
            assert_eq!(key(11.8, air_lit), 11.8);
            assert_eq!(fires_night(0.3, air_lit), 0.3);
            assert_eq!(fill_brightness(air_lit), 0.0);
        }
    }

    #[test]
    fn a_shut_room_is_filled_dim_and_cool() {
        // A stone wall (albedo 0.3) under the fill alone, at the room's key, on screen
        // (1 / (1.2 x 2^EV) per cd/m², Bevy's exposure).
        let on_screen = fill_brightness(0.0) * 0.3 / (1.2 * libm::exp2f(INDOOR_EV));
        // Dim: well under a mid-grey, three stops at least, so the torches' pools stand out.
        assert!(on_screen < 0.18 / 8.0, "{on_screen}");
        // Seen: within six stops of it, over 8-bit black after the tonemapper (at 8 cd/m² it
        // stood 7.3 stops under and the shade took none of its colour).
        assert!(on_screen > 0.18 / 64.0, "{on_screen}");
        // Cool against the fires: on the dungeons' warm stone (ochre, linear 0.35, 0.27,
        // 0.18) it gives back a red over blue a tenth of the torches' at least.
        let c = FILL_COLOUR.to_linear();
        let t = crate::fire::TORCH_LIGHT.to_linear();
        let back = (c.red * 0.35) / (c.blue * 0.18);
        let torch = (t.red * 0.35) / (t.blue * 0.18);
        assert!(back * 10.0 < torch, "{back} against the torches' {torch}");
        // A grey's blue, not violet: green over half its blue, so red spill over it is grey.
        assert!(
            c.blue > c.green && c.green > c.red && c.green > 0.5 * c.blue,
            "{c:?}"
        );
        // Never sky: what only it lights, a plain grey ceiling or rock, is a cool grey, its
        // colour's spread under four tenths of its brightest (0.66 read as sky, round 17).
        let spread = (c.blue - c.red) / c.blue;
        assert!(spread < 0.4, "{spread}");
    }
}
