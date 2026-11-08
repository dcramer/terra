use super::*;

fn lamp() -> Lamp {
    Lamp {
        lm: 1000.0,
        colour: Colour::Srgb(1.0, 1.0, 1.0),
        up: 0.5,
        ground: true,
        range: 3.0,
        fall: 0.1,
        toward: 0.0,
    }
}

/// A hit's light set out toward the camera lights the faces of the two bodies the camera
/// sees more than one at the bite, and the faces turned from it less: the irradiance on
/// each body's near and far side, knight and foe a couple of metres apart across the view
/// (the ruin's stage, its camera at 55 degrees down, 20 round).
#[test]
fn a_hit_light_set_toward_the_camera_lights_what_it_sees() {
    let (pitch, yaw) = (55f32.to_radians(), 20f32.to_radians());
    let cam = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
    let to_eye = cam * Vec3::Z;
    assert!(to_eye.y > 0.5, "the camera looks down");
    let near = Vec3::new(to_eye.x, 0.0, to_eye.z).normalize();
    let bite = Vec3::new(1.0, 1.5, -0.7);
    let bodies = [Vec3::new(0.0, 1.2, 0.0), Vec3::new(2.0, 1.3, -1.4)];
    // A face's irradiance from a point light (Lambert over r^2).
    let e = |light: Vec3, at: Vec3, n: Vec3| {
        let d = light - at;
        (d.normalize().dot(n)).max(0.0) / d.length_squared()
    };
    let set = lamp_at(bite + Vec3::Y * 0.5, 1.0, cam);
    assert!(
        (set.y - 2.0).abs() < 1e-5,
        "set out along the ground, not down the view"
    );
    for b in bodies {
        let (seen, hidden) = (near, -near);
        assert!(e(set, b + seen * 0.25, seen) > 2.0 * e(bite, b + seen * 0.25, seen));
        assert!(e(set, b - seen * 0.25, hidden) <= e(bite, b - seen * 0.25, hidden));
    }
}

/// A preset's light is dark before its cue, lights at once, and dies with the effect:
/// nothing past its life, however slow its own fade.
#[test]
fn a_presets_light_dies_with_it() {
    let slow = Lamp {
        fall: 100.0,
        ..lamp()
    };
    assert_eq!(lamp_lm(&slow, -0.01, 0.4), 0.0);
    assert!(lamp_lm(&slow, 0.0, 0.4) > 999.0);
    assert!(lamp_lm(&slow, 0.39, 0.4) < 0.1 * 1000.0);
    assert_eq!(lamp_lm(&slow, 0.4, 0.4), 0.0);
    assert_eq!(lamp_lm(&slow, 5.0, 0.4), 0.0);
    // And it only ever dims.
    let mut last = f32::MAX;
    for i in 0..40 {
        let l = lamp_lm(&lamp(), i as f32 * 0.01, 0.4);
        assert!(l <= last, "brighter at {i}0 ms");
        last = l;
    }
}

/// The bolts' trail is a streak (a hot line in a halo, effects.wesl's mode 3), the slash's
/// the edged smear (mode 2): a bolt drawn as the slash would be a crescent again.
#[test]
fn a_bolt_trails_a_streak_and_a_slash_an_edge() {
    let mode = |name: &str| {
        let p = Preset::load(name).unwrap();
        Glow::trail(&p.trail.unwrap(), 0.0).look.shape.z
    };
    assert_eq!(mode("bolt"), 3.0);
    assert_eq!(mode("slash"), 2.0);
}

/// Every preset parses, and every biome's effects load within the kit's budget.
#[test]
fn budgets_hold() {
    for name in names() {
        Preset::load(&name).unwrap_or_else(|e| panic!("{e}"));
    }
    for f in std::fs::read_dir(crate::kit::dir()).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "ron") {
            let b = Biome::load(p.file_stem().unwrap().to_str().unwrap()).unwrap();
            let fx = Effects::load(Some(&b)).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let n = fx.budget();
            assert!(
                n.quads <= QUADS && n.lights <= LIGHTS,
                "{}: {n:?}",
                p.display()
            );
        }
    }
    let fight = Effects::load(None).unwrap();
    for name in FIGHT {
        assert!(fight.get(name).is_some(), "the staged fight's {name}");
    }
}

