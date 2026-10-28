//! Making art and looking at it: models, props, materials, characters, icons.

use super::sandbox::cargo_run;
use crate::{Task, node, run, run_owned};

pub(crate) const TASKS: &[Task] = &[
    Task {
        name: "models",
        takes: "[--port 8765]",
        does: "the model gallery in a browser: every model, its size, problems and a page each (tools/models)",
        group: "Look at things",
        run: |args| node("tools/models/serve.ts", args),
    },
    Task {
        name: "props",
        takes: "<step> [prop…]",
        does: "make props: concept, model, refine, all; fronts to see every model from the front (tools/props)",
        group: "Make art",
        run: |args| node("tools/props/props.ts", args),
    },
    Task {
        name: "textures",
        takes: "<step> [args]",
        does: "make materials: fetch from Poly Haven, material from an image, stack for the ground (tools/textures)",
        group: "Make art",
        run: |args| node("tools/textures/textures.ts", args),
    },
    Task {
        name: "cook",
        takes: "[--force]",
        does: "the texture bake (D188): models', materials' and the fire's pictures cooked into BC7, BC5 and BC4 KTX2 with mipmaps, into the cache the game reads them from; what is cooked is kept (terra_bake)",
        group: "Make art",
        run: |args| {
            let mut cargo = vec!["run", "-q", "--release", "-p", "terra_bake", "--"];
            cargo.extend(args.iter().map(String::as_str));
            run("cargo", &cargo)
        },
    },
    Task {
        name: "pack",
        takes: "<name> --from <source> [--pattern block|frame|coursed|roof]",
        does: "make a construction material in one command: its pack from a CC0 scan, an image or a reference model, checked and credited, and its file (tools/kits, D290)",
        group: "Make art",
        run: |args| node("tools/kits/pack.ts", args),
    },
    Task {
        name: "tripo",
        takes: "<command> [args]",
        does: "Tripo on our Studio subscription: image, model, rig, animate, balance… (tools/tripo)",
        group: "Make art",
        run: |args| node("tools/tripo/tripo.ts", args),
    },
    Task {
        name: "bodies",
        takes: "",
        does: "rebuild every character asset the game loads from art/ (tools/characters)",
        group: "Make art",
        run: |_| run("tools/characters/build_bodies.sh", &[]),
    },
    Task {
        name: "blender",
        takes: "<script.py> [args]",
        does: "run a script in Blender, headless (tools/blender)",
        group: "Make art",
        run: |args| run_owned("tools/blender/run.sh", args),
    },
    Task {
        name: "icons",
        takes: "[--keep <item>]… [--renders <dir>] [--sheet <prefix>]",
        does: "bake every item's icon from its look into assets/icons (D129); under WSL on the Windows GPU; --keep alone renders nothing",
        group: "Make art",
        run: icons,
    },
    Task {
        name: "paint-icons",
        takes: "<item…> | --unpainted",
        does: "paint item icons in the art style (tools/icons)",
        group: "Make art",
        run: |args| node("tools/icons/paint.ts", args),
    },
];

/// The icon baker. `--keep` with nothing else to write renders nothing (the sandbox keeps the
/// painting by fingerprinting its look's file), so it runs here. A render under WSL runs on
/// the Windows GPU through the GPU wrapper (D319), as WSL's CPU renderer never brought the
/// baker's window up (0.4% CPU for minutes, `--offscreen` too); the copy writes its icons and
/// manifest into the checkout (`terra_world::data::source_assets`), and `--renders` and
/// `--sheet` are given to it as Windows names them.
fn icons(args: &[String]) -> bool {
    let renders = args.iter().any(|a| a == "--renders" || a == "--sheet");
    let keeps = args.iter().any(|a| a == "--keep");
    if (keeps && !renders) || !crate::windows::on_wsl() {
        return cargo_run("terra_sandbox", &["--experiment", "icons"], args);
    }
    let mut all: Vec<String> = [
        "tools/gpu/gpu.ts",
        "bake",
        "--",
        "cargo",
        "xtask",
        "win",
        "-p",
        "terra_sandbox",
        "--",
        "--experiment",
        "icons",
        "--offscreen",
    ]
    .map(String::from)
    .to_vec();
    let mut path_next = false;
    for arg in args {
        if path_next {
            let folder = all.last().is_some_and(|flag| flag == "--renders");
            let Some(windows) = windows_path(arg, folder) else {
                eprintln!("icons: ✗ could not name {arg} for Windows (wslpath -w)");
                return false;
            };
            all.push(windows);
        } else {
            all.push(arg.clone());
        }
        path_next = arg == "--renders" || arg == "--sheet";
    }
    run_owned("node", &all)
}

/// A path as Windows names it, relative ones from the root, where the Linux run starts too:
/// a folder made first, a file's folder made and its name put back, as `wslpath -w` names
/// only what exists.
fn windows_path(path: &str, folder: bool) -> Option<String> {
    let path = crate::root().join(path);
    if folder {
        std::fs::create_dir_all(&path).ok()?;
        return crate::output("wslpath", &["-w", &path.to_string_lossy()]);
    }
    let (parent, name) = (path.parent()?, path.file_name()?.to_string_lossy());
    std::fs::create_dir_all(parent).ok()?;
    let parent = crate::output("wslpath", &["-w", &parent.to_string_lossy()])?;
    Some(format!("{parent}\\{name}"))
}
