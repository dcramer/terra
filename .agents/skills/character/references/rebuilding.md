# Character: rebuilding everything, and into the game

## 7. Rebuilding everything, and into the game

```bash
tools/characters/build_bodies.sh
```

One command rebuilds every character asset the game loads from the source art in `art/`, for free: the base, its gear, every morph (each checked with every look on it), each template's painted skin, and the game's files:

| File | What it is |
|---|---|
| `assets/models/character/base.glb` | The base body with every morph as a morph target, and each morph's joint and socket offsets in the skeleton node's glTF extras (`{"morphs": {M: {node: [x, y, z]}}}`) |
| `assets/models/character/skins/<template>.png` | A template's painted skin, on the base's layout |
| `assets/models/character/templates.json` | The templates (D111): each a mix of morphs and a skin. Edited by hand |
| `assets/models/gear/<item>.glb` | Each worn look, with the same morph targets in the same order |

Run it after changing a script it calls, or after adding a morph or a skin (`bodies.json`); a piece of gear is its data, read as it runs. It is incremental (D563, `build.ts`): a step whose scripts, data and upstream outputs are unchanged is skipped, a changed one reruns with what it reaches, and a run killed part-way resumes; `--plan` says what would run. A line a step (built and why, skipped, failed); each step's output in `tools/characters/out/steps/`. It ends with `BUILD pass`, or `BUILD fail` and every failure a step printed or exited with (`build_verdict.ts`); its whole output is in `tools/characters/out/build.log`. Today it fails on known faults alone, listed in D593 (the smith's apron's lap on four bodies, thin's shoes swimming, the quiver in a swim dive), and the dressed man's Sprint_Loop skating 2.7/7.8 cm (`markers.ts`): a new failure is anything else, or one of those worse. Cold, about half an hour; a rerun with nothing changed, about a minute; one garment changed, only that garment on each body and its game file. Every step it builds goes into the step cache shared by all worktrees (`~/.cache/terra/steps`, by the stamp's key), so a fresh worktree restores what another built from the same inputs (`↓ restored`) rather than building it; after each build the steps used longest ago are dropped to keep it under `$TERRA_STEP_CACHE_GB` (5 GB). It makes `fight.glb` too (the `fight` step: `art/animation/sets/fight.json`'s takes, its holds by `hold.py`, mirrored and markers baked); a change to the set or the holds rebuilds that step alone. Quaternius's bodies come out of it as well, from the wardrobe catalogue (D611, `assets/models/character/wardrobe.json`: bodies, pieces, looks as rows; a new piece or look is a row, never code), a `wardrobe:BODY` step each: the bare cuts (`cut_slots.py`), every piece as its own glTF with shared textures (`export_pieces.py`, `assets/models/character/male/<piece>.gltf`), `intact.ts` on each, and the body's file with no mesh (`male.glb`: his skeleton, sockets and the fight's clips sampled on him and held, the sockets and lift read off his default look); the game composes him a piece a slot (`terra_walker::wardrobe`). `--step wardrobe:male` builds it and what it needs alone (about 4 minutes cold, also rewriting `fight.glb`, whose floats move with the Blender build: restore it if you did not mean to change it); `node tools/characters/wardrobe.ts` then checks every look on its body (pieces theirs, bound to his skeleton, every slot filled or bare, nothing drawn twice, every pairing measured). A new worn piece is measured against every worn piece of the other slots by `node tools/characters/pairings.ts`, which writes the catalogue's `pairings` (never by hand; a mix judged against its two outfits' own pairings, D611); the game composes no refused pairing and no look may wear one. That needs each step to make the same bytes from the same inputs: `build.ts --again STEP,...` runs steps a second time and names any output that differs and where (`glb_diff.ts`); a script that makes elements in an order of memory addresses (a set of Blender elements, an extrusion's output) sorts them first. `make_morph.sh` makes its morph and fits through the same build (`build.ts --morph NAME`). Start it in the background (`cargo xtask gpu bake --stall 60 -- tools/characters/build_bodies.sh`). Don't edit the scripts while it runs: each step loads the Python afresh, so a change mid-run mixes two versions.

Rewriting the items' files changes their bytes, so `cargo xtask check` then finds their painted icons stale. If an item's look did not change, compare its render with its icon (`cargo xtask icons --renders DIR`) and keep the painting, `cargo xtask icons --keep <item>`. If it did change, paint it again (the `item-icons` playbook).

**In the game** (`crates/terra_walker/src/appearance.rs`):
- A body's `Appearance` (morph weights and skin) sets its morph weights by name.
- Each of its joints and sockets moves by the mix of its morphs' offsets, kept there under animation.
- Its skin, and everything worn on it, gets bind poses for the moved joints, shared between bodies that look the same.

**Check it in the game's own renderer**: each template side by side, dressed, through the clips, with where its sockets stand:

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/base.glb --template male --model models/character/base.glb --template female --model models/character/base.glb --template heavy --wear leather_coat --wear trousers --wear small_pack --anim Idle_Loop --anim Jog_Fwd_Loop --anim Sword_Attack --sides 30,200 --moments 0.3,0.7 --joints --sheet shots/wb/templates --clean --settle 60
```

- **The `JOINTS` lines**, in Blender's axes, must match the `SOCKETS` lines the export printed for that morph (`tools/characters/out/M-game.glb`) to a millimetre or so: that is the game shaping a body as the tools do.
- **Live, with `--remote`**: `node tools/sandbox/remote.ts shape --model 2 heavy=0.5 strong=0.3` reshapes a body in the running workbench, to look at any mix.

### Gear on every body

A worn look is made once, on the base, and every morph carries it (`carry_gear.py`):
- **Skinned parts close to the skin** follow it vertex by vertex: garments, straps.
- **Parts that stand off** move as one, far enough to keep their clearance over the skin that grew most beneath them: a pack's bag.
- **A look hung on a socket** moves with its socket, which each morph moves, and then by the same rule as a part that stands off. A bigger seat does not swallow a quiver. Its shift per morph is in its extras (`morphs`), and the game mixes it by the body's morph weights (`character::Shifts`).
- **The clipping fixer.** A morph can still bring skin through a look, as a heavy flank through a strap in a swing.
  - `make_morph.sh` checks each look on the morph's body with `gear_check.py --fix`. That records how far each part of the look must move out to clear the skin in every frame, plus 4 mm.
  - It then carries the look again with that push baked into the morph, away from the skin and spread smoothly round each place, and checks again, for up to three rounds.
  - A round is kept only while the look gets better. One that makes it worse is undone, and no more are tried.
  - That is BodySlide's clipping fixer per extreme, through our clips as well as at rest. The game's files carry the same pushes (`out/M-ITEM-push.npz`).
  - What a push cannot fix: clipping that happens only in pose, where it would have to go one way in one frame and another in the next. A thigh lifting into a coat's skirt is that kind. Pushing the skirt out sinks it into the belly at rest, and it measured worse each round, so its rounds are undone. That wants the look's weights fixed (the skirt partly following the thigh), not more push.

