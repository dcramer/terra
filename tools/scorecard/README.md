# Scorecard

The ratchet every area's scorecard shares (D452, D457): `ratchet.ts` judges a criterion's number by its miss (how far outside its bar, so worse is always more), holds it to a baseline, prints a line a criterion and the summary, and raises (`--raise "<why>"`) or lowers (`--lower`, nightly's alone, D414) the baseline. Its users measure their own numbers and call `finish`: [tools/combat](../combat/README.md) (`cargo xtask combat`) and [tools/controller](../controller/README.md) (`cargo xtask controller`). Its pure parts are tested through combat's `scorecard.test.ts`.
