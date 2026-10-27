import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { readPng, writePng } from "../sandbox/png.ts";
import { gatherFaults, type Key, make, score, sideBySide } from "./judge.ts";
import { binomialP, bootstrapDiff, judgesNeeded, mean, permutationP, sd, tCritical, tP, welch, wilson } from "./stats.ts";

test("mean and spread of known numbers", () => {
  assert.equal(mean([2, 4, 4, 4, 5, 5, 7, 9]), 5);
  assert.ok(Math.abs(sd([2, 4, 4, 4, 5, 5, 7, 9]) - Math.sqrt(32 / 7)) < 1e-12);
});

test("the same judges twice show no difference; far-apart judges do", () => {
  const a = [4.5, 4.8, 5, 4.7, 4.6, 5.1, 4.9, 4.4];
  const [lo, hi] = bootstrapDiff(a, [...a]);
  assert.ok(lo <= 0 && hi >= 0, `${lo} ${hi}`);
  assert.ok(permutationP(a, [...a]) > 0.9);
  const b = a.map((x) => x + 1.5);
  const [blo] = bootstrapDiff(a, b);
  assert.ok(blo > 1, `${blo}`);
  assert.ok(permutationP(a, b) < 0.01);
});

test("Student's t at table values, and Welch's test", () => {
  // Tables: t 2.228 leaves 5% two-sided at 10 df, 2.571 at 5 df, 1.960 at large df; t 1 at 1 df leaves exactly 0.5.
  assert.ok(Math.abs(tCritical(10) - 2.228) < 1e-3);
  assert.ok(Math.abs(tCritical(5) - 2.571) < 1e-3);
  assert.ok(Math.abs(tCritical(1e6) - 1.96) < 1e-3);
  assert.ok(Math.abs(tP(1, 1) - 0.5) < 1e-9);
  // Equal spreads and sizes: Welch's t is the pooled t, (5 - 4) / sqrt(2 * 1 / 3), on 4 df.
  const w = welch([3, 4, 5], [4, 5, 6]);
  assert.ok(Math.abs(w.diff - 1) < 1e-12 && Math.abs(w.df - 4) < 1e-9);
  assert.ok(Math.abs(w.p - tP(1 / Math.sqrt(2 / 3), 4)) < 1e-12);
  assert.ok(w.lo < 0 && w.hi > 0);
});

test("judges needed follow the two-sample normal formula", () => {
  // 2 (1.96 + 0.84)^2 s^2 / d^2: s 0.5, d 0.25 gives 62.8, so 63.
  assert.equal(judgesNeeded(0.5, 0.25), 63);
  assert.equal(judgesNeeded(0.25, 0.25), 16);
});

test("Wilson interval and binomial test at known values", () => {
  const [lo, hi] = wilson(5, 10);
  assert.ok(Math.abs(lo - 0.2366) < 1e-3 && Math.abs(hi - 0.7634) < 1e-3);
  assert.ok(Math.abs(binomialP(8, 10) - 0.109375) < 1e-9); // 2 * (45 + 10 + 1) / 1024
  assert.equal(binomialP(5, 10), 1);
});

test("side by side keeps each picture's pixels and puts a bar between", () => {
  const solid = (w: number, h: number, v: number) => ({ width: w, height: h, rgba: new Uint8Array(w * h * 4).fill(v) });
  const p = sideBySide(solid(3, 2, 10), solid(2, 3, 200), 4);
  assert.equal(p.width, 9);
  assert.equal(p.height, 3);
  const at = (x: number, y: number) => p.rgba[(y * p.width + x) * 4];
  assert.equal(at(0, 0), 10);
  assert.equal(at(4, 1), 128);
  assert.equal(at(8, 2), 200);
  assert.equal(at(0, 2), 0); // the shorter one padded black
});

function fixture() {
  const root = mkdtempSync(join(tmpdir(), "judge-"));
  for (const [b, v] of [["main", 50], ["new", 150]] as const) {
    mkdirSync(join(root, "set", b), { recursive: true });
    for (const view of ["dusk.png", "night.png"])
      writePng(join(root, "set", b, view), { width: 2, height: 2, rgba: new Uint8Array(16).fill(v) });
  }
  mkdirSync(join(root, "anchors"));
  writeFileSync(join(root, "anchors", "hi.png"), "x");
  writeFileSync(join(root, "anchors", "anchors.json"), JSON.stringify([{ file: "hi.png", score: 9, what: "the bar" }]));
  writeFileSync(join(root, "brief.md"), "Score the fire.");
  return root;
}

