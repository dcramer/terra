//! A burning building's light: hung out in front of the face that burns, and seen from a
//! film's distance (castle round 4).
use super::*;

/// A timber storey burning along a wall 12 m up, the wall's face at z 0 looking +z: four
/// blazes over 8 m, in one gathering.
fn wall() -> (Vec3, Blazes) {
    let ground = Vec3::new(0.0, 2000.0, 0.0);
    let blazes = (0..4)
        .map(|i| Blaze {
            at: ground + Vec3::new(0.5 + 2.0 * i as f32, 12.0, 0.3),
            tall: 2.5,
            heat: 0.9,
            seed: 0.2 * i as f32,
            wide: 0.0,
            out: Vec3::Z,
        })
        .collect();
    (
        ground,
        Blazes {
            blazes,
            ..default()
        },
    )
}

/// How square on a light meets a point of a face looking `normal`: the cosine of its incidence
/// over the square of its distance, per lumen.
fn on_face(lit: &Lit, point: Vec3, normal: Vec3) -> f32 {
    let to = lit.at - point;
    (to.normalize().dot(normal)).max(0.0) / to.length_squared()
}

#[test]
fn a_burning_walls_light_hangs_out_in_front_of_it_and_lights_the_stone_under_it() {
    let (ground, blazes) = wall();
    let lit = lit(&blazes, 1.0, crate::fire::BreathTier::Cards);
    assert_eq!(lit.len(), 1, "one gathering, one light");
    let light = &lit[0];
    assert!(
        light.at.z > 3.0,
        "the light hangs {:.1} m out of the face",
        light.at.z
    );
    // The stone 6 m under the fire, face-on to the light: lit at under 60 degrees of
    // incidence, where a light hung in or over the flames meets it edge-on.
    let stone = ground + Vec3::new(3.5, 6.0, 0.0);
    let to = (light.at - stone).normalize();
    assert!(
        to.z > 0.5,
        "the stone under the fire meets the light at {to}"
    );
    // And more of it than a light at the flames' own middle, 1.5 m over them, would give.
    let over = Lit {
        at: ground + Vec3::new(3.5, 13.5, 0.3),
        ..light.clone()
    };
    assert!(on_face(light, stone, Vec3::Z) > 5.0 * on_face(&over, stone, Vec3::Z));
}

#[test]
fn a_roofs_light_hangs_over_it_as_before() {
    // Flames on a roof face up: no side to hang their light out of, so it hangs over them.
    let (ground, mut blazes) = wall();
    for blaze in &mut blazes.blazes {
        blaze.out = Vec3::Y;
    }
    let lit = lit(&blazes, 1.0, crate::fire::BreathTier::Cards);
    let off = lit[0].at - (ground + Vec3::new(3.5, 12.0, 0.3));
    assert!(off.y > 1.0 && off.xz().length() < 1.0, "{off}");
}

#[test]
fn a_films_fire_lights_the_walls_from_three_times_plays_distance() {
    let (_, blazes) = wall();
    let lit = lit(&blazes, 1.0, crate::fire::BreathTier::Cards);
    let was = vec![
        Was {
            at: Vec3::ZERO,
            on: false,
            near: false,
        };
        4
    ];
    // The castle fall's cameras stand 35 to 50 m off.
    let eye = lit[0].at + Vec3::new(0.0, -8.0, 45.0);
    let play = choose_within(lit.clone(), Some(eye), &was, 1, light_reach(Quality::High));
    assert!(play.iter().all(Option::is_none), "play lights to 40 m");
    let film = choose_within(lit, Some(eye), &was, 4, light_reach(Quality::Ultra));
    let shown = film.iter().flatten().next().expect("a film lights it");
    assert!(shown.fade > 0.99, "eased to {}", shown.fade);
}
