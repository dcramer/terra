"""Checks the Blender scripts it is given without running them: each one compiles, what it
imports exists in this Blender, and what it uses from a module beside it is there. The
modules other scripts import are imported for real, since they run nothing when imported.
`cargo xtask check` runs it on every tracked script.

    tools/blender/run.sh tools/blender/check.py tools/characters/look.py ...

A script that runs in another program's Python (Kimodo's) says so in a line `# runs-in: <program>`
and is only compiled: what it imports is that program's, not this Blender's.

Prints a line for each fault and ends with BLENDERCHECK pass or BLENDERCHECK fail.
"""

import ast
import importlib
import importlib.util
import os
import sys

paths = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
faults = []
elsewhere = []
trees = {}
for path in paths:
    try:
        with open(path, encoding="utf-8") as f:
            text = f.read()
        tree = ast.parse(text, path)
        if any(line.startswith("# runs-in: ") for line in text.splitlines()):
            elsewhere.append(path)
        else:
            trees[path] = tree
    except SyntaxError as e:
        faults.append(f"{path}:{e.lineno}: {e.msg}")

# Our own modules are the .py files in the scripts' folders; a script may reach another
# folder's, as the props scripts do look.py. Two folders may each have one of a name
# (haft.py): a script means its own folder's, or else the one that defines what it takes.
ours = {}
for folder in sorted({os.path.dirname(os.path.abspath(p)) for p in paths}):
    for n in os.listdir(folder):
        if n.endswith(".py"):
            ours.setdefault(n[:-3], []).append(folder)


def defines(folder, name, taken):
    with open(os.path.join(folder, name + ".py"), encoding="utf-8") as f:
        top = ast.parse(f.read()).body
    names = {t.id for n in top if isinstance(n, ast.Assign) for t in n.targets if isinstance(t, ast.Name)}
    names |= {n.name for n in top if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
    return all(a in names for a in taken or [])


def home(path, name, taken):
    folders = ours[name]
    own = os.path.dirname(os.path.abspath(path))
    if own in folders:
        return own
    return next((f for f in folders if defines(f, name, taken)), folders[0])


shared = {}
for path, tree in trees.items():
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            names = [(a.name.split(".")[0], None, node.lineno) for a in node.names]
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            names = [(node.module.split(".")[0], [a.name for a in node.names], node.lineno)]
        else:
            continue
        for top, taken, line in names:
            if top in ours:
                shared.setdefault((home(path, top, taken), top), []).append((path, line, taken))
            elif importlib.util.find_spec(top) is None:
                faults.append(f"{path}:{line}: no module {top} in this Blender")

for (folder, name), users in sorted(shared.items()):
    sys.path.insert(0, folder)
    try:
        module = importlib.import_module(name)
    except Exception as e:  # anything a module raises on import is the fault being looked for
        faults.append(f"{os.path.join(folder, name)}.py: importing it fails: {e!r}")
        continue
    finally:
        sys.path.pop(0)
    for path, line, taken in users:
        for attr in taken or []:
            if attr != "*" and not hasattr(module, attr):
                faults.append(f"{path}:{line}: {name} has no {attr}")
    # What `import name` users reach for as name.attr.
    for path, tree in trees.items():
        if not any(p == path and t is None for p, _, t in users):
            continue
        for node in ast.walk(tree):
            if (
                isinstance(node, ast.Attribute)
                and isinstance(node.value, ast.Name)
                and node.value.id == name
                and not hasattr(module, node.attr)
            ):
                faults.append(f"{path}:{node.lineno}: {name} has no {node.attr}")
    del sys.modules[name]

for fault in faults:
    print(f"  {fault}")
print(f"BLENDERCHECK {'fail' if faults else 'pass'}: {len(paths)} scripts" + (f", {len(elsewhere)} only compiled (runs-in)" if elsewhere else ""))
sys.exit(1 if faults else 0)