test("absolute panel: every judge sees one build only, and the score reads it back", () => {
  const root = fixture();
  try {
    const out = join(root, "p");
    const key = make({ brief: join(root, "brief.md"), set: join(root, "set"), out, judges: 3, anchors: join(root, "anchors") });
    assert.equal(key.judges.length, 6);
    for (const j of key.judges) {
      const files = readdirSync(join(out, j.id)).sort();
      assert.deepEqual(files, ["anchor-1.png", "dusk.png", "night.png", "prompt.md"]);
      const shade = readPng(join(out, j.id, "dusk.png")).rgba[0];
      assert.equal(shade, j.build === "main" ? 50 : 150);
      assert.ok(!readFileSync(join(out, j.id, "prompt.md"), "utf8").includes(j.build!));
      const s = j.build === "main" ? 4 : 6;
      writeFileSync(join(out, j.id, "scores.txt"), `dusk.png ${s}\nnight.png ${s + 0.5}\n`);
    }
    const lines = score(out);
    assert.ok(lines.some((l) => l.startsWith("new - main: +2.00") && l.includes("new better")), lines.join("\n"));
    assert.throws(() => make({ brief: join(root, "brief.md"), set: join(root, "set"), out, judges: 1 }));
    const more = make({ brief: join(root, "brief.md"), set: join(root, "set"), out, judges: 1, builds: ["new"], add: true });
    assert.deepEqual(more.judges.slice(6), [{ id: "j07", build: "new" }]);
    assert.equal(readFileSync(join(out, "judges.txt"), "utf8").trim(), join(out, "j07", "prompt.md"));
    assert.ok(score(out).some((l) => l.startsWith("missing or malformed (1): j07")));
  } finally {
    rmSync(root, { recursive: true });
  }
});

test("faults are asked for, gathered by build and view beside the scores, and leave the statistics alone", () => {
  const root = fixture();
  try {
    const out = join(root, "f");
    const key = make({ brief: join(root, "brief.md"), set: join(root, "set"), out, judges: 2, anchors: join(root, "anchors") });
    const p = readFileSync(join(out, key.judges[0].id, "prompt.md"), "utf8");
    assert.ok(p.includes(join(out, key.judges[0].id, "faults.txt")) && p.includes("6 lines"), p);
    for (const j of key.judges) {
      const s = j.build === "main" ? 4 : 6;
      writeFileSync(join(out, j.id, "scores.txt"), `dusk.png ${s}\nnight.png ${s + 0.5}\n`);
    }
    const before = score(out);
    const judged = key.judges.filter((j) => j.build === "new");
    // One judge writes faults (with a dropped extension, a colon and a stray line); the other writes none.
    writeFileSync(
      join(out, judged[0].id, "faults.txt"),
      "dusk.png the pool is olive on the grass\ndusk: the stones go pink\nnotes follow\nnight.png the sky is flat\n",
    );
    assert.deepEqual(score(out), before);
    const lines = gatherFaults(out);
    const at = (head: string) => lines.indexOf(head);
    assert.ok(at("new dusk.png") >= 0 && at("new night.png") > at("new dusk.png"), lines.join("\n"));
    assert.deepEqual(lines.slice(at("new dusk.png") + 1, at("new night.png")), [
      `  ${judged[0].id} 6    the pool is olive on the grass`,
      `  ${judged[0].id} 6    the stones go pink`,
    ]);
    assert.equal(lines[at("new night.png") + 1], `  ${judged[0].id} 6.5  the sky is flat`);
    assert.ok(!lines.some((l) => l.includes("notes follow")));
    const none = lines.find((l) => l.startsWith("no faults"))!;
    assert.ok(none.includes(judged[1].id) && none.startsWith("no faults (3)"), none);
  } finally {
    rmSync(root, { recursive: true });
  }
});

test("forced choice: half the judges see each side, and a pure side bias scores as no preference", () => {
  const root = fixture();
  try {
    const out = join(root, "c");
    const key: Key = make({ brief: join(root, "brief.md"), set: join(root, "set"), out, judges: 8, choice: ["main", "new"] });
    assert.equal(key.judges.filter((j) => j.left === "main").length, 4);
    for (const j of key.judges) {
      const p = readPng(join(out, j.id, "night.png"));
      assert.equal(p.rgba[0], j.left === "main" ? 50 : 150);
      writeFileSync(join(out, j.id, "scores.txt"), "dusk.png R\nnight.png R\noverall R\n");
    }
    const lines = score(out);
    assert.ok(lines.some((l) => l.startsWith("new chosen 4/8 = 50%")), lines.join("\n"));
    assert.ok(lines.some((l) => l.startsWith("left chosen 0/8 = 0%")), lines.join("\n"));
  } finally {
    rmSync(root, { recursive: true });
  }
});
