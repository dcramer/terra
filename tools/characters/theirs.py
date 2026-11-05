"""Quaternius's parts stay as made (D599): a script that reshapes a mesh (the stretch, scaling or
re-proportioning a body, decimating it, the clipping fixer's pushes, a garment built from a
body's skin) calls `refuse(script)` before it touches anything, and stops when it was given
one of their files or a file made from them, saying what to do instead (D556).

    import theirs
    theirs.refuse("morph.py")

Their files are those under art/characters/quaternius/, the wardrobe's game pieces
(assets/<body's pieces folder>/, from assets/models/character/wardrobe.json) and the build's
copies of them (tools/characters/out/quaternius-*). A path is matched wherever it stands in an
argument (`A.gltf+B.gltf`, `--clips=PATH`, a Windows path), so no spelling slips past.
`node tools/characters/wardrobe.ts`'s `reshape` stage holds that every such script calls it.
Standard library only, so it runs inside Blender and out (its test: vendored.test.ts).
"""

import json
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def _marks():
    marks = ["art/characters/quaternius/", "tools/characters/out/quaternius-"]
    try:
        with open(os.path.join(ROOT, "assets", "models", "character", "wardrobe.json")) as f:
            bodies = json.load(f).get("bodies", {})
        marks += [f"assets/{b['pieces'].strip('/')}/" for b in bodies.values() if b.get("pieces")]
    except (OSError, ValueError):
        pass
    return marks


def named(args):
    """The arguments among `args` that name one of their files or a file made from them."""
    marks = _marks()
    hits = []
    for arg in args:
        text = arg.replace("\\", "/")
        for part in re.split(r"[+,]|=", text):
            whole = os.path.abspath(part).replace("\\", "/") if part else ""
            if any(m in part or m in whole for m in marks):
                hits.append(part)
    return hits


def refuse(script, args=None):
    """Stops `script` (exit 2) when its arguments (Blender's after `--`, else all) name theirs."""
    argv = sys.argv if args is None else args
    given = argv[argv.index("--") + 1 :] if args is None and "--" in argv else argv
    hits = named(given)
    if not hits:
        return
    print(
        f"✗ {script}: refused {', '.join(hits)}, Quaternius's or made from theirs. Their parts stay as made (D599): "
        "no stretch, scaling, re-proportioning, decimation or clipping push is ever applied to one of their pieces. "
        "To change how one looks, repaint it (new maps on their UVs); where a piece of ours clips on them, fix ours "
        "(gear_check.py --fix on our item); where one of theirs does not fit, report it with the evidence "
        "(pairings.ts, layers.ts). Do not copy their file elsewhere to get past this.",
        file=sys.stderr,
    )
    sys.exit(2)


# What marks a script that reshapes a mesh: the stretch or a garment built from a body's skin
# (skin_piece), decimation, a shrinkwrap, the clipping fixer's pushes.
RESHAPES = re.compile(r"^\s*(import|from) (stretch|skin_piece)\b|DECIMATE|SHRINKWRAP|-push\.npz|\.decimate", re.M)

# Scripts that carry a mark yet never move one of their vertices, and why. Anything else that
# carries one is refused their files by tools/blender/run.sh.
SPARED = {
    "tools/characters/fit_gear.py": "fits a rigid piece of ours over a body, moving only ours",
    "tools/characters/gear_check.py": "measures a piece of ours over a body; --fix pushes only ours",
    "tools/characters/bands.py": "lays straps of ours over a body as new ribbons",
    "tools/characters/paint_skin.py": "paints new maps on a body's UVs, the change D599 allows",
    "tools/characters/proportions.py": "measures a body, changing nothing",
    "tools/characters/test_stretch.py": "the stretch's own test, on made-up points",
    "tools/characters/skin_piece.py": "a library; the stretch it calls refuses their files itself",
}


def reshaper(script):
    """Whether the script at `script` reshapes meshes and is not spared: its reason, or None."""
    rel = os.path.relpath(os.path.abspath(script), ROOT).replace("\\", "/")
    if rel in SPARED:
        return None
    try:
        with open(script, encoding="utf-8") as f:
            hit = RESHAPES.search(f.read())
    except OSError:
        return None
    return hit.group(0).strip() if hit else None


if __name__ == "__main__" and sys.argv[1:2] == ["--list"]:
    # wardrobe.ts's reshape stage: every script under tools/ that reshapes, and the spared.
    for top, dirs, files in os.walk(os.path.join(ROOT, "tools")):
        dirs[:] = [d for d in dirs if d not in ("node_modules", "__pycache__")]
        for name in sorted(f for f in files if f.endswith(".py")):
            path = os.path.join(top, name)
            rel = os.path.relpath(path, ROOT).replace("\\", "/")
            mark = reshaper(path)
            if mark:
                print(f"RESHAPES {rel}\t{mark}")
    for rel, why in SPARED.items():
        print(f"SPARED {rel}\t{why}" if os.path.exists(os.path.join(ROOT, rel)) else f"STALE {rel}\t{why}")
elif __name__ == "__main__":
    # tools/blender/run.sh SCRIPT ARGS...: refuse a reshaping script given their files.
    if len(sys.argv) > 1 and reshaper(sys.argv[1]):
        refuse(os.path.basename(sys.argv[1]), sys.argv[2:])
