# Sandbox remote

Drives a running sandbox from the command line over Bevy's remote protocol (D69): find things, move them, hold the camera on them, pause, pose a model, and take screenshots, all without restarting. Use it whenever you would otherwise change a number and restart to see the result, such as placing a socket, fitting gear or framing a shot. TypeScript, run directly by Node 22.18 or later, with no dependencies.

```bash
r() { node tools/sandbox/remote.ts "$@"; }

r start --windows -- --experiment workbench --model models/character/base-male.glb --hold stone_axe --anim Walk_Loop
r find socket.hand                       # names containing it, with where they are
r pose Walk_Loop 0.6                     # every model held 60% through the walk
r look socket.hand.R --distance 0.9 --around 90
r place socket.hand.R --rotation 70,0,0  # its local transform; rotation in degrees
r shot shots/hand.png                    # waits for the file
r stop
```

`start` runs the sandbox in the background on a port nothing else answers on, records the port in `shots/remote.port`, and waits until it answers; every other command talks only to that port, so commands never reach a sandbox another session is driving (`TERRA_REMOTE_PORT` overrides it); anything after `--` goes to the sandbox, and `--windows` runs it on the Windows GPU from WSL (`cargo xtask win`), which is much faster than WSL's CPU renderer. Its log is `shots/remote.log`. A sandbox started by hand with `--remote` works the same.

| Command | What it does |
|---|---|
| `start [--windows] [-- options]` | Start a sandbox listening, and wait for it |
| `stop` | Close it |
| `find NAME` | Entities whose name contains NAME: id, where they are, their local transform |
| `place NAME [--translation x,y,z] [--rotation x,y,z] [--scale s]` | Set the local transform of the entity called exactly NAME |
| `look NAME\|x,y,z [--distance m] [--around degrees] [--height m] [--aim m]` | Hold the camera on an entity, following it, or a point; `around` is from the model's front, `aim` how far above it to look (a person's position is their feet) |
| `release` | Let the camera go |
| `pause`, `resume` | Stop and start the game's clock, animations included |
| `pose CLIP [FRACTION]` | In the workbench: hold every model that far through a clip given with `--anim`, or play it |
| `shot FILE.png` | Save a screenshot and wait for it |
| `call METHOD [JSON]` | Any method; `call rpc.discover` lists them, Bevy's `world.*` ones included |

Changes made this way last only as long as the run. When a value is right, put it where it belongs (a socket's place goes in `spike/character_gear/export_character.py`), and check it with a workbench sheet.

## How it reaches the sandbox

The sandbox listens on `localhost:15702`, or the port `--remote-port` gives. Under WSL, a sandbox running on Windows is on Windows' own localhost, which WSL cannot reach, so the client sends through Windows' `curl.exe` there, and writes screenshots through WSL's share so they land where you asked. The remote protocol can change anything in the running world, so it only listens on localhost and only when started with `--remote`.

To add a method, see `crates/terra_sandbox/src/remote.rs`: an experiment registers its own with `remote::method`, as the workbench does with `workbench.pose`. An experiment that moves its camera in `PostUpdate` orders that `.before(remote::CameraHold)`, so a held camera wins and what it sees is not culled from the old view.
