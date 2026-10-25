# Loadouts and animation layers: how shipped games put items in two hands and move every body with them

2026-10-25. The question: when a body can hold one item, two, a two-handed one or nothing, in either hand, how do shipped games and engines decide what it looks like and which animations play, without a set of clips for every pair? And how do they keep many takes of a move from looking canned? It informs [docs/holding.md](../docs/holding.md). The developer, 2026-10-25: "we have various weapons, two arms, sometimes two handed items, sometimes two one handed items, sometimes one one handed item... and then how do animations work? what happens with variance?"

What was read this session is marked *read*. What comes from earlier knowledge of a game or engine, not re-checked against a source today, is marked *recalled*. Treat a recalled point as a lead, not a fact.

## Answer

Every game looked at does the same three things:

- **A small graph shared by every body.** Locomotion is shared; the item chooses an upper-body layer and a move set.
- **A move set per weapon class, shared by every item of that class.** A new item is a model, a few grip points and numbers, never clips.
- **Hands by rule.** What the pair of hands holds chooses the stance. A two-handed item puts the off-hand item away. A shield is an off-hand item with its own actions.

Left-handed bodies are rare in shipped action games. Where mirroring is used, it is a table of bone pairs applied to the pose. Variety comes from a few takes picked without repeats, small additive layers, and play rate. Motion Matching is the exception: there, variety comes from a large database.

## Engines

