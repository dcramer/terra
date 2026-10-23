# Campfire as one body of flame: how shipped games lay it on the logs

2026-10-23. The question: our blind reviewers say the campfire is "a cluster of separate cards, not one rolling body", and that it covers only the middle of the logs. How do RDR2, The Witcher 3, Horizon and Unreal's Niagara make a campfire read as one body that follows its fuel?

## Answer

No studio has published a breakdown of the RDR2, Witcher 3 or Horizon campfire. A graphics study of RDR2 shows only that its campfire light casts point-light shadows (imgeself, "Graphics study: RDR2", 2020). What is published is the general practice for flipbook fire. Five points apply to us:

- **The flame is laid on the fuel.** Niagara places particles on a mesh's surface, area-weighted, with its Static Mesh data interface ("Static Mesh Location"; Epic, NiagaraDataInterfaceStaticMesh). Flipbook campfire breakdowns on RealTimeVFX lay a card every log's width or so along each log's top, overlapping, tallest where the logs meet. A fire born at the middle reads as a cluster in the middle.
- **Whole-fire flipbooks, layered at offset phases.** The sheets are whole sims, such as EmberGen or Houdini Pyro renders, as ours are (books.rs). Each card runs at its own start frame, with its own mirror and rate. Frames are blended along baked motion vectors, as SideFX Labs' Flipbook Textures ROP bakes them, so no card steps. Ours already does all of this.
- **Velocity alignment is for sparks.** Niagara's Velocity Aligned facing stretches a sprite along its flight. Flames stay upright and camera-facing, anchored at the foot. This is convention: no source was found.
- **Soft particles.** Depth fade over about a log's radius sinks a card's foot into the logs and the ground (Unreal's DepthFade expression). Ours is `SOFT_M`, faded only very near for a hearth's cards.
- **Colour from a temperature ramp, white only in a small core.** A ramp too narrow for the heat it maps clips to white (SideFX forum). Where flames overlap, their heats sum, and the sum shows white. Our noon pale was that sum behind the logs (fire body round).

## What we do

The hearth tells the shader where its logs lie: how many, their bearing in the world, their feet and tops, and their thickness (`fire/hearth.rs` `Logs`, held against `tools/props/hearth.ts` by its test). The tongues take the logs in turn, each life born somewhere along its log, tallest at the top. The side columns each stand over a log of their own. Each log burns at its own slowly changing heat, which sets its tongues' height and lean, and a glow lies along each log's underside and over the coals, drawn as flame. Cooling the side columns and the low tongues instead read the body pale and pink to a blind judge (4.2 against main's 4.9): a thinner flame let the coals behind it through, tonemapped as the world, salmon.

Sources: dev.epicgames.com (NiagaraDataInterfaceStaticMesh, ENiagaraSpriteAlignment, DepthFade), sidefx.com (Labs Flipbook Textures), realtimevfx.com (motion-vector flipbooks; simple fire shader breakdown), simonschreibt.de (Battlefront II layered explosion), imgeself.github.io (RDR2 graphics study).
