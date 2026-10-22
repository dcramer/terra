# Survival

What a player carries, makes and needs, in Bevy (D407): the first crate of the gameplay layer (D198), which creatures, townsfolk and building play stand on.

- **What a player carries** ([docs/player.md](../../docs/player.md)): the kinds of item and their rules (`kinds`, `registries`), the backpack, belt and gear (`inventory`) and its screen (`bag`), what is held and worn on the figure (`held`, `worn`), wear and breaking (`wear`), what does not fit dropped to the ground (`pile`), storage set down in a base and its screen (`storage`, `storage_screen`), and the item icons baked from the models (`icons`).
- **What a player makes** ([docs/building.md](../../docs/building.md)): the catalogue of what can be set down (`catalogue`), props, their models and the contract each keeps (`props`, `contract`), crafting at stations and its screen (`crafting`, `station_screen`), claims (`claim`), harvesting, digging and what takes time to do (`harvest`, `dig`, `work`), the build menu's and stations' tiles (`tiles`), what comes in, shown at the side (`gains`), and skills (`skills`).
- **What a player needs**: health and food (`vitals`), body heat (`warmth`), going down and coming back (`death`), what a player is told (`told`), what a fire or a dragon is to what meets it (`hazards`), a torch's light (`torchlight`), interacting with what is near (`interact`), and the noise things make (`noise`).

It stands on the walker (`terra_walker`), the base on screen (`terra_bases`), the world (`terra_world`) and the plain crates; nothing here names creatures, townsfolk, building play or the app, and Cargo holds that. `village_kinds`, the settlement's palette as the kits on disk name it, is here because building play and the townsfolk both ask it and this is the lowest crate both stand on. Its tests need no GPU:

```bash
cargo test -p terra_survival
```

The game's `GameplayPlugin` adds each module's plugin and re-exports the modules at their old paths (`terra_game::play::inventory`, `terra_game::icons`, ...). The model contract (`cargo xtask contract`) and the icons test run here, over every model and item. The `testing` feature gives the crates above `test_world`, `test_player` and `inventory::undress`, for their tests.
