# Model viewer

Shows any model in `assets/models` or `art` in a web browser, so a model can be looked at without Blender or the sandbox (D71): orbit and zoom, beside the 1.8 m figure, over a metre grid (tenths near the middle), with an arrow at each model's front (glTF +Z). Beside it: each model's size, triangles, lowest point and centre (both should be 0 for a model the game loads), its materials with their colours, and its animations, which play when clicked. It draws with three.js, not the game's renderer, so it shows shape, size, orientation and colour regions faithfully but not the game's exact look; the sandbox's workbench (`--prop`, `--hold`) is the check for that.

```bash
node tools/models/serve.ts            # http://localhost:8765/
```

From WSL, open the address in a Windows browser. Click a model to show it; shift-click to add one beside it. A model rewritten on disk, as `tools/props` rewrites one, is loaded again within two seconds.

**Links.** The address always holds the view, so a link opens it again, for a person or a script:

```
http://localhost:8765/?m=assets/models/props/Forge.glb,assets/models/props/Chair.glb&view=front&shading=materials&wire=1&figure=0
```

| Parameter | What it does |
|---|---|
| `m` | The models shown, by path from the repository root, comma separated |
| `view` | `front`, `side`, `top` or `three` (three-quarter, the default) |
| `shading` | `file` (its own colours, the default), `materials` (a colour per material, to see which faces each covers) or `normals` |
| `wire` | `1` for wireframe |
| `figure` | `0` to hide the figure |

**Scripts.** Once `body[data-ready=true]`, `window.viewer.measured()` returns what is shown with its measurements, and `window.viewer.show([paths])` and `window.viewer.view(name)` change it. With agent-browser:

```bash
agent-browser open "http://localhost:8765/?m=assets/models/props/Bed.glb&view=front"
agent-browser wait 'body[data-ready=true]'
agent-browser eval 'JSON.stringify(window.viewer.measured())'
agent-browser screenshot shots/bed.png
```

| File | What it is |
|---|---|
| `serve.ts` | The server: the page, a list of the models, and the files under `assets/models` and `art`, nothing else, on localhost only |
| `index.html` | The viewer; three.js 0.170 from jsDelivr |
