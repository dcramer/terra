# Creatures

Creatures and fighting, in Bevy (D407): the gameplay crate on survival that the townsfolk stand on, and that building play stands beside.

- **Creatures** ([docs/creatures.md](../../docs/creatures.md)): the kinds of creature read from `assets/data/creatures.ron` with the registries (`species`), the creatures themselves, their minds, gaits and how they look (`creatures`), the wildlife put out round each player and taken away once all have gone (`wildlife`), what a film sees of it (`sighted`), coats moulted with the seasons (`coats`), and fliers on a path, from `assets/data/fliers.ron` (`fliers`).
- **Fighting** ([docs/combat.md](../../docs/combat.md)): both sides of a fight together: where on a creature a blow lands (`zones`), swinging and being bitten (`combat`), the bow (`archery`), how a blow is shown (`hits`), the sword-and-shield prototype on screen, its rules in `terra_motion` (`melee`), and staged figures told their set piece over the remote (`staged`).

It stands on survival (`terra_survival`), the walker (`terra_walker`), the base on screen (`terra_bases`), the world (`terra_world`) and the plain crates; nothing here names townsfolk, building play or the app, and Cargo holds that. Its tests need no GPU:

```bash
cargo test -p terra_creatures
```

The game's `GameplayPlugin` adds each module's plugin and re-exports the modules at their old paths (`play::creatures`, `play::species`, ...). Until building play is a crate, the game's seams test (`crates/terra_game/src/seams.rs`) holds that it names nothing here.