| Engine or game | What it does | Source |
|---|---|---|
| **Unreal, Lyra** | One main Animation Blueprint (`AnimBP_Mannequin_Base`) holds the graph: locomotion states, aiming, skeletal controls. The places a weapon may override are an Animation Layer Interface (`ALI_ItemAnimLayers`). Each weapon has a child of one base layer Blueprint (`ABP_ItemAnimLayersBase`, for example `ABP_PistolAnimLayers`) that only slots in its clips and a few variables. The layer reads the main graph's speed and acceleration rather than working them out again. Starts use distance matching, then stride warping; orientation warping covers the angles between its four strafe directions. *Read* | [Animation in Lyra](https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-in-lyra-sample-game-in-unreal-engine) |
| Unreal, Lyra, choosing the layer | The weapon instance holds an equipped and an unequipped layer selection. Each is a list of (layer class, required gameplay tags) rules and a default, picked by the cosmetic tags of the character (`FLyraAnimLayerSelectionSet::SelectBestLayer`). `LinkAnimClassLayers` swaps the layers in on equip. *The API call read; the Lyra class names recalled from its source* | [LinkAnimClassLayers](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/UAnimInstance/LinkAnimClassLayers) |
| Unreal, Lyra, hand IK | The left hand is pulled onto the weapon by IK after the pose, so additive aim offsets do not drag it off the grip. The right-hand IK bone is there for retargeting only. The mannequin's IK bones (`ik_hand_gun`, `ik_hand_l`, `ik_hand_r`) carry the grip through clips. *Forum answer read; the bone names recalled* | [Epic forum: right-hand IK in Lyra](https://forums.unrealengine.com/t/whats-the-purpose-of-right-hand-ik-in-lyra/2743139) |
| **Unreal, mirroring** | A Mirror Data Table pairs left and right bones; the centre bones (pelvis, spine, neck, head) map to themselves. It also mirrors curves, notifies and sync markers. A Mirror node in the graph flips a pose on a boolean; a notify can ask whether it came from a mirrored clip. *Read* | [Mirroring animation](https://dev.epicgames.com/documentation/en-us/unreal-engine/mirroring-animation-in-unreal-engine), [UMirrorDataTable](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/UMirrorDataTable) |
| Unreal, variety | The Random Sequence Player has a shuffle mode: every entry plays once before any repeats, and the per-entry weights are then ignored. It was buggy in 4.x (UE-28987). *Read* | [bShuffleMode](https://docs.unrealengine.com/4.26/en-US/API/Runtime/AnimGraphRuntime/AnimNodes/FAnimNode_RandomPlayer/bShuffleMode/index.html), [UE-28987](https://issues.unrealengine.com/issue/UE-28987) |
| Unreal, Motion Matching | The pose search keeps a history of recent poses (`PoseSearchHistoryCollector`). Variety comes from the size of the database plus a cost that biases the search away from what was just played. *History API read; the anti-repeat bias recalled* | [PoseSearchHistory](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/PoseSearchHistory) |
| **Godot 4.7, AnimationTree** | Blend2 and Blend3 take track filters, which is how an upper-body layer is laid over locomotion. OneShot plays a move and returns, with its own fades and filter. BlendSpace1D and 2D, a StateMachine, Transition. No mirroring is built in. *Read* | [Using AnimationTree](https://docs.godotengine.org/en/stable/tutorials/animation/animation_tree.html) |
| **Bevy 0.20** (ours) | `AnimationNodeType` has Clip, Blend and Add nodes. Every node takes a mask, a bitfield of mask groups (sets of bones) it and its children leave alone. Masks were added for "characters holding objects". No mirroring. *Read in the pinned source,* `bevy_animation-0.20.0/src/graph.rs` | [Bevy issue 14395](https://github.com/bevyengine/bevy/issues/14395) |

## Games

| Game | Hands and stances | Moves and timing | Source |
|---|---|---|---|
| **Elden Ring and Dark Souls** | A right and a left slot of weapons each; either hand can hold most one-handers. Two-handing either hand's weapon is a button, and the other hand's item goes on the back. Two of the same class, one in each hand, give a power stance: a dual moveset on the left attack button, the standard buttons still attacking with the right hand alone, and no block. Claws, fists and twinblades come paired; some pairs across classes work too. A shield's left button blocks; its strong left button parries. A left-hand weapon's light attack is the right-hand light mirrored (DS3). *Read in guides; mechanics agree across sources* | A moveset is per weapon class, shared by its weapons, with some uniques. Two-handing gives the class's own two-handed moveset. A weapon's skill (Ash of War) adds one move, not a set. *Read in part (the greatsword class page); the rest recalled* | [Push Square](https://www.pushsquare.com/guides/elden-ring-how-to-two-hand-and-dual-wield-power-stance-weapons), [Windows Central](https://www.windowscentral.com/elden-ring-how-power-stance-weapons), [Greatswords](https://eldenring.wiki.gg/wiki/Greatswords), [DS3 Scimitar](https://darksouls.fandom.com/wiki/Scimitar_(Dark_Souls_III)) |
| **Enshrouded** | Classes: one-handed (sword, axe, club, dagger), shields, two-handed (greatsword, two-handed axe, hammer), bows, wands, staves. A one-hander can go with a shield; blocking needs a shield or a two-handed weapon. Whether two one-handers can be held at once is unclear across sources (an early-access article says one item at a time). *Read in guides* | A chain per class; a two-handed chain ends in a whirlwind. *Read in guides* | [pindrop.gg](https://pindrop.gg/enshrouded/weapons), [Game Watcher](https://www.gamewatcher.com/enshrouded-weapons-list), [research/enshrouded-combat.md](enshrouded-combat.md) |
| **Mordhau** | One- and two-handers, shields separate. Many weapons have an alt mode: a change of grip (a two-handed axe held long or choked up) or a throw. Most one-handers cannot change grip, so their alt is a throw. *Read* | An attack is a windup, a release and an afterblow (recovery), with a combo window. Each weapon is numbers on its class's swings: length, windup and release times, turn cap, combo or not. Directional swings come from the mouse. *Phases and stat names read; per-weapon times recalled* | [Magic Game World](https://magicgameworld.com/mordhau-detailed-weapon-stats-guide), [Mordhau wiki: Combat](https://mordhau.fandom.com/wiki/Combat?oldid=3196), [Steam](https://steamcommunity.com/app/629760/discussions/0/2530372519561512660) |
| **Chivalry 2** | Weapon classes share swing sets; swing direction comes from keys. *Recalled; not found today* | — | [Steam](https://steamcommunity.com/app/1824220/discussions/0/4339860800344891970) |

## Variety

| Technique | What it varies | Cost | Where |
|---|---|---|---|
| A few takes a move, picked without repeats (shuffle bag) | The whole motion | One clip a take | Unreal's Random Sequence Player (*read*); every action game's hit reactions and idles (*recalled*) |
| An additive layer on the pose | Small offsets: breath, sway, a lean, a flinch | One small clip, any body | Godot Add2, Bevy Add, Unreal Apply Additive (*read*). Additive keeps only small differences; a different stance does not work additively ([Iridar](https://www.patreon.com/Iridar/posts/mod-tech-69172597), *read*) |
| Play rate by a rule | Tempo by size (Froude, [creature-rigs.md](../docs/creature-rigs.md)) or by the weight of what is held | None | Mordhau's numbers on shared swings (*recalled*); our troll at Froude speed ([troll-fight.md](../docs/troll-fight.md)) |
| Motion Matching | Everything, from a large database | Minutes of motion a body and a style, and a search a body each frame | Unreal Pose Search (*read*), Simon Clavet, GDC 2016 ("Motion Matching and The Road to Next-Gen Animation", *recalled*) |

## What this means for us

- **The pattern to follow is Lyra's.** One graph for every body; a class-and-stance layer that only supplies clips and numbers; a selection by rule, not code. That matches D136 (kinds as data) and movement.md's pose layer.
- **The rules to follow are FromSoftware's**:
  - a set per class, shared by its items;
  - stance from the pair in the hands;
  - a two-hander puts the off item away;
  - the left button is the off hand's: block with a shield, or the off blade's light;
  - the same class in both hands gives a dual set.
- **Bevy has the graph's pieces** (blend, add, masks) **but no mirroring.** Unreal's mirror table is the reference. It is documentation only, so we build it.
- **Variety is cheap with takes and additive noise.** Motion Matching stays the later option movement.md already gives it.

## Limits

- Lyra's selection-set class names and its IK bone use are recalled, not re-read; its source sits behind Epic's GitHub access.
- Elden Ring, Enshrouded and Mordhau details come from guides and wikis, not from the developers. Mordhau's per-weapon times could not be read: its wiki refused the fetch.
- No game was measured: no clip counts and no timings by weight.
- Whether Enshrouded lets you hold two one-handers is unsettled.
- No source was found on how any shipped game handles a left-handed player body. That it is rare is an inference, from none found.
