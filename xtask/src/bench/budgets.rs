//! The performance budgets (D607): `assets/data/perf/budgets.ron`, what each scene may spend
//! by phase (the load, the settle after the loading screen lifts, and play), the one place
//! they are set; the file's head says where each comes from. A scene's budgets are the
//! file's `every` phases with its own entry laid over them.

use crate::ron::{self, Value};

/// Where the budgets are, from the repository's root.
pub(super) const FILE: &str = "assets/data/perf/budgets.ron";

/// What one phase of a scene is held to; None where it is held to nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Limits {
    /// How long the phase lasts (the settle's, after the loading screen lifts).
    pub seconds: Option<f32>,
    pub frame_p99_ms: Option<f32>,
    pub over_pct: Option<f32>,
    pub hitches: Option<u32>,
    pub freezes: Option<u32>,
    pub drift_pct: Option<f32>,
    pub tick_p99_ms: Option<f32>,
    pub gpu_growth_pct: Option<f32>,
    pub integrated_median_ms: Option<f32>,
    /// The render world's CPU, median ms a frame.
    pub render_cpu_ms: Option<f32>,
    /// Each group's median ms a frame (`terra_gen::stat`), each schedule's (the recorder's
    /// marks: `PostUpdate`, `render.Render`), and each GPU pass's.
    pub groups: Vec<(String, f32)>,
    pub schedules: Vec<(String, f32)>,
    pub gpu: Vec<(String, f32)>,
}

/// A scene's budgets.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Scene {
    /// The most its load may take, if it is held to one.
    pub load_s: Option<f32>,
    /// The time to playable every load aims at (D580), and whether it is still provisional.
    pub load_target_s: Option<f32>,
    pub load_provisional: bool,
    pub settle: Limits,
    pub play: Limits,
}

/// The budgets of the scene named, from the file.
pub(super) fn of(scene: &str) -> Result<Scene, String> {
    let path = crate::root().join(FILE);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    parse(&text, scene)
}

/// The budgets of the scene named, from the file's text: an error names the line or the field
/// and says what is wanted (D556).
pub(super) fn parse(text: &str, scene: &str) -> Result<Scene, String> {
    let file = ron::parse(text).map_err(|e| format!("{FILE}, {e}"))?;
    // `streaming` is the game's, what enters the world a frame after the load (D608).
    known(&file, "the file", &["load", "streaming", "every", "scenes"])?;
    let mut budgets = Scene::default();
    if let Some(load) = file.get("load") {
        known(load, "load", &["target_s", "provisional"])?;
        budgets.load_target_s = load
            .get("target_s")
            .and_then(Value::number)
            .map(|n| n as f32);
        budgets.load_provisional = load.get("provisional") == Some(&Value::Bool(true));
    }
    let every = file.get("every");
    if let Some(every) = every {
        known(every, "every", &["settle", "play"])?;
    }
    for (name, entry) in file.get("scenes").map_or(&[][..], Value::fields) {
        if !crate::scenes::SCENES.iter().any(|s| s.name == name) {
            return Err(format!(
                "{FILE}: `{name}` is budgeted but is no scene of xtask/src/scenes.rs; rename it to a scene's name or remove it"
            ));
        }
        known(entry, name, &["load_s", "settle", "play"])?;
    }
    let own = file.get("scenes").and_then(|scenes| scenes.get(scene));
    budgets.load_s = own
        .and_then(|own| own.get("load_s"))
        .and_then(Value::number)
        .map(|n| n as f32);
    for (phase, limits) in [("settle", &mut budgets.settle), ("play", &mut budgets.play)] {
        for layer in [every, own] {
            if let Some(value) = layer.and_then(|layer| layer.get(phase)) {
                limits.lay(value, &format!("{scene}'s {phase}"))?;
            }
        }
    }
    Ok(budgets)
}

/// An error for a field the file has that nothing reads: a misspelt budget would hold nothing.
fn known(value: &Value, of: &str, names: &[&str]) -> Result<(), String> {
    match value
        .fields()
        .iter()
        .find(|(k, _)| !names.contains(&k.as_str()))
    {
        Some((key, _)) => Err(format!(
            "{FILE}: `{key}` in {of} is no budget this bench reads (it reads {}); a misspelt budget holds nothing",
            names.join(", ")
        )),
        None => Ok(()),
    }
}

