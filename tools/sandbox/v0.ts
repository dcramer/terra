#!/usr/bin/env node
// Where v0's session stands (docs/v0-goals.md "v0 is done when", the Proof track): its seven
// steps, each with the checks that hold what of it is built, and in a word what is not. With
// no argument it only says so (nothing is run); `--run` runs every check named, one after
// another, a line each (front.ts's runner), and says which steps' checks passed. So one
// command, and a dozen lines, say how far v0 is: for a developer, and for a job deciding what
// is next.
//
//   node tools/sandbox/v0.ts [--run]
//
// Node 22.18 or later; no dependencies. `--run` takes about forty minutes.

import { resolve } from "node:path";
import { run } from "./front.ts";

const ROOT = resolve(import.meta.dirname, "../..");

type Step = { does: string; checks: string[]; held: string; missing: string };

/// The session's steps, in v0-goals' order. `checks` are scripts beside this one.
export const STEPS: Step[] = [
  { does: "1. A player makes a world on the title screen", checks: ["title"], held: "worlds made, left and loaded from the title screen, in one process", missing: "" },
  { does: "2. They host it; a friend joins from their own title screen", checks: ["join"], held: "hosted and joined by address, each player seen as made, the session's ends told in words", missing: "over the internet (a relay, D34); only this machine's address is tried" },
  { does: "3. Both walk three biomes in changing weather", checks: ["land"], held: "two players walk three biomes of a seed's land, each game saying the same biome, weather and hour, and the weather changes for both", missing: "the check failed one run in its last seven, a guest moved by the script not standing where its host put it (not understood, so it does not gate); the players are not walked by their keys; one seed; rain and wind were calm in the run (Land, Living world)" },
  { does: "4. They dig, gather, craft, equip, and meet creatures, each seeing the same", checks: ["net", "join"], held: "walking, digging and the trees felled are the same in both games; each shows what the other holds in hand; a joined game shows the host's creatures where the host has them, its player's blow fells one on the host, and a creature's blow takes its health", missing: "crafting and the pack do not cross the network (netcode.md step 5)" },
  { does: "5. They beat the boss, are marked, and open the gate", checks: ["gate", "join", "den"], held: "the gate shut until the troll's mark, earned by its fall at its den, opens it, for good, saved, and open for both players of a shared world; every world has a den found from its seed, walked from its mouth to its chamber, where the troll lives; its fall marks the players by it, a joined one too", missing: "a player's own fight to the kill: a script deals the last blow (the troll's roar and blows land on a standing player, den.ts --fight, and were looked at as a sheet of stills); no film or duel has judged the fight" },
  { does: "6. They cross, together or apart, and come back", checks: ["gate", "crossing", "portal"], held: "one player crosses to the industrial realm, its light and parts as its data says, and back; a joined game is handed between a headless host's realms; five round trips leave nothing behind, each within 3 s", missing: "crossing in a shared world is refused (D640); the industrial realm has no monsters or sound of its own, and its animals are a stand-in (Realm two)" },
  { does: "7. They quit and return; both realms and characters are as left", checks: ["gate", "join"], held: "a game closed in the realm beyond starts again there, and the first world is as it was left; a friend whose game is started anew is given the character the host kept", missing: "a guest's character is kept by the host at the guest's word: the pack is not the server's yet (D592, netcode.md step 5)" },
];

const running = process.argv.includes("--run");
const checks = [...new Set(STEPS.flatMap((step) => step.checks))];
let passed = new Set<string>();
let failed = 0;
if (running) {
  const ran = run(checks, import.meta.dirname, resolve(ROOT, "target/check"), { name: "v0", passed: "every check v0 has passes" });
  console.log(ran.said.join("\n"));
  passed = new Set(ran.said.filter((line) => line.startsWith("✓ ") && !line.startsWith("✓ v0")).map((line) => line.slice(2).split(" ")[0]));
  failed = ran.failed;
}
for (const step of STEPS) {
  const state = step.checks.length === 0 ? "✗" : !running ? "·" : step.checks.every((check) => passed.has(check)) ? "✓" : "✗";
  console.log(`${state} ${step.does}`);
  if (step.held) console.log(`    held by ${step.checks.map((check) => `${check}.ts`).join(", ")}: ${step.held}`);
  if (step.missing) console.log(`    not yet: ${step.missing}`);
}
const whole = STEPS.filter((step) => step.missing === "").length;
console.log(`v0: ${whole} of ${STEPS.length} steps whole; ${running ? `${checks.length - failed} of ${checks.length} checks passed` : `${checks.length} checks (--run runs them)`}`);
process.exit(failed ? 1 : 0);
