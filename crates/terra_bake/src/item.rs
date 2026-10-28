//! An assembled item as a model (D565, D566): the assembler's placements written as a glTF binary
//! the held-look checks read as they read a held look (attachment.md's contract, D546): the grip
//! at the origin, +Y up the handle, its working end where the kind says, its materials named by
//! role, the look's extras on its first mesh node. The code-made handle is turned from its
//! section (a ring a join, linear between); each modelled part keeps its own nodes, meshes,
//! materials and pictures under one node placed where the assembler put it, never merged or
//! scaled. The same assembly and models give the same bytes.

use serde_json::{Map, Value, json};
use terra_items::assembly::{Assembly, Library};

use crate::glb::{self, Glb};

/// Sides of the turned handle: off a true circle by r (1 - cos(pi/16)), 0.3 mm on a 3.5 cm haft.
const SIDES: usize = 16;
/// The handle's wood, plain: a look's materials are named by role (part.ts's roles stage).
const WOOD: [f32; 4] = [0.42, 0.3, 0.19, 1.0];

/// The item's model; `model` gives the bytes of a part's `model` path (under the asset root).
pub fn write(
    lib: &Library,
    item: &Assembly,
    model: impl Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<Vec<u8>, String> {
    let mut out = Glb {
        json: json!({
            "asset": {"version": "2.0", "generator": "terra_bake item (terra_items::assembly)"},
            "scene": 0, "scenes": [{"nodes": []}], "nodes": [], "meshes": [], "materials": [],
            "accessors": [], "bufferViews": [], "buffers": [{"byteLength": 0}],
        }),
        bin: Vec::new(),
    };
    let handle = item
        .placed
        .iter()
        .find(|p| p.slot == "handle")
        .ok_or("an assembly without its handle")?;
    let mut extras = Map::new();
    extras.insert(
        "recipe".into(),
        serde_json::to_value(&item.recipe).map_err(|e| e.to_string())?,
    );
    if let Some(s) = item.second {
        extras.insert("second".into(), json!(s[1]));
    }
    let skin = lib.parts.get(&handle.part).and_then(|p| p.skin.as_deref());
    let wood = match skin {
        Some(path) => skin_material(
            &mut out,
            &glb::read(&model(path)?).map_err(|e| format!("{path}: {e}"))?,
        )
        .map_err(|e| format!("skin of {} ({path}): {e}", handle.part))?,
        None => push(
            &mut out.json["materials"],
            json!({"name": "Wood", "pbrMetallicRoughness": {"baseColorFactor": WOOD, "metallicFactor": 0.0, "roughnessFactor": 0.85}}),
        ),
    };
    let root = turned_handle(
        &mut out,
        &handle.part,
        &item.section,
        Value::Object(extras),
        wood,
    );
    let mut roots = vec![root];
    for placed in &item.placed {
        let part = &lib.parts[&placed.part];
        let Some(path) = &part.model else { continue };
        let children = merge(
            &mut out,
            &glb::read(&model(path)?).map_err(|e| format!("{path}: {e}"))?,
        )
        .map_err(|e| format!("part {} ({path}): {e}", part.part))?;
        roots.push(push(
            &mut out.json["nodes"],
            json!({"name": part.part, "translation": placed.at, "rotation": placed.turn, "children": children}),
        ));
    }
    out.json["scenes"][0]["nodes"] = json!(roots);
    for empty in [
        "images",
        "textures",
        "samplers",
        "extensionsUsed",
        "extensionsRequired",
    ] {
        if out.json[empty].as_array().is_some_and(Vec::is_empty) {
            out.json.as_object_mut().map(|o| o.remove(empty));
        }
    }
    out.json["buffers"][0]["byteLength"] = json!(out.bin.len());
    Ok(glb::write(&out))
}

/// A skin's first material, with its pictures, appended to the item: the skin merged whole, then
/// its meshes and nodes dropped (nothing points at them), so only its look is kept.
fn skin_material(out: &mut Glb, skin: &Glb) -> Result<usize, String> {
    let (materials, meshes, nodes) = (
        count_of(out, "materials"),
        count_of(out, "meshes"),
        count_of(out, "nodes"),
    );
    if count_of(skin, "materials") == 0 {
        return Err("it has no material; make it with tools/items/haft.py skin".into());
    }
    merge(out, skin)?;
    for (key, keep) in [("meshes", meshes), ("nodes", nodes)] {
        if let Some(a) = out.json[key].as_array_mut() {
            a.truncate(keep);
        }
    }
    Ok(materials)
}

fn count_of(g: &Glb, key: &str) -> usize {
    g.json[key].as_array().map_or(0, Vec::len)
}

/// Appends `v` to the array `at` (made one if missing) and gives its index.
fn push(at: &mut Value, v: Value) -> usize {
    if !at.is_array() {
        *at = json!([]);
    }
    let a = at.as_array_mut().expect("an array");
    a.push(v);
    a.len() - 1
}

/// Appends bytes as a buffer view, 16-byte aligned (enough for any accessor), and gives its index.
fn view(out: &mut Glb, bytes: &[u8], target: Option<u32>) -> usize {
    while !out.bin.len().is_multiple_of(16) {
        out.bin.push(0);
    }
    let mut v = json!({"buffer": 0, "byteOffset": out.bin.len(), "byteLength": bytes.len()});
    if let Some(t) = target {
        v["target"] = json!(t);
    }
    out.bin.extend_from_slice(bytes);
    push(&mut out.json["bufferViews"], v)
}

fn floats(out: &mut Glb, data: &[[f32; 3]]) -> usize {
    let bytes: Vec<u8> = data
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let v = view(out, &bytes, Some(34962));
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in data {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    push(
        &mut out.json["accessors"],
        json!({"bufferView": v, "componentType": 5126, "count": data.len(), "type": "VEC3", "min": lo, "max": hi}),
    )
}

fn uvs(out: &mut Glb, data: &[[f32; 2]]) -> usize {
    let bytes: Vec<u8> = data
        .iter()
        .flatten()
        .flat_map(|f| f.to_le_bytes())
        .collect();
    let v = view(out, &bytes, Some(34962));
    push(
        &mut out.json["accessors"],
        json!({"bufferView": v, "componentType": 5126, "count": data.len(), "type": "VEC2"}),
    )
}

/// The code-made handle turned from its section, `[y, diameter]` up the line: each span between
/// two rings its own quads with the cone's normal, a cap at either end. Its node is the item's
/// first, holding the look's extras.
fn turned_handle(
    out: &mut Glb,
    name: &str,
    section: &[[f32; 2]],
    extras: Value,
    m: usize,
) -> usize {
    let ring = |i: usize| {
        let a = 2.0 * std::f32::consts::PI * (i % SIDES) as f32 / SIDES as f32;
        (libm::cosf(a), libm::sinf(a))
    };
    let (mut pos, mut nrm, mut idx) = (Vec::new(), Vec::new(), Vec::<u32>::new());
    // Its UVs as tools/items/haft.py bakes a skin on: u round from +X toward +Z, v 1 at the butt
    // and 0 at the top, so a skin stretches over the handle by its length.
    let (butt, top) = (
        section.first().map_or(0.0, |s| s[0]),
        section.last().map_or(1.0, |s| s[0]),
    );
    let v = |y: f32| 1.0 - (y - butt) / (top - butt).max(1e-6);
    let mut uv = Vec::<[f32; 2]>::new();
    for w in section.windows(2) {
        let ([y0, d0], [y1, d1]) = (w[0], w[1]);
        if y1 <= y0 {
            continue;
        }
        let slope = -(d1 - d0) / 2.0 / (y1 - y0);
        let base = pos.len() as u32;
        for i in 0..=SIDES {
            let (c, s) = ring(i);
            let n = [c, slope, s];
            let l = (c * c + slope * slope + s * s).sqrt();
            for (y, d) in [(y0, d0), (y1, d1)] {
                pos.push([c * d / 2.0, y, s * d / 2.0]);
                uv.push([i as f32 / SIDES as f32, v(y)]);
                nrm.push(n.map(|x| x / l));
            }
        }
        for i in 0..SIDES as u32 {
            let (a, d, b, c) = (
                base + 2 * i,
                base + 2 * i + 1,
                base + 2 * i + 2,
                base + 2 * i + 3,
            );
            idx.extend([a, d, c, a, c, b]);
        }
    }
    let ends = match (section.first(), section.last()) {
        (Some(&a), Some(&b)) => vec![(a, -1.0f32), (b, 1.0)],
        _ => Vec::new(),
    };
    for ([y, d], up) in ends {
        let centre = pos.len() as u32;
        pos.push([0.0, y, 0.0]);
        uv.push([0.5, v(y)]);
        nrm.push([0.0, up, 0.0]);
        for i in 0..SIDES {
            let (c, s) = ring(i);
            pos.push([c * d / 2.0, y, s * d / 2.0]);
            uv.push([0.5, v(y)]);
            nrm.push([0.0, up, 0.0]);
        }
        for i in 0..SIDES as u32 {
            let (p, q) = (centre + 1 + i, centre + 1 + (i + 1) % SIDES as u32);
            idx.extend(if up < 0.0 {
                [centre, p, q]
            } else {
                [centre, q, p]
            });
        }
    }
    let (p, n) = (floats(out, &pos), floats(out, &nrm));
    let t = uvs(out, &uv);
    let bytes: Vec<u8> = idx.iter().flat_map(|i| i.to_le_bytes()).collect();
    let iv = view(out, &bytes, Some(34963));
    let i = push(
        &mut out.json["accessors"],
        json!({"bufferView": iv, "componentType": 5125, "count": idx.len(), "type": "SCALAR"}),
    );
    let mesh = push(
        &mut out.json["meshes"],
        json!({"name": name, "primitives": [{"attributes": {"POSITION": p, "NORMAL": n, "TEXCOORD_0": t}, "indices": i, "material": m}]}),
    );
    push(
        &mut out.json["nodes"],
        json!({"name": name, "mesh": mesh, "extras": extras}),
    )
}

/// A part's model appended to the item: its buffer views, accessors, pictures, materials, meshes
/// and nodes, each index moved by what was there before. Gives its scene's root nodes.
fn merge(out: &mut Glb, part: &Glb) -> Result<Vec<usize>, String> {
    let count = |key: &str| out.json[key].as_array().map_or(0, Vec::len) as u64;
    let [
        views,
        accessors,
        images,
        samplers,
        textures,
        materials,
        meshes,
        nodes,
    ] = [
        "bufferViews",
        "accessors",
        "images",
        "samplers",
        "textures",
        "materials",
        "meshes",
        "nodes",
    ]
    .map(count);
    let list = |key: &str| part.json[key].as_array().cloned().unwrap_or_default();
    for mut v in list("bufferViews") {
        let (o, l) = (
            v["byteOffset"].as_u64().unwrap_or(0) as usize,
            v["byteLength"].as_u64().unwrap_or(0) as usize,
        );
        let bytes = part
            .bin
            .get(o..o + l)
            .ok_or("a buffer view runs past its buffer")?
            .to_vec();
        let n = view(out, &bytes, None);
        v["byteOffset"] = out.json["bufferViews"][n]["byteOffset"].clone();
        v["buffer"] = json!(0);
        out.json["bufferViews"][n] = v;
    }
    for mut a in list("accessors") {
        shift(&mut a, "bufferView", views);
        for s in ["indices", "values"] {
            if let Some(sparse) = a.get_mut("sparse").and_then(|x| x.get_mut(s)) {
                shift(sparse, "bufferView", views);
            }
        }
        push(&mut out.json["accessors"], a);
    }
    for mut img in list("images") {
        if let Some(uri) = img.get("uri").and_then(Value::as_str) {
            return Err(format!(
                "its picture {uri} is outside the file; export it with its pictures in the glb"
            ));
        }
        shift(&mut img, "bufferView", views);
        push(&mut out.json["images"], img);
    }
    for s in list("samplers") {
        push(&mut out.json["samplers"], s);
    }
    for mut t in list("textures") {
        shift(&mut t, "sampler", samplers);
        shift(&mut t, "source", images);
        push(&mut out.json["textures"], t);
    }
    for mut m in list("materials") {
        textures_moved(&mut m, textures);
        push(&mut out.json["materials"], m);
    }
    for mut mesh in list("meshes") {
        for p in mesh
            .get_mut("primitives")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(set) = p.get_mut("attributes") {
                shift_every(set, accessors);
            }
            for set in p
                .get_mut("targets")
                .and_then(Value::as_array_mut)
                .into_iter()
                .flatten()
            {
                shift_every(set, accessors);
            }
            shift(p, "indices", accessors);
            shift(p, "material", materials);
        }
        push(&mut out.json["meshes"], mesh);
    }
    for mut n in list("nodes") {
        if n.get("skin").is_some() || n.get("matrix").is_some() {
            return Err(
                "a node has a skin or a matrix: a part is rigid and placed by TRS (D303, D566)"
                    .into(),
            );
        }
        shift(&mut n, "mesh", meshes);
        if let Some(children) = n.get_mut("children") {
            shift_every(children, nodes);
        }
        n.as_object_mut().map(|o| o.remove("camera"));
        push(&mut out.json["nodes"], n);
    }
    for key in ["extensionsUsed", "extensionsRequired"] {
        for e in list(key) {
            if !out.json[key].as_array().is_some_and(|a| a.contains(&e)) {
                push(&mut out.json[key], e);
            }
        }
    }
    let scene = part.json["scene"].as_u64().unwrap_or(0) as usize;
    Ok(part.json["scenes"][scene]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .map(|n| (n + nodes) as usize)
        .collect())
}

/// Every index an object's values or an array's items hold, moved by `by`.
fn shift_every(set: &mut Value, by: u64) {
    let items: Box<dyn Iterator<Item = &mut Value>> = match set {
        Value::Object(o) => Box::new(o.values_mut()),
        Value::Array(a) => Box::new(a.iter_mut()),
        _ => return,
    };
    for v in items {
        if let Some(i) = v.as_u64() {
            *v = json!(i + by);
        }
    }
}

/// The index at `o[key]` moved by `by`, when it has one.
fn shift(o: &mut Value, key: &str, by: u64) {
    if let Some(v) = o.get_mut(key)
        && let Some(i) = v.as_u64()
    {
        *v = json!(i + by);
    }
}

/// Every texture a material names (`baseColorTexture`, `normalTexture`, an extension's), moved.
fn textures_moved(v: &mut Value, by: u64) {
    if let Some(o) = v.as_object_mut() {
        for (k, x) in o.iter_mut() {
            if k.ends_with("Texture")
                && let Some(i) = x["index"].as_u64()
            {
                x["index"] = json!(i + by);
            }
            textures_moved(x, by);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_items::assembly::{Recipe, assemble};

    const TYPES: &str = include_str!("../../../assets/data/parts/types.json");
    const AXE: &str = include_str!("../../../assets/data/kinds/axe.json");
    const MATERIALS: &str = include_str!("../../../assets/data/parts/materials.json");

    /// A made-up part's model: one triangle of `material`, coloured by one picture of its own.
    fn model(material: &str, picture: &[u8]) -> Vec<u8> {
        let pos: Vec<u8> = [[0.0f32, 0.0, 0.0], [0.01, 0.0, 0.0], [0.0, 0.01, 0.0]]
            .iter()
            .flatten()
            .flat_map(|f| f.to_le_bytes())
            .collect();
        let mut bin = pos.clone();
        bin.extend_from_slice(picture);
        glb::write(&Glb {
            json: json!({
                "asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}],
                "nodes": [{"name": material, "mesh": 0}],
                "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 0}]}],
                "materials": [{"name": material, "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}],
                "textures": [{"source": 0}],
                "images": [{"bufferView": 1, "mimeType": "image/png"}],
                "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [0.01, 0.01, 0]}],
                "bufferViews": [{"buffer": 0, "byteLength": pos.len()}, {"buffer": 0, "byteOffset": pos.len(), "byteLength": picture.len()}],
                "buffers": [{"byteLength": bin.len()}],
            }),
            bin,
        })
    }

    fn lib() -> Library {
        let head = format!(
            r#"{{"part": "test_head", "type": "axe_head", "model": "models/parts/test_head.glb", "volume_m3": {{"value": {}}}, "centre_m": [0, 0.03, 0], "bounds_m": [[-0.03, 0, -0.03], [0.03, 0.06, 0.03]],
            "points": [{{"name": "eye.lower", "type": "eye", "at": [0, 0, 0], "dir": [0, 1, 0], "size": 0.035}},
                       {{"name": "eye.upper", "type": "eye", "at": [0, 0.06, 0], "dir": [0, 1, 0], "size": 0.035}}]}}"#,
            1.15e-4 // about 0.9 kg in iron, inside the axe's head mass
        );
        let haft = r#"{"part": "test_haft", "type": "haft", "length_m": {"value": 0.68}, "grip_from_end_m": {"value": 0.08}, "grip_diameter_m": {"value": 0.035}, "density_kg_m3": {"value": 670}}"#;
        let collar = r#"{"part": "test_collar", "type": "collar", "model": "models/parts/test_collar.glb", "mass_kg": {"value": 0.01}, "centre_m": [0, -0.01, 0], "bounds_m": [[-0.022, -0.02, -0.022], [0.022, 0, 0.022]], "cover_m": [-0.02, 0],
            "points": [{"name": "bore", "type": "bore", "at": [0, 0, 0], "dir": [0, 1, 0], "size": 0.035}]}"#;
        Library::from_json(TYPES, MATERIALS, &[AXE], &[&head, haft, collar])
            .expect("the made-up parts read")
    }

    fn item(seed: u64) -> (Assembly, Vec<u8>) {
        let l = lib();
        let recipe = Recipe {
            kind: "axe".into(),
            parts: ["test_head", "test_haft", "test_collar"]
                .map(String::from)
                .to_vec(),
            material: "iron".into(),
            seed,
        };
        let a = assemble(&l, &recipe).expect("the axe assembles");
        let bytes = write(&l, &a, |path| {
            Ok(match path {
                "models/parts/test_head.glb" => model("Metal", &[1; 8]),
                "models/parts/test_collar.glb" => model("Leather", &[2; 12]),
                _ => return Err(format!("no {path}")),
            })
        })
        .expect("the axe's model writes");
        (a, bytes)
    }

    #[test]
    fn the_same_recipe_writes_the_same_bytes() {
        assert_eq!(item(3).1, item(3).1);
        assert_ne!(
            item(3).1,
            item(4).1,
            "the recipe, seed and all, rides in its extras"
        );
    }

    #[test]
    fn an_item_reads_as_a_held_look_its_parts_where_the_assembler_put_them() {
        let (a, bytes) = item(1);
        let g = glb::read(&bytes).expect("a glTF binary");
        let nodes = g.json["nodes"].as_array().unwrap();
        // The first mesh node is the turned haft, carrying the look's extras (the held checks read them there).
        assert_eq!(nodes[0]["name"], "test_haft");
        assert_eq!(g.json["materials"][0]["name"], "Wood");
        assert_eq!(nodes[0]["extras"]["recipe"]["kind"], "axe");
        // The haft runs from its butt, the grip below the origin, to its top, +Y.
        let pos = &g.json["accessors"][nodes[0]["mesh"].as_u64().unwrap() as usize];
        assert!((pos["min"][1].as_f64().unwrap() + 0.08).abs() < 1e-6);
        assert!((pos["max"][1].as_f64().unwrap() - 0.60).abs() < 1e-6);
        // Each modelled part hangs under a node placed as the assembler placed it, never scaled.
        for p in a.placed.iter().filter(|p| p.part != "test_haft") {
            let n = nodes
                .iter()
                .find(|n| n["name"] == p.part.as_str())
                .expect("its node");
            assert_eq!(n["translation"], json!(p.at));
            assert_eq!(n["rotation"], json!(p.turn));
            assert!(n.get("scale").is_none());
        }
        // Each part keeps its own picture: the collar's material names the second texture, whose image holds its bytes.
        let collar = g.json["materials"]
            .as_array()
            .unwrap()
            .iter()
            .position(|m| m["name"] == "Leather")
            .unwrap();
        let t = g.json["materials"][collar]["pbrMetallicRoughness"]["baseColorTexture"]["index"]
            .as_u64()
            .unwrap() as usize;
        let image = g.json["textures"][t]["source"].as_u64().unwrap() as usize;
        assert_eq!(glb::image_bytes(&g, image).unwrap(), &[2; 12]);
    }
}
