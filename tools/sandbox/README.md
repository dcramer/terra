# Sandbox remote

Drives a running sandbox from the command line over Bevy's remote protocol (D69): find things, move them, hold the camera on them, pause and step the clock, press keys, read what is true and what the run has said, wait for something to happen, pose a model, and take screenshots, all without restarting. Use it whenever you would otherwise change a number and restart to see the result, such as placing a socket, fitting gear or framing a shot, and to test a feature in one running game instead of a scripted run per step. TypeScript, run directly by Node 22.18 or later, with no dependencies.

```bash
r() { node tools/sandbox/remote.ts "$@"; }

r start --windows -- --experiment workbench --model models/character/base.glb --hold stone_axe --anim Walk_Loop
r find socket.hand                       # names containing it, with where they are
r pose Walk_Loop 0.6                     # every model held 60% through the walk
r shape --model 0 heavy=0.5 strong=0.3   # the first model's body, half heavy and a little strong
r look socket.hand.R --distance 0.9 --around 90
r place socket.hand.R --rotation 70,0,0  # its local transform; rotation in degrees
r shot shots/hand.png                    # waits for the file
r stop
```

Testing a feature in play, in one running game:

```bash
r start --windows -- --experiment play --fresh --save test.ron
r give bow quiver arrow:4                # waits until each is given
r warp -32,34 --facing -45,36            # waits until the player is there
r state                                  # where they are, their body, what they carry, what is near
r snap before                            # keep it, to say later what changed
r aim -45,36 --up 0.8                    # the middle of the view on the deer's chest
r press 5 e@10:60                        # the bow, then ten frames later draw for 60 and loose
r wait "An arrow hit" --timeout 20       # or fail, showing what was said instead
r said                                   # everything said since the last action
r diff before                            # only what changed since: ~ health: 100 -> 80, - carrying.arrow: 1
r why Player Transform                   # tick.rs:166 in show, …; drawn between ticks, last moved outside the ticks by walker.rs:512 in warp, frame 484
```

