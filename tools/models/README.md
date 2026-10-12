# Model viewer

Every model in `assets/models` and `art`, in a web browser, without Blender or the sandbox (D71).

- **The gallery** (the front page) draws every model the same way: one camera angle, the game's light, metre squares, and the 1.8 m figure idling beside it at the same scale, so proportions read at a glance. Each tile gives its height, width and depth, and triangles, and flags a catalogue model (props, village) that floats, is sunk or is off-centre. Filter by path, order by name, height, triangles or problems, and draw every tile at one scale to compare sizes across tiles.
- **A model's page** (click a tile) shows it full screen: orbit and zoom, an arrow at its front (glTF +Z), its size, triangles, lowest point, centre, materials with their colours, and its animations, which play when clicked. Shading by material shows which faces each material covers; wireframe, normals, its box and origin, and its skeleton are a click away.

It draws with three.js, not the game's renderer, so it shows shape, size, orientation and colour regions faithfully but not the game's exact look; the sandbox's workbench (`--prop`, `--hold`) is the check for that. A model rewritten on disk, as `tools/props` rewrites one, is drawn again within two seconds.

```bash
node tools/models/serve.ts            # http://localhost:8765/
```

From WSL, open the address in a Windows browser.

**Links.** The address always holds the view, so a link opens it again, for a person or a script.

| Parameter | Page | What it does |
|---|---|---|
| `q` | gallery | Only models whose path contains this, such as `props` |
| `sort` | gallery | `height`, `triangles` or `problems`; by name if not given |
| `scale` | gallery | `same` to draw every tile at one scale |
| `size` | gallery | Tile width in pixels: `150`, `200` or `300` |
| `m` | model | The models shown, by path from the repository root, comma separated; several stand side by side |
| `view` | model | `front`, `side`, `top` or `three` (three-quarter, the default) |
| `shading` | model | `file` (its own colours, the default), `materials` or `normals` |
| `wire`, `bounds`, `bones` | model | `1` for wireframe, its box and origin, or its skeleton |
| `figure` | model | `0` to hide the figure |

For example `?q=props&sort=height`, or `?m=assets/models/props/Forge.glb,assets/models/props/Chair.glb&view=front&shading=materials`.

**Scripts.** Once `body[data-ready=true]`, `window.viewer.measured()` returns the models on a model's page with their measurements and problems; in the gallery, once `body[data-thumbs=done]`, `window.viewer.tiles()` returns every tile's. `window.viewer.open([paths])` and `window.viewer.view(name)` change what is shown. With agent-browser:

```bash
agent-browser open "http://localhost:8765/?m=assets/models/props/Bed.glb&view=front"
agent-browser wait 'body[data-ready=true]'
agent-browser eval 'JSON.stringify(window.viewer.measured())'
agent-browser screenshot shots/bed.png
agent-browser open "http://localhost:8765/?q=props"           # every prop with a problem:
agent-browser wait 'body[data-thumbs=done]'
agent-browser eval 'JSON.stringify(window.viewer.tiles().filter(t => t.problems.length))' 
```

| File | What it is |
|---|---|
| `serve.ts` | The server: the page, a list of the models, and the files under `assets/models` and `art`, nothing else, on localhost only |
| `index.html` | The gallery and the model page; three.js 0.170 from jsDelivr |
