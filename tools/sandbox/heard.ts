// node tools/sandbox/heard.ts [--seed 3] [--seconds 180]: a walk among wildlife, judged by what
// was heard (D69): begins a world, walks for so long, steps up to each kind of animal it comes
// near, and asks `sounds.heard` and `sounds.unheard`. It fails when one voice was heard more
// often than the bar a minute, or a cry asked for more often (heard/judge.ts says where the bar
// is from, D657), or when a kind stepped up to was not heard as its data says. A line when it passes (D388). Run through `cargo xtask gpu still -- …`.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { call, holdKey, ROOT, start, state, status, stop, waitSeconds, waitUntil, warp } from "./client.ts";
import { BAR_PER_MIN, busiest, faults, readKinds, type Heard, type Met } from "./heard/judge.ts";

const option = (name: string, otherwise: string) => {
  const at = process.argv.indexOf(`--${name}`);
  return at < 0 ? otherwise : process.argv[at + 1];
};
const seed = option("seed", "3");
const seconds = Number(option("seconds", "180"));
const kinds = readKinds(readFileSync(resolve(ROOT, "assets/data/creatures.ron"), "utf8"));
const heard = async () => (await call("sounds.heard")) as Heard;

let found: string[] = [];
let passed = "";
try {
  await start(["--experiment", "play", "--fresh", "--save", "shots/heard.ron", "--seed", seed, "--offscreen", "--mute"], { realTime: true, release: true });
  await waitUntil("the world to be played", async () => (await call("title.state")) as { screen: string }, (now) => now.screen === "Playing", 600);
  const before = await heard();
  const began = (await status()).seconds;
  const met: Met[] = [];
  for (;;) {
    const now = (await status()).seconds;
    if (now - began >= seconds) break;
    const one = (await state()).near.find((near) => near.what && (near.health ?? 1) > 0 && !met.some((m) => m.kind === near.what));
    if (!one) {
      await holdKey("w", Math.min(8, seconds - (now - began)));
      continue;
    }
    // Two metres from it, facing it: nearer than any shy kind lets a walker come.
    const was = await heard();
    await warp([one.at[0] + 2, one.at[1]], [-1, 0]);
    await waitSeconds(5);
    const is = await heard();
    met.push({ kind: one.what!, after: Object.keys(is).filter((name) => is[name] > (was[name] ?? 0)) });
  }
  const after = await heard();
  // The animal voices in earshot not played, over their budget (D657).
  const unheard = (await call("sounds.unheard")) as Heard;
  const minutes = ((await status()).seconds - began) / 60;
  found = faults(kinds, before, after, minutes, met, unheard);
  if (met.length === 0) found.push(`no animal came within play.state's reach in ${seconds} s on seed ${seed}: nothing was judged; walk longer (--seconds) or another seed`);
  const most = busiest(kinds, before, after, minutes);
  passed = `✓ heard: ${minutes.toFixed(1)} min on seed ${seed}, met ${met.map((m) => m.kind).join(", ")}; the busiest voice ${most.voice} ${most.rate.toFixed(1)} a minute (bar ${BAR_PER_MIN}); each kind met was heard as its data says`;
  if (process.env.TERRA_VERBOSE) console.log(JSON.stringify({ met, before, after, unheard }));
} catch (problem) {
  found = [`the run did not finish: ${problem instanceof Error ? problem.message : problem} (a refusal is in shots/remote.log: the game runs one copy a machine, wait for the other and run again)`];
} finally {
  await stop().catch(() => {});
}
if (found.length > 0) {
  for (const fault of found) console.error(`✗ heard: ${fault}`);
  process.exit(1);
}
console.log(passed);