/// A biome's `light` scales its preset's lamps and nothing else; left out, the preset's.
#[test]
fn a_biomes_light_scales_its_lamps() {
    let mut b = Biome::load("neon-street").unwrap();
    b.effects = vec![crate::kit::biome::Effect {
        preset: "hit-spark".into(),
        at: None,
        light: 0.25,
        scale: 1.0,
    }];
    let dim = Effects::load(Some(&b)).unwrap();
    let full = Preset::load("hit-spark").unwrap();
    let (d, f) = (&dim.0[0].1, &full);
    assert_eq!(d.lamps.len(), f.lamps.len());
    for (a, z) in d.lamps.iter().zip(&f.lamps) {
        assert!((a.lm - z.lm * 0.25).abs() < 1e-3, "{} vs {}", a.lm, z.lm);
        assert_eq!(a.range, z.range);
    }
    let s = |p: &Preset| p.sparks.as_ref().map(|s| s.count);
    assert_eq!(s(d), s(f), "the sparks unchanged");
    let e: crate::kit::biome::Effect = ron::from_str("(preset: \"slash\")").unwrap();
    assert_eq!(e.light, 1.0);
}

/// A biome's `scale` grows the effect in space and keeps its timing: each spark, thrown
/// as the preset throws it, is where the preset's would be times the scale at every age
/// (its path, not only its quad), the flash's quad and its lights' reach scale, and its life
/// and lumens are the preset's. Left out, the preset's own size.
#[test]
fn a_biomes_scale_grows_the_effect_and_keeps_its_timing() {
    let mut b = Biome::load("forest").unwrap();
    b.effects = vec![crate::kit::biome::Effect {
        preset: "hit-spark".into(),
        at: None,
        light: 1.0,
        scale: 2.5,
    }];
    let big = Effects::load(Some(&b)).unwrap().0[0].1.clone();
    let base = Preset::load("hit-spark").unwrap();
    let (k, k0) = (big.sparks.unwrap(), base.sparks.unwrap());
    for a in [0.05, 0.2, 0.4] {
        let way = Vec3::new(0.6, 0.8, 0.0);
        let (p, _) = spark_at(Vec3::ZERO, way * k.speed.1, k.gravity, a);
        let (p0, _) = spark_at(Vec3::ZERO, way * k0.speed.1, k0.gravity, a);
        assert!(
            (p - p0 * 2.5).length() < 1e-4,
            "at {a} s: {p} vs {p0} x 2.5"
        );
    }
    assert!((k.width - k0.width * 2.5).abs() < 1e-6);
    let (f, f0) = (big.flash.unwrap(), base.flash.unwrap());
    assert!((f.size - f0.size * 2.5).abs() < 1e-6 && f.core_size == f0.core_size);
    for (l, l0) in big.lamps.iter().zip(&base.lamps) {
        assert!((l.range - l0.range * 2.5).abs() < 1e-4 && l.lm == l0.lm);
    }
    assert_eq!(
        (big.life, k.life, k.shutter),
        (base.life, k0.life, k0.shutter)
    );
    let e: crate::kit::biome::Effect = ron::from_str("(preset: \"slash\")").unwrap();
    assert_eq!(e.scale, 1.0);
}

/// A parry as the forest plays it is a flash at the blade, not a disc about him: its quad
/// narrower than his shoulders (0.5 m), its streaks thrown in two jets to either side of
/// the blow (along a blade held across it), none straight back along it (j29's judge: "a
/// body-sized blue disc with radial spikes").
#[test]
fn a_parry_flashes_at_the_blade_and_throws_its_sparks_along_it() {
    let b = Biome::load("forest").unwrap();
    let fx = Effects::load(Some(&b)).unwrap();
    let p = &fx.0.iter().find(|e| e.0 == "parry-ring").unwrap().1;
    let f = p.flash.unwrap();
    assert!(f.size < 0.5, "the parry's flash is {} m across", f.size);
    let s = p.sparks.unwrap();
    let out = Vec3::Z;
    let (a, b) = (spark_out(&s, out, 0), spark_out(&s, out, 1));
    assert!(a.x * b.x < 0.0, "both jets go one way: {a}, {b}");
    for w in [a, b] {
        // Out more across the blow than along it, the cone not closing the gap.
        assert!(
            w.x.abs() > w.z.abs() && s.spread < w.x.abs(),
            "{w}, cone {}",
            s.spread
        );
    }
}

/// A name with no file fails naming the folder and the presets there are.
#[test]
fn an_unknown_preset_names_the_fix() {
    let e = Preset::load("no-such-effect").unwrap_err();
    assert!(
        e.contains("crates/terra_iso/biomes/effects/") && e.contains("hit-spark"),
        "{e}"
    );
}

/// A streak flies out along its way and falls under gravity; embers (gravity under
/// zero) rise.
#[test]
fn sparks_fly_and_fall() {
    let (p, v) = spark_at(Vec3::ZERO, Vec3::X * 4.0, 9.8, 0.5);
    assert!((p.x - 2.0).abs() < 1e-5 && p.y < -1.2 && v.y < -4.8);
    let (p, _) = spark_at(Vec3::ZERO, Vec3::ZERO, -1.0, 1.0);
    assert!(p.y > 0.49);
}

