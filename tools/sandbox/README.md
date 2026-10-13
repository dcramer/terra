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
r aim -45,36 --up 0.8                    # the middle of the view on the deer's chest
r press 5 e@10:60                        # the bow, then ten frames later draw for 60 and loose
r wait "An arrow hit" --timeout 20       # or fail, showing what was said instead
r said                                   # everything said since the last action
```

`start` runs the sandbox in the background on a port nothing else answers on, records the port in `shots/remote.port`, and waits until it answers; every other command talks only to that port, so commands never reach a sandbox another session is driving (`TERRA_REMOTE_PORT` overrides it); anything after `--` goes to the sandbox, and `--windows` runs it on the Windows GPU from WSL (`cargo xtask win`), which is much faster than WSL's CPU renderer. Its log is `shots/remote.log`. A sandbox started by hand with `--remote` works the same. `start` also passes `--steady`, so the game moves a sixtieth of a second every frame however fast frames are drawn, as scripted runs do: a key held for 60 frames is a second of play on any machine. `--real-time` leaves the clock alone. It passes `--background` too, so the run stays out of the way of whoever is at the machine: its window opens behind the others without taking focus, never takes the mouse, ignores the real keyboard and mouse, and is silent; only the script drives it. `--foreground` gives a normal window, to play alongside the script.

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
| `status` | The frame, the game's time, whether the clock is stopped, whether the loading screen is up, and the next line's number |
| `said [--containing TEXT] [--all] [--from LINE]` | What the run has said since the mark, with the frame of each |
| `wait TEXT [--timeout s]` | Wait until the run says something containing TEXT since the mark; on timeout, show what it said instead and fail |
| `mark` | Start `said` and `wait` from now |
| `state` | In play: where the player is and faces, their body, what they hold, carry and wear, and the creatures and people within 40 m |
| `warp EAST,SOUTH [--facing EAST,SOUTH]` | In play: move the player, as `--warp` does, and wait until they are there |
| `take ITEM[:COUNT] …` | In play: take raw items away, all of a kind if no count (`meat`, `stone:5`), so a test starts from a known pack |
| `unpress` | Let go of every key held or queued by `press` |
| `point X,Y` | Move the pointer to a place in the window, in pixels from its top left, so what is there is hovered |
| `click X,Y [--right] [--frames N]` | Move the pointer there and click, with the left button unless `--right`, held one frame unless `--frames`: what a person's click on a menu's button does |
| `aim EAST,SOUTH [--up m]` | In play: turn the view until its middle, where a bow shoots and a tool reaches, is on a spot so far above the ground (1 m), and wait until it is |
| `give ITEM[:COUNT] …` | In play: carry something, as `--give` does, or set the body (`health:20`), and wait until it is given; fails if it did not fit |
| `pose CLIP [FRACTION]` | In the workbench: hold every model that far through a clip given with `--anim`, or play it |
| `shape [--model N] TEMPLATE` or `shape heavy=0.5 thin=0.2` | In the workbench: reshape a model's body live, as a template or any mix of morphs (`appearance`) |
| `shot FILE.png` | Save a screenshot and wait for it |
| `call build.state`, `call build.lay {…}` … | In play: the player's base, read and built through the same checked operations as the hammer: `build.state`, `build.lay`, `build.opening`, `build.fill`, `build.take_apart`, `build.fit`, `build.door`, `build.undo`, `build.op`, `build.point` (see `crates/terra_game/src/build/remote.rs`) |
| `call METHOD [JSON]` | Any method; `call rpc.discover` lists them, Bevy's `world.*` ones included |

## Scripts

The commands are also a module, [client.ts](client.ts), for scripts that test the game: `start`, `stop`, `press`, `unpress`, `step`, `waitFor`, `waitUntil`, `waitSeconds`, `state`, `warp`, `aim`, `give`, `take`, `hold` and `shot`. Three use it:

- [loop.ts](loop.ts), run by `cargo xtask loop`: the play experiment's loop end to end in one running game, restarted once to load what it saved. Each step sets itself up, aims at what it needs by asking the game where it is, acts, and waits for the game to say what happened. A failing step says what the game said instead and leaves `shots/loop-<step>.png`.
- [cabin.ts](cabin.ts), run by `cargo xtask cabin`: a log cabin on a stone floor from plan to door: laid, a doorway and window cut, a roof laid, built step by step from the pack, a cube taken out and the taking undone, a refusal, a door and window fitted, the shut door keeping the player out and opened with F letting them in, saved and loaded the same. Each step checked by `build.state`; screenshots in `shots/cabin-*.png`.
- [vantage.ts](vantage.ts), run by `cargo xtask vantage [--time 13,17]`: the vantage sheet ([docs/biomes.md](../../docs/biomes.md#how-wed-know-d69)), the lush forest from fixed views (its floor at eye height, its edge, the clearing, an overlook) at each hour asked for, side by side in `shots/vantage/sheet.jpg`, so a change to how the world looks is judged on the same views. One scripted run an hour, about ten seconds each, moving its camera from view to view (`--vantage …@frame`) and saving each with `--shots`.
- [pops.ts](pops.ts), run by `cargo xtask pops [--frames 1200-1320] [-- sandbox options]`: what pops in or out, or flickers for a frame, in a stretch of a scripted run, by default the bench's walk through the lush forest at 17:00. It saves consecutive frames with `--shots` and compares each with the ones either side, small and blurred so the camera's smooth motion cancels: a pop changes a frame from the one before but not into the one after, a flicker from both neighbours, which agree. Prints each frame's share that popped and flickered, and puts the worst beside where it happened, marked red, in `shots/pops/pops.jpg`. A planted 20-pixel patch scores about 1%; a smooth walk under 0.2%. Leaves brushing the camera score high too.
- [film.ts](film.ts), run by `cargo xtask film [--frames 300-2700] [--time 15] [-- sandbox options]`: a video of a scripted run, by default the bench's 40-second walk through the lush forest, to see how the world looks and moves. Every second frame is saved with `--shots …/2` and put together by ffmpeg at 30 frames a second, so it plays at the game's speed however slowly it was drawn; `shots/film.mp4`, about 4 MB a second at 1080p. Needs ffmpeg.
- [diverge.ts](diverge.ts): where two runs of the game part ways. The sandbox writes the play experiment's whole state every frame with `--trace FILE.jsonl`; `diverge.ts A.jsonl B.jsonl` names the first frame that differs and the values that differ in it. `diverge.ts --twice -- <options for a scripted run>` runs the same run twice and compares, which says whether the game does the same thing from the same start and keys.

## What the run said, and the mark

Everything the log keeps is also kept in memory, numbered, with the frame it was said on (`crates/terra_world/src/record.rs`), so a script can ask what happened instead of counting frames. Every on-screen notice is logged, as are kills, gains, things made and much else, so most checks need no new code: act, then `wait` for the words.

The client keeps a mark in `shots/remote.mark`: the first line not yet looked at. Commands that act (`press`, `step`, `warp`, `give`) move it to now before acting, so `wait` and `said` see only what followed; `wait` moves it past what it found, so waiting twice for the same words waits for them twice. When `wait` times out it prints what was said instead, which is usually the reason.

When a check needs a fact the log does not say, add it to `play.state` (`crates/terra_game/src/play/probe.rs`) rather than parsing the screen: one place that answers what is true, read the same way by every test.

Changes made this way last only as long as the run. When a value is right, put it where it belongs (a socket's place goes in `tools/characters/export_character.py`), and check it with a workbench sheet.

## How it reaches the sandbox

The sandbox listens on `localhost:15702`, or the port `--remote-port` gives. Under WSL, a sandbox running on Windows is on Windows' own localhost, which WSL cannot reach, so the client sends through Windows' `curl.exe` there, and writes screenshots through WSL's share so they land where you asked. The remote protocol can change anything in the running world, so it only listens on localhost and only when started with `--remote`.

To add a method, see `crates/terra_world/src/remote.rs`: an experiment registers its own with `remote::method`, as the workbench does with `workbench.pose` and play with `play.state`. A method that changes the game asks for it and lets the system that already does the same for the command line do it on the next frame, as `play.warp` does with `--warp`, so the two cannot drift apart; it logs what it did, so the client can wait for that. An experiment that moves its camera in `PostUpdate` orders that `.before(remote::CameraHold)`, so a held camera wins and what it sees is not culled from the old view.