impl Limits {
    /// Lays a phase's entry over these: what it names replaces what was, its groups and passes
    /// joining those already budgeted.
    fn lay(&mut self, value: &Value, of: &str) -> Result<(), String> {
        let names = [
            "seconds",
            "frame_p99_ms",
            "over_pct",
            "hitches",
            "freezes",
            "drift_pct",
            "tick_p99_ms",
            "gpu_growth_pct",
            "integrated_median_ms",
            "render_cpu_ms",
            "groups",
            "schedules",
            "gpu",
        ];
        known(value, of, &names)?;
        for (key, field) in value.fields() {
            let number = field.number().map(|n| n as f32);
            let wanted = || format!("{FILE}: {of}'s `{key}` wants a number of ms or a count");
            match key.as_str() {
                "groups" | "schedules" | "gpu" => {
                    let into = match key.as_str() {
                        "groups" => &mut self.groups,
                        "schedules" => &mut self.schedules,
                        _ => &mut self.gpu,
                    };
                    for (name, ms) in field.fields() {
                        let ms = ms.number().ok_or_else(wanted)? as f32;
                        into.retain(|(n, _)| n != name);
                        into.push((name.clone(), ms));
                    }
                }
                "hitches" | "freezes" => {
                    let count = Some(number.ok_or_else(wanted)? as u32);
                    *(if key == "hitches" {
                        &mut self.hitches
                    } else {
                        &mut self.freezes
                    }) = count;
                }
                _ => {
                    let slot = match key.as_str() {
                        "seconds" => &mut self.seconds,
                        "frame_p99_ms" => &mut self.frame_p99_ms,
                        "over_pct" => &mut self.over_pct,
                        "drift_pct" => &mut self.drift_pct,
                        "tick_p99_ms" => &mut self.tick_p99_ms,
                        "gpu_growth_pct" => &mut self.gpu_growth_pct,
                        "integrated_median_ms" => &mut self.integrated_median_ms,
                        _ => &mut self.render_cpu_ms,
                    };
                    *slot = Some(number.ok_or_else(wanted)?);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_parses_and_every_scene_has_the_frame_and_tick_budgets() {
        let text = std::fs::read_to_string(crate::root().join(FILE)).unwrap();
        for scene in crate::scenes::SCENES {
            let budgets = parse(&text, scene.name).unwrap_or_else(|e| panic!("{e}"));
            assert!(budgets.play.frame_p99_ms.is_some(), "{}", scene.name);
            assert!(budgets.play.tick_p99_ms.is_some(), "{}", scene.name);
            assert!(budgets.settle.seconds.is_some(), "{}", scene.name);
        }
        // D580 is Open: its load target is said as provisional.
        let lake = parse(&text, "lake-play").unwrap();
        assert!(lake.load_provisional && lake.load_target_s.is_some());
    }

    #[test]
    fn a_scene_lays_its_own_over_every_scenes() {
        let text = r#"(
            every: (play: (frame_p99_ms: 13.3, groups: {"creatures": 1.0, "flora": 2.0})),
            scenes: {"castle": (load_s: 40, play: (frame_p99_ms: 20, groups: {"flora": 3.5},
                schedules: {"render.Render": 4.0}))},
        )"#;
        let castle = parse(text, "castle").unwrap();
        assert_eq!(castle.load_s, Some(40.0));
        assert_eq!(castle.play.frame_p99_ms, Some(20.0));
        assert_eq!(
            castle.play.groups,
            vec![("creatures".to_string(), 1.0), ("flora".to_string(), 3.5)]
        );
        assert_eq!(
            castle.play.schedules,
            vec![("render.Render".to_string(), 4.0)]
        );
        let forest = parse(text, "forest-run").unwrap();
        assert_eq!(forest.play.frame_p99_ms, Some(13.3));
        assert_eq!(forest.load_s, None);
    }

    #[test]
    fn a_misspelt_budget_or_a_scene_that_is_none_is_an_error() {
        let misspelt = parse("(every: (play: (frame_p99: 13.3)))", "castle").unwrap_err();
        assert!(
            misspelt.contains("`frame_p99` in castle's play is no budget"),
            "{misspelt}"
        );
        let none = parse(r#"(scenes: {"nowhere": (load_s: 1)})"#, "castle").unwrap_err();
        assert!(
            none.contains("`nowhere` is budgeted but is no scene"),
            "{none}"
        );
        let word = parse(r#"(every: (play: (hitches: "none")))"#, "castle").unwrap_err();
        assert!(word.contains("`hitches` wants a number"), "{word}");
    }
}