/// A spark cools as hot metal does: only ever dimmer, white-yellow at birth, orange (its
/// green under half its red) by mid-life, and dark at death, not a white streak cut off.
#[test]
fn sparks_cool_to_dark() {
    let s = Preset::load("hit-spark").unwrap().sparks.unwrap();
    let born = spark_glow(&s, 0.0);
    assert!(born.y > 0.6 * born.x, "born {born}");
    let mid = spark_glow(&s, 0.5);
    assert!(mid.y < 0.5 * mid.x, "mid-life {mid}");
    assert!(spark_glow(&s, 1.0).max_element() < 1e-3);
    let mut last = f32::MAX;
    for i in 0..=20 {
        let l = spark_glow(&s, i as f32 / 20.0).element_sum();
        assert!(l <= last, "brighter at {i}/20");
        last = l;
    }
}

/// A puff under a light is lit by it as a lux meter would read (lm / 4π d²), and a light
/// out of reach or dark leaves nothing.
#[test]
fn smoke_is_lit_by_the_lights_at_it() {
    let lm = 4.0 * std::f32::consts::PI * 100.0;
    let e = lit_at(Vec3::ZERO, 0.0, &[(Vec3::Y, Vec3::ONE, lm, 5.0)], None);
    assert!((e.x - 100.0).abs() < 0.01, "{e}");
    assert_eq!(
        lit_at(
            Vec3::ZERO,
            0.0,
            &[(Vec3::Y * 6.0, Vec3::ONE, lm, 5.0)],
            None
        ),
        Vec3::ZERO
    );
    assert_eq!(
        lit_at(Vec3::ZERO, 0.0, &[(Vec3::Y, Vec3::ONE, 0.0, 5.0)], None),
        Vec3::ZERO
    );
    // A light inside a puff a metre in radius lights it as one a metre off, not as one
    // at its heart: what is near the light is not burned out.
    let inside = lit_at(Vec3::ZERO, 1.0, &[(Vec3::ZERO, Vec3::ONE, lm, 5.0)], None);
    assert!((inside.x - 100.0).abs() < 0.01, "{inside}");
}

/// A still shows each effect at its chosen moment; a film plays it from its cue, and a
/// loop round its period.
#[test]
fn a_still_holds_the_chosen_moment() {
    let p = Preset::load("hit-spark").unwrap();
    let cues = Cues {
        blow: 2.0,
        bolt: 0.0,
        contact: None,
        ..Cues::default()
    };
    assert_eq!(
        age(
            &p,
            &Clock {
                t: 9.0,
                still: true
            },
            &cues
        ),
        p.still
    );
    assert!(
        (age(
            &p,
            &Clock {
                t: 2.25,
                still: false
            },
            &cues
        ) - 0.25)
            .abs()
            < 1e-5
    );
    let s = Preset {
        cue: Cue::Loop(2.0),
        ..p
    };
    assert!(
        (age(
            &s,
            &Clock {
                t: 5.5,
                still: false
            },
            &cues
        ) - 1.5)
            .abs()
            < 1e-5
    );
}

/// A blow landed in play takes the blow's sparks to its bite and its dust to the floor
/// under it, turned the blow's way; a loop, a bolt and a placed effect stay put, and
/// with no contact nothing moves.
#[test]
fn a_blow_in_play_moves_its_effects_to_the_bite() {
    let stage = Stage::default();
    let toward = (stage.foe - stage.knight).normalize();
    let bite = stage.strike();
    let mut cues = Cues::default();
    let spark = Preset::load("hit-spark").unwrap();
    assert!(shift(&spark, None, bite, toward, &cues).is_none());
    let point = Vec3::new(5.0, 2.2, 7.0);
    cues.contact = Some(Contact {
        point,
        floor: 1.0,
        toward: Vec2::new(-toward.y, toward.x),
    });
    let s = shift(&spark, None, bite, toward, &cues).unwrap();
    assert!(s.point(bite).distance(point) < 1e-5);
    // A spark thrown along the staged blow now flies a quarter turn off it, as the blow.
    let ahead = bite + toward.extend(0.0).xzy();
    let flown = s.point(ahead) - point;
    assert!(flown.distance(Vec2::new(-toward.y, toward.x).extend(0.0).xzy()) < 1e-5);
    let dust = Preset::load("smoke-puff").unwrap();
    let under = stage.ground(None, bite.xz());
    let d = shift(&dust, None, under, toward, &cues).unwrap();
    assert!(d.point(under).distance(Vec3::new(5.0, 1.0, 7.0)) < 1e-5);
    for name in ["steam", "bolt", "spell-burst"] {
        let p = Preset::load(name).unwrap();
        assert!(shift(&p, None, bite, toward, &cues).is_none(), "{name}");
    }
    assert!(shift(&spark, Some(Vec2::ONE), bite, toward, &cues).is_none());
}
