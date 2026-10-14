# Foliage at the camera and at the player

2026-10-14. Question, from an adversarial review of the lush forest's film (`cargo xtask film --time 15`): the third-person camera sits inside shrubs and spruces, a near-lens bush fills the screen, and the player runs through a hazel as if it were not there. How do shipped games handle foliage near the camera and the player, and what does doing the same change, measured?

Sources are marked **P** (the developers' own material) or **C** (third-party guides, tools, community). The talks with the detail (Sucker Punch's GDC 2021 grass and wind talks, Guerrilla's 2018 vegetation talk) are behind GDC Vault; abstracts and write-ups were used.

## What games do

- **Plants yield to bodies: foliage interaction.** Ghost of Tsushima's grass and wind systems take character displacement, and wind the player's movement makes, as inputs (P, [PlayStation Blog](https://blog.playstation.com/?p=345372)); its grass is generated per blade on the GPU and bent in its vertex shader by height up the blade (P, [GDC Vault abstract](https://gdcvault.com/play/1027214/Advanced-Graphics-Summit-Procedural-Grass); C, [GodotGrass](https://github.com/2Retr0/GodotGrass)). Engines do it in the vertex shader from a few nearby bodies, or from a displacement texture round the camera when there are many.
- **What hides the character fades: a camera-obstruction fade.** Foliage on the line from the camera to the character is dithered out in a moving pattern for TAA to resolve; Unreal's DitherTemporalAA is the usual way, used for masked materials, camera-obstruction fades and LOD crossfades, and it ghosts where temporal history is weak (C, [guide](https://www.seeles.ai/resources/blogs/unreal-engine-dither-temporal-aa-lod-fade-guide)). Middleware does the same by line of sight to a target (C, Unity's See Through, Opsive's [Object Fader](https://opsive.com/support/documentation/ultimate-character-controller/camera/object-fader/)). Players ask for it where a game lacks it: Enshrouded's tracker has requests to fade foliage between the camera and the character (C, [featureupvote](https://enshrouded.featureupvote.com/suggestions/720007/make-foliage-between-the-camera-and-the-player-character-transparent)).
- **Leaves at the lens fade too: a near-camera fade.** The same dither by distance from the camera, so the camera does not look through a card at the lens.
- **The camera's boom pulls in against solid things, not plants.** Shrubs are not solid, so the camera passing through them is answered by the fades, not by collision.

## What we found

- D216 already faded leaves within 1.6 m of the camera; D264 narrowed that to 0.3 to 0.9 m, since wider dithered a bough by the camera into a see-through smear. It works, but at the wrong reach for a camera 4.6 m behind the player.
- Our camera (`terra_game` `play/camera.rs`) pulls in before build walls and the ground, not before trunks or rocks (`Obstacles`); shrubs are not solid (D243), so it backs straight into them.
- Tinting leaves by distance from the eye showed the leaves filling the old film's frames were mostly 1 to 3 m from the lens, where nothing faded; with the camera 4.6 m behind the player, a hazel round the camera puts its leaf mass at 1.5 to 2.5 m on every side.
- The player inside a hazel stood among leaves within 0.35 m in front of their chest: a tube that ends short of the body's front does not show them. It has to reach the body's middle.
- Pushing by the wind weight's square root (a vertex's height up its plant) at 1.5 moved a hazel's leaves at chest height under 0.2 m; at 3.0, about 0.35 m, enough to read as pushing through.

## Measured

The same film, this change (D271) against main filmed at the same time, sampled every 0.5 s (80 frames). "Near-lens foliage" is counted per 15-pixel block at 480×270: mostly foliage green and with little fine detail for its brightness, since leaf cards magnified at the lens lose their high frequencies (a throwaway script, checked by eye against masks of the frames; it misses dark spruce sprays at the lens and counts a big-leaved hazel 3 to 4 m off).

| | main | this change |
|---|---|---|
| Frames where near-lens foliage covers over 30% | 11 (14%) | 6 (8%) |
| Frames over 80% (the screen all leaves) | 2 | 0 |
| Mean share of the screen | 13.7% | 9.3% |
| Frames where the player is lost in foliage, by eye (not looking up) | about 10 | none |
| Frame-to-frame difference (of 255): median, 95th percentile, worst | 12.4, 21.3, 26.0 | 12.3, 20.6, 25.6 |
| Frames differing from the last by over 19 | 112 | 83 |

The frames still over 30% are bushes 2.5 to 4 m off beside the route, which is scenery, softened by the fade's edge.

Cost, on the walk bench's quietest runs on the RTX 3080 (other sessions shared the GPU; main benched alternately with it): the GPU's frame 10.4 ms on main, 11.0 ms with this. Run with no subject, so nothing is cleared, it is 10.56 ms; with the fade computed but nothing discarded, 10.55 ms. So pushing plants and the shared buffer cost about 0.1 ms, the fade's sums nothing measurable, and the remaining half millisecond is the deeper forest drawn where near leaves no longer cover it.

## Second pass: a boom that stops at trunks, and plants that fade whole (D274)

2026-10-14. The next review of the film (6/10) found what the first pass made: a screen-door stipple where foliage covering 30 to 50% of the frame sat partly faded, smeared by TAA as the camera moved; a dark round tunnel carved through a hazel round the player, its dark inside showing through the tube's cut; and the camera, at the look-up, sinking into the player's shoulder and into a trunk, since it met only build walls and the ground.

What shipped games do, and what we did:

- **A camera boom swept as a sphere** (Unreal's `USpringArmComponent` with its probe, and most third-person games): the boom pulls in before what is solid, fast in and slow out (Unreal's arm lags out, not in). We sweep a 0.3 m sphere (the near plane's corners are 0.21 m from the lens) and a 0.6 m one against the fitted solids that stop the walker (D267) by sphere tracing their pieces as columns; the boom eases in to the wide sweep and is never let past the narrow one. A spruce's skirt (D248) stops the walker but is boughs, so the boom passes into it. Hemmed in behind the right shoulder, the boom swings to the left, as shooters swap shoulders. Looking up, the boom shortens over the ground behind no shorter than 3 m and then slides up over it, rather than coming in to the shoulder.
- **Whole-plant obstruction fade** (Unreal's camera-obstruction fades work per actor; The Witcher 3 dissolves whole bushes between the camera and Geralt): each shrub and young tree has a box, its parts' bounds; while the line from the eye to the figure's chest passes through 0.8 of it, every part of it fades together over 0.3 s, through a per-instance value the shaders read (Bevy's `MeshTag`), written only when it changes. Faded to a sparse dither (a fifth, then a twelfth left), it stippled half the screen when the camera stood in a hazel, and TAA smeared the dots into streaks as it moved; faded out entirely, it is clean, its shadow kept. So it goes all the way.
- **A short band at the lens**: leaves are gone within 1.05 m of the lens and whole from 1.4 m, from 1.65 to 3 m, so few are partly gone at once; a plant the camera stands in fades whole instead.
- **Plants lean, not part**: the push round a body reaches 0.3 m past its side, not 0.6, at 0.6 of its strength.

The dither itself was left: Jimenez's interleaved gradient noise shifted each frame (D214) is what Unreal and Call of Duty use for TAA, and the fade's value is not quantised coarsely (256 steps; Bevy's own crossfades 16). What showed was a partial fade held over a large area, which no pattern resolves under motion.

Measured on the film (80 frames, every 0.5 s), this against main filmed at the same time (08d4a78, D272's denser leaves in both). "Stipple" is the share of 16-pixel blocks at 1080p that are leaf-green and whose pixel-scale energy (|I − 3×3 mean|) is high and above the 2-to-6-pixel scale's: a dither's dots, which leaves' own detail and the ground's are not. Near-lens foliage is as in the table above. Inside a trunk and near the figure come from the camera's place each frame (2400 frames), against the fitted solids and the line up the figure's middle.

| | main | this change |
|---|---|---|
| Frames with stipple over 5% of the screen | 3 | 0 |
| Frames with stipple over 2% | 11 | 7 |
| Worst stipple, of the screen | 13.5% (young spruce at the lens) | 3.8% (a bush fading out) |
| Frames where near-lens foliage covers over 30% | 5 | 1 |
| Mean near-lens share | 11.5% | 9.4% |
| Frames with the lens inside a trunk or rock (of 2400) | 91 | 0 |
| ... of them in the look-up (of 261) | 44 | 0 |
| Nearest the lens comes to a trunk's bark | inside | 0.30 m |
| Nearest the lens comes to the figure's middle line | 0.99 m | 0.55 m |

The figure is nearer in metres but smaller in the picture: main's lens came in along the boom to hip height a metre behind and looked up past the whole torso, this one stays 3 m back along the boom and slides up, so at the top of the look-up only the head shows at the frame's foot. The frames left over 2% are bushes in their 0.3 s fade.

## Third pass: a boom that does not pump among trunks (D285)

2026-10-14. The next review (6.0/10) put the camera first: among pines and veteran oaks the boom collapsed to the player's skull in 0.2 s, sprang out and slammed in again, about eight times in twenty seconds, and once the player left the frame. The cause: D274 swept the boom against every fitted solid, so each trunk crossing the line from the shoulder to the lens, which in a dense stand is one a second, pulled it in at once; the shoulder swap threw the framing sideways as well; and a tree's fitted solid was a column as wide as its roots spread (D267 fits the wood up to 3 m, root flare and all, with no top), standing round the lens metres from the bark.

What shipped cameras do:

- **Collision and occlusion are separate.** Unity's Cinemachine Deoccluder keeps the camera out of colliders by a camera radius, and answers the line to the target being blocked only once it has been blocked for a minimum occlusion time, moving at its own "damping when occluded" and returning at another; raising the minimum occlusion time is the usual answer to the camera jumping forward for tree trunks (P, [Deoccluder manual](https://docs.unity3d.com/Packages/com.unity.cinemachine@3.0/manual/CinemachineDeoccluder.html); C, [Unity forum](https://discussions.unity.com/t/pull-camera-forward-and-player-occlusion/761274)).
- **Thin things do not pull the boom.** Unreal's spring arm sweeps its probe on one collision channel (`ProbeChannel`, the camera channel), so what should not pull it in, foliage and often trees, is set to ignore that channel (P, [USpringArmComponent](https://docs.unrealengine.com/4.27/en-US/API/Runtime/Engine/GameFramework/USpringArmComponent/); C, [a forum fix moving the probe off the camera channel](https://answers.unrealengine.com/questions/900797/view.html)). A trunk is then let hide the character for the moment it crosses.
- **Lag out, not in.** The arm comes in at once before a wall and lags back out.

What we did (`play/camera.rs`):

- The boom is swept only against walls, the ground and what is broad (`Solid::breadth` 2.5 m or more and not a tree: a boulder). Trees and small things do not pull it in.
- The lens is a 0.3 m sphere that never goes into any solid: from where it was it moves to the boom's end by collide-and-slide, round a trunk in its way (met head on, round the side the end is on), and comes back to the end at a rate of 2 a second once past. Slid aside, it turns to look at the point it would have looked through from the boom's end, so the player stays where they were in the picture.
- A trunk that hides the chest and head from the boom's end for 0.6 s has the boom ease in front of it (2.5 a second, no shorter than 2 m), until the player has been in sight from the end for 0.5 s.
- No shoulder swap: the boom stays over the right shoulder.
- A tree's own solid, what `Solid::clearance` keeps the lens out of, is its bole, its wood taller than a body; its root flare is added as extra pieces, as a spruce's skirt is, so it still stops a walker (`footing::trunk_solid`). `collide` reads the same on the trees as before.

Measured on both films (the standard `cargo xtask film --time 15` and the pines route), 2400 frames each from the camera's place every frame, logged by a throwaway patch, against main (eda9605) run the same way. A punch-in is the lens-to-head distance falling by over 30% within 0.3 s, the look-up (where the ground behind rightly brings the boom in, as D274) counted apart. In the picture means the point is inside the 55-degree, 16:9 frustum. Inside bark is measured two ways: against the run's own fitted solids, and against the meshes as drawn between 1.5 and 3 m up, the lens's own heights (`lush.footprint` with its band moved, uncommitted).

| | main, film | this, film | main, pines | this, pines |
|---|---|---|---|---|
| Punch-ins (look-up apart) | 10 | 0 | 12 | 0 |
| Lens-to-head distance: least, 5th percentile, median (m) | 0.55, 0.75, 4.53 | 1.89, 2.30, 4.67 | 0.61, 0.86, 4.40 | 3.69, 4.37, 4.67 |
| Fastest the lens comes in (m/s) | 224 | 16 (the look-up) | 227 | 6 |
| Framing jumps (chest over 1/8 of the width in 0.3 s) | 17 | 0 | 12 | 0 |
| Frames with the chest out of the picture (look-up apart) | 255 | 0 | 194 | 0 |
| ... head, chest and feet all out | 58 | 0 | 96 | 0 |
| Frames the player is hidden behind a solid; longest | 28; 0.27 s | 88; 0.95 s | 0 | 63; 1.05 s |
| Lens inside a fitted solid; nearer than 0.2 m | 0; 144 | 0; 0 | 0; 53 | 0; 0 |
| Lens over a tree's wood as drawn at 1.5 to 3 m | 0 | 0 | 0 | 0 |

Main's hiding counts are against its wide root columns and its fitted-solid distances are nearer than the bark; this one's against boles. The cost is trunks crossing the line: the player is hidden behind one for a second at most, and a trunk the lens slides round sweeps across a third of the picture for half a second, smeared by TAA as it goes. Shipped games fade a trunk at the lens (a camera-distance dither in its material); bark has no such fade here yet (D271 fades leaves only).

Cost, on the walk bench beside main run alternately on the RTX 3080 (load 4 to 5, the GPU otherwise quiet): median 15.06 and 15.01 ms against 14.94 and 15.00, the 99th percentile 17.85 and 17.87 against 17.96, the game's CPU frame 4.27 to 4.47 ms against 4.35 to 4.36, the GPU's 12.40 to 12.48 against 12.34. Nothing measurable; both miss the budget at the 99th percentile, as main already did.
