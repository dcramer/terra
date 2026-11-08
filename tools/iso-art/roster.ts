// A biome's roster names each foe's walk speed (forest.ron's `walk`, the speed a steered mob's walk
// clip plays at, kit/biome.rs): it must be its model's own, as `check` measures it on the GLB (the
// planted foot's slide, check.ts motion), or the feet slide on the ground. The rule changed under
// the rosters once (iso-forest-art j4, 2026-11-08: treant-a 1.06 -> 0.96, tree-monster 1.31 ->
// 1.27), unseen for a job; this says where a roster went stale.

import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { measure } from "./check.ts";
import { readGlb } from "./gltf.ts";

/** How far a roster's walk may be from the model's measured walk, m/s: the ledger's rounding. */
export const WALK_APART = 0.02;

/** Every `(model: "models/iso/NAME.glb", … walk: N` in a biome file, by line. */
export function rosterWalks(ron: string): { line: number; model: string; walk: number }[] {
  return ron.split("\n").flatMap((text, i) => {
    const m = /model: "models\/iso\/([a-z0-9-]+)\.glb"[^\n]*?\bwalk: ([0-9.]+)/.exec(text);
    return m ? [{ line: i + 1, model: m[1], walk: Number(m[2]) }] : [];
  });
}

/** What is stale in the biome files under `biomes`, a line each, the measured speed read from
 * `models` (assets/models/iso): empty when every roster walk is its model's. */
export function staleWalks(biomes: string, models: string, files = readdirSync(biomes).filter((f) => f.endsWith(".ron"))): string[] {
  const speeds = new Map<string, number>();
  const speed = (model: string) => {
    if (!speeds.has(model)) speeds.set(model, measure(readGlb(readFileSync(join(models, `${model}.glb`)))).clips.walk?.motion?.speed ?? NaN);
    return speeds.get(model)!;
  };
  return files.flatMap((file) =>
    rosterWalks(readFileSync(join(biomes, file), "utf8")).flatMap(({ line, model, walk }) => {
      const got = speed(model);
      if (Math.abs(got - walk) <= WALK_APART) return [];
      return [`${file}:${line}: ${model}'s walk is ${walk} m/s, but its model walks ${got.toFixed(2)} m/s (\`node tools/iso-art/iso-art.ts check ${model}\`, the planted foot's slide), so its feet slide; set \`walk: ${got.toFixed(2)}\` there (data, no code). Do not change the GLB or check.ts to match the roster.`];
    }),
  );
}
