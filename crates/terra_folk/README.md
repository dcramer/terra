# Folk

Townsfolk, in Bevy (D407): the gameplay crate on creatures, which building play stands beside.

- **People** ([docs/people.md](../../docs/people.md)): figures in the world who are not the player, on the base body, with a role and a few states (`people`); the first quest's settlement, its elder and villagers (`settlement`), and the place where magic pools, where spiders form and the smith kneels bound (`pooling`).
- **A living town** ([docs/demos.md](../../docs/demos.md)): a market town asked for at a place, laid out by `terra_town` and put up by the shortcuts a player's base is, its people living their days (`town`); its people's bodies dressed by role from `assets/data/townsfolk.ron` (`townsfolk`), its animals (`town_animals`), fences (`town_fences`), the goods on its stalls (`stall_goods`), the ground of its gardens (`town_gardens`), and its murmur and what its people do that is heard (`town_sound`).
- **A town rising** (D381): a settlement going up as a time-lapse from `assets/data/timelapse/` (`rising`), with the scaffold round each building as its walls rise (`scaffold`).

It stands on creatures (`terra_creatures`: a town's animals are tame creatures, its lairs and perches wildlife's), survival (`terra_survival`), the walker (`terra_walker`), the base on screen (`terra_bases`), the world (`terra_world`), the town's layout and life (`terra_town`) and the plain crates; nothing here names building play or the app, and Cargo holds that. Its tests need no GPU:

```bash
cargo test -p terra_folk
```

The game's `GameplayPlugin` adds each module's plugin and re-exports the modules at their old paths (`play::town`, `play::people`, ...). Until building play is a crate, the game's seams test (`crates/terra_game/src/seams.rs`) holds that it names nothing here. A town's people's day is checked on the CPU by `cargo xtask life` (terra_town's).
