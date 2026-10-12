---
name: tripo
description: Make, texture, rig, animate or export 3D models with Tripo on the team's Studio subscription, through tools/tripo/tripo.ts. Use when asked to generate a model, concept image or views, retexture, retopologise, segment, rig or animate a model, or check Tripo credits.
---

# Tripo

Everything Tripo does goes through `tools/tripo/tripo.ts`, the team's CLI for Tripo's Studio. It spends the subscription's credits. Read [tools/tripo/README.md](../../../tools/tripo/README.md) for every command, and run `node tools/tripo/tripo.ts <command> --help` for a command's options before using it.

- **Check the login and credits first** with `node tools/tripo/tripo.ts balance`. If it says `TRIPO_STUDIO_COOKIE` is missing or the login failed, stop and ask the developer to set or copy it again (README, Setup). Never ask for the cookie's value in chat.
- **Say what it will cost before spending much.** About 55 credits for a model at the app's defaults (25 turned down; README), 55 for a P1 Smart Mesh with its texture and 120 for P2, 20 for a rig or a texture, 40 for segmenting; images and preset animations are free. Several models at once, or anything over about 100 credits, needs the developer's go-ahead.
- **Chain by project.** Each command prints one line of JSON; take `project` from it for the next command. Commands on a project wait for anything already running on it.
- **Look at what you made.** Read the images you generate, and render or open models before calling them done (`docs/development.md`, Blender). A model for the game is judged against `docs/art.md`.
- **Rigs:** `--skeleton tripo` is the older V1 rig, which our skeleton converter takes and which rigs our bodies better (`tools/characters/make_character.sh`); the default, `unreal`, is the app's V3 rig with the only twist bones that carry skin, for outside tools. `--type other` rigs an animal or creature.
- **Drafts are build output; what is kept is source art.** Write drafts under an ignored folder such as `tools/characters/out/`. What the developers approve, and what the game's files are made from, goes in `art/` and is committed (`art/README.md`).
- **When a command breaks** because Tripo changed its requests, or to check the CLI still matches the app, follow [tools/tripo/MAINTAINING.md](../../../tools/tripo/MAINTAINING.md) and fix the CLI. Don't call Tripo's API or web backend some other way.