Trying values of a number in one running game, with no rebuild ([tunables](../../docs/development.md#tunables), D547):

```bash
r start --windows --release -- --experiment play --quality high --time 15 --vantage-ground --vantage 0,4,0,-600,450,300
r tune list sky.cloud                    # name = value (default, range, who set it): what it does
r pause
r shot shots/cover-0.5.png
r tune set sky.cloud.heaped_cover 0.25   # sky.cloud.heaped_cover: 0.7 -> 0.25 (default 0.7)
r shot shots/cover-0.25.png
r tune reset                             # back to their defaults: sky.cloud.heaped_cover
```

The same tunables on the debug menu's sliders (D81, [development.md](../../docs/development.md#debug-menu)), driven as a person would:

```bash
r start --windows --release -- --experiment play --debug-menu
r press backtick                         # opens it: "Debug menu: open" in the log
r menu --search heaped                   # sky.cloud.heaped_cover = 0.7 (default): slider 212 to 388 at y 273, reset at …
r click 256,273                          # a quarter along: Tune: sky.cloud.heaped_cover 0.5 -> 0.25 (…, by debug menu)
r tune get sky.cloud.heaped_cover        # = 0.25 (default 0.5, 0 to 1, by debug menu)
```

`start` runs the sandbox in the background on a free port, records the port in `shots/remote.port`, and waits until it answers; every other command talks only to that port, so commands never reach a sandbox another session is driving (`TERRA_REMOTE_PORT` overrides it); anything after `--` goes to the sandbox, and `--windows` runs it on the Windows GPU from WSL (`cargo xtask win`), which is much faster than WSL's CPU renderer. Its log is `shots/remote.log`. A sandbox started by hand with `--remote` works the same. `start` also passes `--steady`, so the game moves a sixtieth of a second every frame however fast frames are drawn, as scripted runs do: a key held for 60 frames is a second of play on any machine. `--real-time` leaves the clock alone. It passes `--background` too, so the run stays out of the way of whoever is at the machine: its window opens behind the others without taking focus, never takes the mouse, ignores the real keyboard and mouse, and is silent; only the script drives it. `--foreground` gives a normal window, to play alongside the script.

| Command | What it does |
|---|---|
| `start [--windows] [--real-time] [--foreground] [-- options]` | Start a sandbox listening, and wait for it |
| `stop` | Close it |
| `find NAME` | Entities whose name contains NAME: id, where they are, their local transform |
| `place NAME [--translation x,y,z] [--rotation x,y,z] [--scale s]` | Set the local transform of the entity called exactly NAME |
| `look NAME\|x,y,z [--distance m] [--around degrees] [--height m] [--aim m] [--from x,y,z]`, `look --vantage x,y,z,x,y,z` | Hold the camera on an entity, following it, or a point; `around` is from the model's front, `aim` how far above it to look (a person's position is their feet); `from` puts the eye at a point in the world; `--vantage`, as the sandbox's option and the kit sheet's log give it, an eye and what it looks at from the start |
| `release` | Let the camera go |
| `pause`, `resume` | Stop and start the game's clock, animations included |
| `step FRAMES` | Stop the clock, let it run that many frames, stop it again, and wait until it has |
| `press KEY[@AFTER][:FRAMES] …` | Hold keys, named as `--press` names them, for so many frames (one if not given), starting so many frames from now (none if not given): `e`, `w:60`, `f@40`, `e@90:80`. A sequence of keys goes in one command, timed against the game's frames rather than the script's |
| `status` | The frame, the game's time, whether the clock is stopped, whether the loading screen is up, the next line's number, and the run's render failures (`render_failures`, D555), each also said as a `✗ render:` line after it |
| `call sandbox.frames {"seconds": 10}` | How the frames of the last so many seconds went on the real clock: median, 99th percentile and worst of the time between frames, their work, the game's update and the GPU's, to time a stretch of play |
| `call gpu.memory {"top": 20}` | What fills the GPU's memory, from wgpu's allocator (`crates/terra_world/src/gpu_memory.rs`): this process's blocks now and at the run's peak, by the label each allocation was made with (numbers folded, so every mesh slab sums as one), and the peak's textures by asset and meshes by the named entity that draws them. Every run also logs a line at each new 256 MiB of peak and the whole breakdown as it ends. `(wgpu internal) Staging` is upload memory, host-visible and mostly in system RAM; the rest is on the card |
| `said [--containing TEXT] [--all] [--from LINE]` | What the run has said since the mark, with the frame of each |
| `wait TEXT [--timeout s]` | Wait until the run says something containing TEXT since the mark; on timeout, show what it said instead and fail |
| `mark` | Start `said` and `wait` from now |
| `state` | In play: where the player is and faces, their body, what they hold, carry and wear, and the creatures and people within 40 m |
| `snap NAME` | In play: keep `state` and `build.state` as `shots/snaps/NAME.json` (D400) |
| `diff A [B\|now] [--epsilon m] [--most N]` | What changed from snap A to B, or to the run now: a line a change, `~` changed, `+` appeared, `-` went, `#` how many changed, numbers within 0.01 the same, at most 40 lines and a count of the rest. Things in a list are matched by what they are (`id`, `name`, `who`, `what`, `kind`), and among several of a kind, nearest to nearest by `at` ([snap.ts](snap.ts)) |
| `why ENTITY [COMPONENT] [--json]` | What changed this? (D548): who spawned an entity, given by the id `find` gives or by name (exactly, in any case, then a part of it, as `find`), and for each of its components, or those named (by type path, as Bevy's `world.*` methods name them, by name in any case, then a part of it), the line of code and the function it is in (for a system, its name) that last changed it, and on which frame and how many frames ago, the latest first. A frame is the one the change's tick fell in, for the last ten minutes of frames. Lines are kept only by a build with Bevy's `track_location`, as every dev build of the sandbox is (`r start` without `--release`, `cargo xtask sandbox`, `play`, `win`) and no release or bench build is; otherwise it gives the frames and a note naming the commands that keep them. A name that matches nothing is told the names it may have meant and the call to make, several of one name their ids (D556); [why.ts](why.ts) writes the answer as those few lines. A body drawn between fixed ticks (`Smoothed`, as the player and every creature and person are) has its `Transform` last written by `terra_world::tick`'s `show`, which draws it there, so its line also says what last moved where it truly stands, in a tick or outside the ticks (a teleport), and on which frame. `node tools/sandbox/whycheck.ts --windows` checks it in play |
| `warp EAST,SOUTH [--facing EAST,SOUTH]` | In play: move the player, as `--warp` does, and wait until they are there |
| `take ITEM[:COUNT] …` | In play: take raw items away, all of a kind if no count (`meat`, `stone:5`), so a test starts from a known pack |
| `unpress` | Let go of every key held or queued by `press` |
| `point X,Y` | Move the pointer to a place in the window, in pixels from its top left, so what is there is hovered |
| `click X,Y [--right] [--frames N]` | Move the pointer there and click, with the left button unless `--right`, held one frame unless `--frames`: what a person's click on a menu's button does |
| `aim EAST,SOUTH [--up m]` | In play: turn the view until its middle, where a bow shoots and a tool reaches, is on a spot so far above the ground (1 m), and wait until it is |
| `give ITEM[:COUNT] …` | In play: carry something, as `--give` does, or set the body (`health:20`), and wait until it is given; fails if it did not fit |
| `pose CLIP [FRACTION]` | In the workbench: hold every model that far through a clip given with `--anim`, or play it |
| `shape [--model N] TEMPLATE` or `shape heavy=0.5 thin=0.2` | In the workbench: reshape a model's body live, as a template or any mix of morphs (`appearance`) |
| `call build.state`, `call build.lay {…}` … | In play: the player's base, read and built through the same checked operations as the hammer: `build.state`, `build.lay`, `build.opening`, `build.fill`, `build.take_apart`, `build.fit`, `build.door`, `build.undo`, `build.op`, `build.point`, `build.look` (the camera in the base's own metres), `build.example` (the castle put up as the base), `build.damage_drawn` (damage drawn or hidden everywhere, for a check), `build.material`, `build.replace`, `build.carve`, `build.take_out`, `build.cube`, `build.ground` (the ground's height under columns, to see what building changed of it), and harm and its mending (D305): `build.damage`, `build.subside`, `build.repair`; what comes down breaks into fragments, counted in `build.state` (see `crates/terra_building/src/remote.rs`), and `build.memory`, what every base holds on the CPU and the GPU |
| `call storage.place {…}`, `storage.put`, `storage.state` | In play: set down an item from the build menu, a chest if none is named, paid for as the hammer pays; put things from the pack into the storage within reach; read the storage in the claim and what each holds (see `crates/terra_survival/src/storage.rs`) |
| `debug.view NAME` | Colour every building by a debug view, its legend on screen, as F4 does in the game: `texels`, `roles`, `detail`, `damage`, `shade`, `normals`, `plan`, `light`, `off`, or `next` and `back` ([development.md](../../docs/development.md#buildings-debug-views)) |
| `tune list [PREFIX]`, `tune get NAME`, `tune set NAME VALUE\|default`, `tune reset [PREFIX]` | The run's tunables (D547, [development.md](../../docs/development.md#tunables)), named numbers its code reads live, such as `sky.cloud.heaped_cover`: a line each with its value, default, range, what set it and what it does; set one within its range (`sky.cloud.heaped_cover: 0.7 -> 0.25 (default 0.7)`, logged as `Tune: …` for `wait`) until the run ends; put changed ones back. `--tune NAME=VALUE` sets one at the start. [tunecheck.ts](tunecheck.ts) checks a value reaches the picture in one running game |
| `menu [open\|close] [--search TEXT] [--page NAME] [--json]` | The debug menu (D81), in a run started with `--debug-menu`: open or close it (the backtick does too, `press backtick`), search its tunables or turn to a page, and say what it shows and where each slider and reset was drawn, in the pixels `click` takes, so a script sets a tunable as a person would; a run without the flag is refused, saying how to start one with it |
| `shot FILE.png` | Save a screenshot and wait for it. If anything has failed to render in the run (a pipeline whose shader did not compile, an error wgpu reported, D555), it says each once as a `✗ render:` line and exits 1, as does every script that shoots through `client.ts`; `call sandbox.shot {"path": …, "frames": N}` saves the next N frames one after another, as `FILE-00000.png` on, to compare frame by frame what a change does as it happens |
| `call METHOD [JSON]` | Any method; `call rpc.discover` lists them, Bevy's `world.*` ones included |

## Scripts

The commands are also a module, [client.ts](client.ts), for scripts that test the game: `start`, `stop`, `press`, `unpress`, `step`, `waitFor`, `waitUntil`, `waitSeconds`, `state`, `warp`, `aim`, `give`, `take`, `hold` and `shot`.

The scripts that test the game, by what they look at. A script's full entry (how it is run, what it says) is its line in [scripts.md](scripts.md): grep it, `grep -n "^- \[qa.ts\]" tools/sandbox/scripts.md`, never read the whole file.

- **The loop and building**: [loop.ts](loop.ts) (`cargo xtask loop`, the play loop end to end), [cabin.ts](cabin.ts) (`cargo xtask cabin`), [rules.ts](rules.ts) (`cargo xtask rules`, every building rule live), [demo.ts](demo.ts) (`cargo xtask demo`, the construction demo's pictures), [artifacts.ts](artifacts.ts) (`cargo xtask artifacts`, building's junk only frames show; [png.ts](png.ts), [frames.ts](frames.ts)).
- **QA clips** ([qa.ts](qa.ts), `cargo xtask qa`): the scenarios [build-walls.ts](qa/build-walls.ts), [build-openings.ts](qa/build-openings.ts), [build-roof.ts](qa/build-roof.ts), [build-swap.ts](qa/build-swap.ts), [build-carve.ts](qa/build-carve.ts), [build-storey.ts](qa/build-storey.ts), [build-deck.ts](qa/build-deck.ts), [destroy.ts](qa/destroy.ts), [film.ts](qa/film.ts), [harm.ts](qa/harm.ts), [materials.ts](qa/materials.ts), [shapes.ts](qa/shapes.ts), [castle.ts](qa/castle.ts), written with [script.ts](qa/script.ts) over [lib/cabin.ts](qa/lib/cabin.ts); quiet-machine waits by [load.ts](load.ts).
- **Places and looks**: [places.ts](places.ts) (the places, D326), [vantage.ts](vantage.ts) (`cargo xtask vantage`), [looks.ts](looks.ts) (`cargo xtask looks`, D356) measured by [look.ts](look.ts) against `looks/*.json`, [wind.ts](wind.ts) (D597), [dungeon.ts](dungeon.ts), [forest-walk.ts](forest-walk.ts) (D467), [torchlight.ts](torchlight.ts), [film.ts](film.ts) (`cargo xtask film`).
- **What pops and what stops**: [collide.ts](collide.ts) (`cargo xtask collide`, D267, walk-ins by [ways.ts](ways.ts)), [lodcheck.ts](lodcheck.ts) (`cargo xtask lodcheck`), [pops.ts](pops.ts) (`cargo xtask pops`), [popin.ts](popin.ts).
- **Continuous checks**: [groundlight.ts](groundlight.ts), [turnlight.ts](turnlight.ts), [turnflash.ts](turnflash.ts), [volumelight.ts](volumelight.ts), [indoorlight.ts](indoorlight.ts) (D485), [alone.ts](alone.ts) (D623), [saved.ts](saved.ts) (D579).
- **The trailer** ([trailer.ts](trailer.ts), `cargo xtask trailer`, D357): routes smoothed by [timeline.ts](timeline.ts), long runs reported by [progress.ts](progress.ts); scenes [dragon.ts](trailer/scenes/dragon.ts), [troll.ts](trailer/scenes/troll.ts), [lakesunset.ts](trailer/scenes/lakesunset.ts), [settlement.ts](trailer/scenes/settlement.ts), [cetri.ts](trailer/scenes/cetri.ts) (D534), [flight.ts](trailer/scenes/flight.ts); fliers planned by [flight.ts](trailer/flight.ts) and plotted by [flightplot.ts](flightplot.ts) (D382).
- **The duel** ([duel.ts](duel.ts), `cargo xtask duel`): [checks.ts](duel/checks.ts) judges the fight records, [moments.ts](duel/moments.ts) picks the sheet, [shield.ts](duel/shield.ts) and [pose.ts](duel/pose.ts) measure the shield and the shown pose; a run's output passed on by [streamed.ts](streamed.ts).
- **v0** ([v0.ts](v0.ts)): [front.ts](front.ts): [title.ts](title.ts), [join.ts](join.ts), [creator.ts](creator.ts); [realms.ts](realms.ts): portal.ts, [gate.ts](gate.ts), crossing.ts; den.ts, [fight.ts](fight.ts) (fight/plan.ts); land.ts, ambience.ts, farlook.ts, heard.ts (heard/judge.ts).
- **The engine and the tools**: [sheet.ts](sheet.ts) (one sheet), [tunecheck.ts](tunecheck.ts) (a tunable reaches the picture), [menucheck.ts](menucheck.ts) (the debug menu), [whycheck.ts](whycheck.ts) (`why`), [net.ts](net.ts) (a host and a joined copy: one place and a dig in both, D638; `--soak M`; `--headless`: no game, D640), [portal.ts](portal.ts) (the portal's census), [diverge.ts](diverge.ts) (where two traced runs part), [frametimes.ts](frametimes.ts) (two builds' bench runs compared).

## What the run said, and the mark

Everything the log keeps is also kept in memory, numbered, with the frame it was said on (`crates/terra_world/src/record.rs`), so a script can ask what happened instead of counting frames. Every on-screen notice is logged, as are kills, gains, things made and much else, so most checks need no new code: act, then `wait` for the words.

The client keeps a mark in `shots/remote.mark`: the first line not yet looked at. Commands that act (`press`, `step`, `warp`, `give`) move it to now before acting, so `wait` and `said` see only what followed; `wait` moves it past what it found, so waiting twice for the same words waits for them twice. When `wait` times out it prints what was said instead, which is usually the reason.

When a check needs a fact the log does not say, add it to `play.state` (`crates/terra_game/src/play/probe.rs`) rather than parsing the screen: one place that answers what is true, read the same way by every test.

Changes made this way last only as long as the run. When a value is right, put it where it belongs (a socket's place goes in `tools/characters/export_character.py`, a tunable's default in its owner's code), and check it with a workbench sheet.

## How it reaches the sandbox

The sandbox listens on `localhost:15702`, or the port `--remote-port` gives. Under WSL, a sandbox running on Windows is on Windows' own localhost, which WSL cannot reach, so the client sends through Windows' `curl.exe` there, and writes screenshots through WSL's share so they land where you asked. The remote protocol can change anything in the running world, so it only listens on localhost and only when started with `--remote`.

To add a method, see `crates/terra_world/src/remote.rs`: an experiment registers its own with `remote::method`, as the workbench does with `workbench.pose` and play with `play.state`. A method that changes the game asks for it and lets the system that already does the same for the command line do it on the next frame, as `play.warp` does with `--warp`, so the two cannot drift apart; it logs what it did, so the client can wait for that. An experiment that moves its camera in `PostUpdate` orders that `.before(remote::CameraHold)`, so a held camera wins and what it sees is not culled from the old view.
