# Item pictures

2026-10-12. Question: how do games make the small pictures of items shown in the action bar, the backpack, the equipment slots, storage and the station screens, and which way keeps one style across a procedural number of items and variants (D127)?

## What games do

From general knowledge of the games and their tools, checked against a few sources; nothing was measured.

| Way | Who | Holds a style? | Scales to procedural variants? |
|---|---|---|---|
| **Painted by hand**, one picture per item | Most older RPGs; Valheim and Enshrouded's look | Yes, if one artist paints them all | No: every new item or colour is new work |
| **Rendered from the model ahead of time**, by a batch tool, often with a paint-over or filter pass | Common in studios: Blender add-ons such as IconSet Generator and Poly Icon Maker, Unity's Icon Creator, Rebellion's pipeline that feeds render passes into a Substance graph | Yes: one camera, one light rig, one post pass | Only for what exists when the batch runs |
| **Rendered by the game at run time and kept** | Escape from Tarkov draws a gun with whatever is fitted to it and keeps the picture in an icon cache on disk; Minecraft draws block items in 3D in the inventory, each model saying how it sits in a slot (its `gui` display transform); Rust makes a skin's picture when the skin is published | Yes: the same rig every time | Yes: a picture is made the first time a combination is shown |
| **Drawn by an image model** | Asset stores and AI tools offer it | Weakly: each picture is a new roll, and drift across hundreds is the norm | No: it does not know the item's real shape or colours, so a variant's picture would not match the thing in your hand |
| **A base picture and a colour or frame** | Diablo-likes with generated loot: one picture per base kind, a border colour for rarity | Yes | Only for what a frame or colour can say |

## What fits Terra

Our items already have what a renderer needs. Each item's look is a file found by its name (`models/held/<item>.glb`, `models/gear/<body>/<item>.glb`); its materials are named by role, and a `Tint` recolours a look by role at run time (commit 52e7d42). A picture drawn from that look, with that tint, is right for any variant without anyone painting it, and it shows the player the real thing, which suits D93 and D122: what you see in the slot is what you are carrying.

So the way that fits is the third one, Tarkov's and Minecraft's: **the game draws each picture from the item's own look**, through one fixed camera and light rig, and keeps it.

- **Keyed by what changes the picture**: the look, the tint, and any state shown (broken, how many arrows in a quiver). The same key always gives the same picture, so a picture is drawn once and reused by every slot that shows it (D127).
- **Drawn when first needed**, a few per frame, into an atlas, with the picture before it is ready being the item's name, as today. Old pictures are dropped when the atlas is full.
- **Framed by the item's size in cells** (D84): a two-by-one axe gets a picture twice as wide as it is tall, as Tarkov's grid does.
- **Each look may say how it sits**, as Minecraft's display transforms and our looks' carry tilt in glTF extras do: an angle and a zoom, defaulting to a framing worked out from its bounds. Most items need nothing.
- **Clothing has no body in it.** A coat fitted to a body floats in its shape when drawn alone, as worn gear is in many games. Whether that reads well is to be seen.
- **The same renderer writes pictures to files**, for a contact sheet the workbench can make of every item and tint (D69), and for anything outside the game that wants them.

Bevy 0.19 ships what this needs: a camera can draw to an image (`examples/3d/render_to_texture.rs`), and render layers keep the picture's scene apart from the world.

An image model is still useful for the **style**: drawing a few pictures by hand or by model as targets, then tuning the rig and the post pass until the renders match them.

## Limits

- Nothing was built or measured. How many pictures a frame can draw on an integrated GPU, and how big the atlas should be, are unknown.
- Whether our painted look (D110's mood board, style C) survives at 64 pixels without an outline or a stronger light is untested; the world has no outlines (art.md), and the developers want the pictures in the same style (D130), so a stronger rim light is the first thing to try if they do not read.
- The Tarkov icon cache is known from players clearing it and from a fan tool that reads it, not from its developers.
- Pictures for things that are not models, such as a skill or an effect, are not covered.

Sources: [IconSet Generator](https://superhivemarket.com/products/iconset-generator/docs), [Poly Icon Maker](https://playgroundhub.gumroad.com/l/Poly-Icon-Maker), [Unity Icon Creator](https://assetstore.unity.com/packages/tools/game-toolkits/icon-creator-generate-fast-easy-complete-icons-generator-198488), [Rebellion's icon pipeline](https://scottwoo.artstation.com/projects/aoe2yL), [Tarkov's icon cache folder](https://devtrackers.gg/escapefromtarkov/p/ea8082a6-goodbye-tarkov-after-my-post-yesterday-about-having-trader-flea-problems-now-i-can-t-get-past-profile-data-loading), [a tool that reads it](https://github.com/the-hideout/tarkov-image-generator), [Minecraft's display transforms](https://docs.minecraftforge.net/en/1.21.x/rendering/modelloaders/transform/), [Rust's workshop pictures](https://rust.facepunch.com/blog/devblog-132).
