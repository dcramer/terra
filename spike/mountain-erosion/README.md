# Spike: drainage erosion baked once per world

The question (research/2026-10-19-mountain-look.md, plan step 1): does stream-power erosion on a coarse grid, about 32 m cells over the 30 km world, baked once per world under our per-point gullies, give branching spurs on the settlement's wall in under 10 s, without pits that fill as lakes?

The answer goes in that note. `cargo run --release` (with `--target-dir ../../target`) samples the land round the settlement's site twice (the ground as it is, and without its per-point gullies), erodes the second, adds the gullies back, and writes into `out/` (ignored): the timings and measures, and a before/after sheet of the range seen from the settlement and from above.

The solver is Braun and Willett's implicit stream-power scheme (2013, "A very efficient O(n), implicit and parallel method to solve the stream power equation governing fluvial incision and landscape evolution", Geomorphology 180), routed over a Priority-Flood fill (Barnes, Lehman and Mulla 2014, our `terra_land::lakes`), with the erosion law and hillslope term of Schott et al. 2023 (https://github.com/H-Schott/StreamPowerErosion, MIT, Copyright (c) 2023 Schott).
