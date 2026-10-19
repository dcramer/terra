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
        takes: "[args]",
        // Every item's icon rendered and written into assets/icons. It opens a window for the
        // few seconds it takes; under WSL the CPU renderer is enough.
        does: "bake every item's icon from its look into assets/icons (D129)",
        group: "Make art",
        run: |args| cargo_run("terra_sandbox", &["--experiment", "icons"], args),
    },
    Task {
        name: "paint-icons",
        takes: "<item…> | --unpainted",
        does: "paint item icons in the art style (tools/icons)",
        group: "Make art",
        run: |args| node("tools/icons/paint.ts", args),
    },
];
