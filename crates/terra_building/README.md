# Building

Building play, in Bevy (D407): the gameplay crate on survival, beside creatures and townsfolk.

- **Acting on a base** ([docs/building-plan.md](../../docs/building-plan.md)): every change the hammer, a script or the remote makes to a base is one of `terra_build`'s operations through `act::Building`, which checks it, settles what it costs from the pack and the claim's storage, and keeps it to be undone (`act`); what building costs and how each kind goes in, from the kits the base read (`act::BuildRules`).
- **The hammer and its screens**: aiming, previewing and acting with the tool in hand (`tool`), the build menu (`build_menu`) and the dial (`dial`).
- **What follows from an act**: doors and shutters opening and shutting (`props`), what comes down, its fragments, dust and heaps (`loss`, D305), the base's fire (`fire`, D377), and a house going up as a film (`going_up`).
- **Scenes and the remote**: the kit sheet and villages laid as scenes (`sheet`, `village`), and the remote's `build.*` and `fire.*` methods (`remote`, `fire`).

`building::plugin` adds it over the base on screen (`terra_bases`), without a player, as the sandbox's `build` runs it; `building::player_plugin` adds acting on the base where a player builds. It stands on survival (`terra_survival`), the walker (`terra_walker`), the base on screen, the world (`terra_world`) and the plain building crates; nothing here names the creatures, the townsfolk or the app, and Cargo holds that. What it reads of the layers below that may be missing it asks for as D198 says. Its tests need no GPU:

```bash
cargo test -p terra_building
```

The game's `GameplayPlugin` adds its plugins and re-exports the modules it still names at their old paths (`build::building`, `build::act`).
