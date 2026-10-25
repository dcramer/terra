// The fixture of `cost.forest` (terra_walker movement_bench_forest.rs): the lush yard's fitted
// solids along the walk scene's route, dumped from the game on the Windows GPU by `world.solids`.
// node tools/controller/forest-fixture.ts [--running]  (--running: ask a sandbox already started)
import * as game from "../sandbox/client.ts";
import { writeFileSync } from "node:fs";
import { resolve } from "node:path";
const ROUTE: [number, number][] = [[0, 0], [-58, -39], [-40, -69], [-6, -80], [30, -74], [59, -54], [77, -22], [79, 14]];
const MARGIN = 6;
const out = resolve(import.meta.dirname, "../../crates/terra_walker/src/movement_bench_forest.json");
const running = process.argv.includes("--running");
if (!running) await game.start(["--experiment", "play", "--experiment", "lush", "--level", "yard", "--time", "17"], { windows: true, copy: "forestdump", release: true });
if (!running) await game.waitFor("the floor has", { since: 0, timeout: 300 });
await game.waitUntil("every footing fitted", async () => (await game.call("lush.solids", { at: [0, 0], within: 150 })) as { fitted: boolean }[], (all) => all.length > 0 && all.every((s) => s.fitted), 120);
await game.sleep(5000);
const got: { start: number[]; solids: any[] } = { start: [], solids: [] };
const seen = new Set<string>();
for (let i = 1; i < ROUTE.length; i++) {
  const [a, b] = [ROUTE[i - 1], ROUTE[i]];
  const n = Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1]) / 6);
  for (let k = 0; k <= n; k++) {
    const at = [a[0] + ((b[0] - a[0]) * k) / n, a[1] + ((b[1] - a[1]) * k) / n];
    const part = (await game.call("world.solids", { at, within: 6 })) as { start: number[]; solids: any[] };
    got.start = part.start;
    for (const s of part.solids) {
      const key = s.at.join(",");
      if (!seen.has(key)) (seen.add(key), got.solids.push(s));
    }
  }
}
if (!running) await game.stop();
const segDist = (p: [number, number], a: [number, number], b: [number, number]) => {
  const [dx, dy] = [b[0] - a[0], b[1] - a[1]];
  const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / (dx * dx + dy * dy)));
  return Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy);
};
const near = got.solids.filter((s) => {
  let reach = 0;
  for (const p of s.pieces) for (const h of p.hull) reach = Math.max(reach, Math.hypot(h[0], h[1]) + p.radius);
  const d = Math.min(...ROUTE.slice(1).map((b, i) => segDist(s.east_south, ROUTE[i], b)));
  return d < MARGIN + reach;
});
const r = (x: number, k = 4) => Number(x.toFixed(k));
const fixture = {
  about: "The lush yard's trees, rocks, stumps and logs within 6 m of the walk scene's route (xtask/src/scenes.rs `walk`), as flora fitted them (`world.solids`), for movement_bench's cost.forest",
  start: got.start.map((x) => r(x)),
  route: ROUTE,
  solids: near.map((s) => ({
    at: s.at.map((x: number) => r(x)),
    frame: s.frame.map((v: number[]) => v.map((x) => r(x, 6))),
    pieces: s.pieces.map((p: any) => ({ hull: p.hull.map((h: number[]) => h.map((x) => r(x))), radius: r(p.radius), top: r(p.top), crest: r(p.crest) })),
  })),
};
writeFileSync(out, JSON.stringify(fixture));
const pieces = near.reduce((n: number, s: any) => n + s.pieces.length, 0);
console.log(`dump: ${got.solids.length} solids in all, ${near.length} near the route, ${pieces} pieces; start ${got.start}`);
