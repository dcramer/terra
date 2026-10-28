// The incremental build (build.ts) on a made-up graph of shell steps in a scratch folder, and
// the character build's steps (build_steps.ts) against the committed data.
// node --test tools/characters/build.test.ts

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Builder } from "./build.ts";
import { bodiesData, buildSteps, closure, gearData, morphNames, type Step } from "./build_steps.ts";

const ROOT = join(import.meta.dirname, "..", "..");

/** first: src's first letter; second: first's, doubled; apart: reads nothing of theirs. */
function scratch() {
  const root = mkdtempSync(join(tmpdir(), "terra-build-"));
  mkdirSync(join(root, "out"));
  writeFileSync(join(root, "src.txt"), "ab\n");
  writeFileSync(join(root, "other.txt"), "x\n");
  const steps: Step[] = [
    { name: "first", section: 0, needs: [], inputs: ["src.txt"], outputs: ["out/first.txt"], run: "cut -c1 src.txt > out/first.txt; echo FIRST" },
    { name: "second", section: 1, needs: ["first"], inputs: [], outputs: ["out/second.txt"], run: "cat out/first.txt out/first.txt > out/second.txt" },
    { name: "apart", section: 1, needs: [], inputs: ["other.txt"], outputs: ["out/apart.txt"], run: "cp other.txt out/apart.txt" },
  ];
  const run = async (s = steps, wanted?: string[]) => {
    const lines: string[] = [];
    const b = new Builder(s, root, join(root, "out"), {}, (l) => lines.push(l));
    const ok = await b.build(wanted, 2);
    return { ok, b, states: Object.fromEntries([...b.outcomes].map(([n, o]) => [n, o.state])), lines };
  };
  return { root, steps, run };
}

test("a step runs once, is skipped while nothing it reads changes, and runs again with what its change reaches", async () => {
  const { root, run } = scratch();
  assert.deepEqual((await run()).states, { first: "built", second: "built", apart: "built" });
  const again = await run();
  assert.deepEqual(again.states, { first: "skipped", second: "skipped", apart: "skipped" });
  assert.match(again.lines.join("\n"), /· first skipped: unchanged/);
  writeFileSync(join(root, "src.txt"), "cb\n");
  const changed = await run();
  assert.deepEqual(changed.states, { first: "built", second: "built", apart: "skipped" });
  assert.match(changed.lines.join("\n"), /✓ first built in \d+s \(src\.txt changed\)/);
  assert.match(changed.lines.join("\n"), /✓ second built .*first's outputs changed/);
  assert.equal(readFileSync(join(root, "out", "second.txt"), "utf8"), "c\nc\n");
});

test("a rerun whose outputs come out the same stops there, as ninja's restat does", async () => {
  const { root, run } = scratch();
  await run();
  writeFileSync(join(root, "src.txt"), "az\n"); // first still writes "a"
  assert.deepEqual((await run()).states, { first: "built", second: "skipped", apart: "skipped" });
});

test("a failed step leaves no stamp, what needs it is not run, and the next run starts it again", async () => {
  const { root, steps, run } = scratch();
  const broken = steps.map((s) => (s.name === "first" ? { ...s, run: "echo half > out/first.txt; exit 3" } : s));
  const failed = await run(broken);
  assert.equal(failed.ok, false);
  assert.deepEqual(failed.states, { first: "failed", second: "not run", apart: "built" });
  assert.match(failed.lines.join("\n"), /✗ first failed: exited 3 after \d+s: see out\/steps\/first\.log/);
  assert.match(failed.b.joinedLog(steps.map((s) => s.name)), /^✗ first failed: exited 3/m);
  assert.deepEqual((await run()).states, { first: "built", second: "built", apart: "skipped" });
  assert.equal(readFileSync(join(root, "out", "first.txt"), "utf8"), "a\n");
});

test("an output changed since its step wrote it, or a step that wrote none, is run again", async () => {
  const { root, steps, run } = scratch();
  await run();
  writeFileSync(join(root, "out", "first.txt"), "edited\n");
  const after = await run();
  assert.equal(after.states.first, "built");
  assert.match(after.lines.join("\n"), /out\/first\.txt is not as it left it/);
  const lazy = steps.map((s) => (s.name === "apart" ? { ...s, run: "true", inputs: ["src.txt"] } : s));
  assert.deepEqual((await run(lazy)).states.apart, "failed");
  assert.equal(existsSync(join(root, "out", "stamps", "apart.json")), false);
});

test("a file a later step changes in place is checked by that step, which runs again whenever the first rewrites it", async () => {
  const { root, steps, run } = scratch();
  const paint: Step = { name: "paint", section: 2, needs: ["first"], inputs: [], outputs: [], inPlace: ["out/first.txt"], run: "echo painted >> out/first.txt" };
  const painted = [steps[0], paint];
  assert.deepEqual((await run(painted)).states, { first: "built", paint: "built" });
  assert.deepEqual((await run(painted)).states, { first: "skipped", paint: "skipped" });
  writeFileSync(join(root, "src.txt"), "az\n"); // first writes the same "a", unpainted
  assert.deepEqual((await run(painted)).states, { first: "built", paint: "built" });
  assert.equal(readFileSync(join(root, "out", "first.txt"), "utf8"), "a\npainted\n");
});

test("a committed output rewritten in place is taken as built while git has it and its inputs as committed", async () => {
  const { root, run } = scratch();
  writeFileSync(join(root, "made.txt"), "made\n");
  const git = (...a: string[]) => execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...a], { cwd: root, stdio: "ignore" });
  git("init", "-q");
  git("add", "src.txt", "made.txt");
  git("commit", "-qm", "x");
  const drift: Step = { name: "drift", section: 0, needs: [], inputs: ["src.txt"], outputs: [], inPlace: ["made.txt"], committed: true, run: '[[ -n ${ADOPT:-} ]] || echo again >> made.txt; echo CHECKED' };
  assert.equal((await run([drift])).states.drift, "built");
  assert.equal(readFileSync(join(root, "made.txt"), "utf8"), "made\n", "rewritten though nothing changed");
  writeFileSync(join(root, "src.txt"), "changed\n");
  assert.equal((await run([drift])).states.drift, "built");
  assert.equal(readFileSync(join(root, "made.txt"), "utf8"), "made\nagain\n", "not rebuilt from a changed input");
});

