# terra_motion

How bodies move and fight, by rules every player and the server agree on (D447, D28). No Bevy: it tests in a fraction of a second and a server runs it without a window. What shows (spawning, reading intent, clips, flashes, sounds) is `terra_creatures::melee`'s.

- `moves`: move sets as data (`assets/data/moves/*.ron`, which explains each field) and the move runner: phases, cancel windows, the press buffer, i-frames, the judge of a blow on a guard, and a blow's step in to land.
- `blade`: a blade from hilt to tip swept from tick to tick against a body's hurt capsule.
- `contact`: what a blow does to both fighters (hit-stop, damage, flinch, stagger, broken guard, shoves), and a fighter's state those change (`Fighting`).
- `stamina`: what a move costs (a field of its set) and what a body too tired for it may still do (D449, a placeholder until the developers decide).
- `duel` (tests): two plain bodies on a line driven by scripted presses at 60 Hz through these rules, logging each move, contact and outcome; it fails on an idle frame inside a chain asked for, a blow in range that does not land in its active frames, a parry that does not stagger, a roll's i-frames that do not dodge, or a cost not paid or a too-tired rule not kept.

The crate grows into [docs/movement.md](../../docs/movement.md)'s intent, mover and actions layers as their stages land; one domain crate (D407), not a crate per feature.

`cargo test -p terra_motion`
