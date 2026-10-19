//! What rays meet in a registered scene (`terra_build_scenes::registry`), for the flight
//! plot (tools/sandbox/flightplot.ts, D382) to say whether each breath strikes the building:
//! the scene named, and on standard input a JSON array of rays in its base's own metres,
//! `[{"from": [x, y, z], "to": [x, y, z], "reach": metres}]`; on standard output, JSON:
//! `met`, an array as long, each the first cube its ray meets within its reach going from
//! `from` towards `to` (`{"cube": [x, y, z], "distance": metres, "kind": name}`), or null;
//! and `columns`, the building's footprint, each column it stands on as `[x, z, top]` in
//! metres, its middle and the top of its highest cube, for a plot to draw it under a course.
//!
//!   cargo run -q -p terra_build_scenes --example rays -- "burg: built to burn" < rays.json

use glam::Vec3;
use serde_json::{Value, json};
use std::io::Read;
use std::path::Path;
use std::process::ExitCode;
use terra_build::CELL_M;
use terra_build_scenes::registry::{Materials, registry};

fn main() -> ExitCode {
    let Some(name) = std::env::args().nth(1) else {
        eprintln!("usage: rays <scene name> < rays.json");
        return ExitCode::FAILURE;
    };
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let materials = match Materials::read(&assets, false) {
        Ok(materials) => materials,
        Err(why) => {
            eprintln!("rays: the materials do not read: {why}");
            return ExitCode::FAILURE;
        }
    };
    let scenes = registry(&materials);
    let Some(scene) = scenes.iter().find(|scene| scene.name == name) else {
        eprintln!("rays: there is no scene called {name}");
        return ExitCode::FAILURE;
    };
    let base = scene.base();
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        eprintln!("rays: standard input does not read");
        return ExitCode::FAILURE;
    }
    let rays: Vec<Value> = match serde_json::from_str(&input) {
        Ok(rays) => rays,
        Err(why) => {
            eprintln!("rays: the rays are not a JSON array: {why}");
            return ExitCode::FAILURE;
        }
    };
    let point = |v: &Value| -> Option<Vec3> {
        let a = v.as_array()?;
        Some(Vec3::new(
            a.first()?.as_f64()? as f32,
            a.get(1)?.as_f64()? as f32,
            a.get(2)?.as_f64()? as f32,
        ))
    };
    let met: Vec<Value> = rays
        .iter()
        .map(|ray| {
            let (Some(from), Some(to)) = (point(&ray["from"]), point(&ray["to"])) else {
                return Value::Null;
            };
            let reach = ray["reach"].as_f64().unwrap_or(30.0) as f32;
            let way = (to - from).normalize_or_zero();
            base.raycast(from, way, reach).map_or(Value::Null, |hit| {
                json!({
                    "cube": hit.cube.to_array(),
                    "distance": hit.distance,
                    "kind": base.cube(hit.cube).map(|cube| materials.name(cube.kind)),
                })
            })
        })
        .collect();
    let mut tops = std::collections::BTreeMap::<(i32, i32), i32>::new();
    for (at, _) in base.cubes() {
        let top = tops.entry((at.x, at.z)).or_insert(at.y);
        *top = (*top).max(at.y);
    }
    let columns: Vec<Value> = tops
        .into_iter()
        .map(|((x, z), y)| {
            json!([
                (x as f32 + 0.5) * CELL_M,
                (z as f32 + 0.5) * CELL_M,
                (y + 1) as f32 * CELL_M
            ])
        })
        .collect();
    println!("{}", json!({ "met": met, "columns": columns }));
    ExitCode::SUCCESS
}