test("the joined log keeps the build's order and a skipped step's output from when it ran", async () => {
  const { steps, run } = scratch();
  await run();
  const { b } = await run();
  assert.equal(b.joinedLog(steps.map((s) => s.name)), "== 0. the clips\nFIRST\n== 1. the base\n");
  const only = await run(steps, ["second"]);
  assert.deepEqual(only.states, { first: "skipped", second: "skipped" });
});

test("the character build's steps: every need a step, every input there, each look fitted on every body and carried", () => {
  const bodies = bodiesData();
  const items = gearData();
  const steps = buildSteps(bodies, items);
  const names = new Set(steps.map((s) => s.name));
  assert.equal(names.size, steps.length, "a step named twice");
  for (const s of steps) {
    for (const n of s.needs) assert.ok(names.has(n), `${s.name} needs ${n}, no step`);
    for (const p of closure(s.inputs)) assert.ok(p === "art/animation/library" || existsSync(join(ROOT, p)), `${s.name} reads ${p}, which is not there`);
  }
  // Each step after those it needs, so build.log reads in the order the steps' data flows.
  for (const [i, s] of steps.entries()) for (const n of s.needs) assert.ok(steps.findIndex((t) => t.name === n) < i, `${s.name} before ${n}`);
  const looks = [...items.keys()];
  for (const body of [bodies.base.name, ...morphNames(bodies)]) for (const l of looks) assert.ok(names.has(`fit:${body}:${l}`), `${l} not fitted on ${body}`);
  for (const l of looks) {
    const look = steps.find((s) => s.name === `look:${l}`)!;
    assert.ok(morphNames(bodies).every((m) => look.needs.includes(`fit:${m}:${l}`) && look.needs.includes(`morph:${m}`)), `${l}'s game file before its fits`);
  }
  // A garment worn over another is made after it; one drawn on the female, after her morph.
  const skirt = steps.find((s) => s.name === "garment:skirt")!;
  assert.ok(skirt.needs.includes("garment:shift") && skirt.needs.includes("morph:female"));
  // Her skin is painted onto her body after every look is fitted on it, reading none of them.
  const skin = steps.find((s) => s.name === "skin:female")!;
  assert.ok(skin.after!.includes("fit:female:cap") && !skin.needs.some((n) => n.startsWith("fit:")));
});

test("a script's inputs follow its imports to the data they read", () => {
  const read = closure(["tools/characters/export_character.py"]);
  for (const p of ["tools/characters/look.py", "tools/characters/hold.py", "art/animation/shield_targets.json", "assets/models/character/skinning.json", "tools/blender/run.sh"])
    assert.ok(read.includes(p), `export_character.py's inputs miss ${p}`);
  assert.ok(closure(["tools/characters/markers.ts"]).includes("tools/characters/glb.ts"));
  assert.ok(!read.some((p) => p.includes("gear_check")), "a module it does not import");
});
