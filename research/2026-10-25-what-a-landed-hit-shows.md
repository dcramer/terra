# What a landed hit shows: the contact-point burst, not a whole-body flash

2026-10-25, combat round 17. Question: what do shipped third-person melee games show when a blow lands, in place of or besides a flash of the whole struck body: a burst at the contact point, a flinch layered on the struck body, the camera's kick, a sound, a short hit-stop? Which combination is best supported, what does our reference (Enshrouded, D180) do, and what does each cost us? The developer, on the duel film with D456's whole-body flash (combat round 12): "idk that the flash is good". Proposal: D487.

## Answer

None of the 3D melee games looked at lights the whole struck body. Every one marks the hit **where the blade met the body**: a burst of light, sparks or blood at the contact, sized to the blow, gone in a few frames, with a sound, a kick of the camera and a short hit-stop. The struck body answers by **moving** (a flinch), not by changing colour. A whole-body flash is a 2D and top-down convention (Hades, Hollow Knight, Zelda's red flash), where the body is a few pixels and a contact point would be lost.

**Our reference is the same.** Re-read frame by frame (Update 8's warp comparison, `ftp` 247.68 to 247.78 s at 60, pictures `~/.cache/terra-review/refs/enshrouded-combat/ftp/warp/f_0247.*.jpg`): the "3 to 4 frame flash" measured in round 3 (`tools/combat/refs/enshrouded.json`, `hit_stop.flash_frames_at_60`) is a white-hot glow **at the contact**, a hand's width across, with a dozen thin yellow spark streaks flung out of it (247.717-247.767), then an orange half-ring spreading from the same point as the streaks fly on (247.767-247.783), and a damage number. The dummy's fur and the shield keep their colours on every frame; only what is right by the contact is lit. D456 read "a short flash on what was struck" from it; it is a flash **at** what was struck. Round 12 built it as the whole body lit, which is what the developer is now unsure of.

## What each game shows

| Game | At the contact | The struck body | Camera | Hit-stop | Confidence |
|---|---|---|---|---|---|
| Enshrouded (Update 8) | white glow, spark streaks, orange half-ring, damage number | no colour change; the dummy rocks | kick, about 0.1 s | none measured (0 to 0.05 s) | measured, one hit on a dummy |
| Elden Ring, Dark Souls | blood spray on flesh, sparks on stone and metal, along the cut | a flinch when its poise breaks, otherwise none | little or none on a light hit | a few frames, longer on a heavy | recalled from play and footage |
| Sekiro | blood on a hit, a big spark burst on a deflect | flinch, posture bar | small kick on a deflect | a few frames on a deflect | recalled |
| Monster Hunter World | hit spark at the contact, coloured by how well it hit; damage number | flinch only on a part break or a stagger threshold | small shake on a heavy | strong, per weapon; World's thought too strong by its players and cut in Wilds' beta ([MP1st](https://mp1st.com/news/monster-hunter-wilds-hitstops-final-game-closer-to-people-are-looking-forward)) | recalled; hit-stop sourced |
| God of War (2018) | blood and impact sparks at the axe's contact | additive flinch, knock-back by weight | shake and a kick on heavies, controller rumble | a short freeze ([GDC 2019, Sheth](https://gdconf.com/news/see-how-god-wars-combat-evolved-match-its-new-camera-system-gdc)) | recalled; the talk's slides not read |

The weighting from a study of impact feel ([Lin et al., arXiv 2208.06155](https://arxiv.org/abs/2208.06155v1)): of 19 features, hit-stop, the sound fitting the hit, and the camera weigh most; a visual effect at the contact is one of the rest, and a whole-body flash is not among the strongest.

How engines build it: Unreal spawns a Niagara impact system at the hit's location, turned to the hit's direction (`SpawnSystemAtLocation`, [docs](https://dev.epicgames.com/documentation/unreal-engine/BlueprintAPI/Niagara/SpawnSystematLocation)); a flinch is an additive pose applied over whatever the body plays (`Apply Additive`, [blend nodes](https://dev.epicgames.com/documentation/unreal-engine/animation-blueprint-blend-nodes-in-unreal-engine)), or a layered blend per bone on the upper body.

## The best-supported combination, and what it costs us

1. **A burst at the contact point**, along the blade's travel: every game, and our reference. Costs us least: the blade sweep already finds the contact (`terra_motion::blade::sweep`) and the blade's way (last tick's edge to this one's); a dozen unlit additive sprites and a short point light, gone in about 0.15 s, as the staged impact's sprites are drawn (`terra_creatures::staged::impact`). Built this round (D487): a glow 3 frames at 60 (a white core in a warm halo, 0.4 m across, drawn 0.3 m toward the view so the body does not hide it) and 20 sparks.
2. **Keep the kick and the short hit-stop** (D456): every game has both; ours are already within Enshrouded's measure.
3. **The sound**: already chosen by what the blade met (`Cue::Struck`, docs/audio.md); the study weighs it among the strongest, so its fit is worth a listening pass, not new code.
4. **A flinch on the struck body**: every game but Enshrouded's dummy. We have one already, played as a whole clip (`Reel::Flinch` plays `Hit_Chest`), which cuts whatever the body was doing; an additive flinch over the current move (Unreal's Apply Additive) would let a blow land without stopping the other's. Costs a clip made additive and a layer in the animation graph (terra_walker's figure), more than a round; next, if the developer wants it.
5. **Drop the whole-body flash**: no 3D melee game here has it, and our reference doesn't. It is a number now (`landed.flash`, 0 to 1), so it can go without code.

Not tried: blood. How much blood the game shows is the developers' call; sparks and light read on a training partner and on armour alike.
