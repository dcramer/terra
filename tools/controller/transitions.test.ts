// transitions.ts's checks on made-up walks: each fault planted, and its clean twin passing.
// node --test tools/controller/transitions.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import type { Frame } from "../sandbox/duel/checks.ts";
import { settle, starts, turns } from "./transitions.ts";
import type { Walk } from "./yard-checks.ts";

type Row = { speed: number; yaw?: number; x?: number; feet?: [number, number]; kept?: [string, string] };

/// A walker's frames from rows: its speed, facing (deg about the up), where it is east, each
/// ball's place east, and how each foot is kept.
function walk(rows: Row[]): Walk {
  const frames = rows.map((row, i) => {
    const yaw = ((row.yaw ?? 0) * Math.PI) / 180;
    const [l, r] = row.feet ?? [0, 0];
    return {
      frame: i,
      fighters: [{
        who: "you",
        feet: [row.x ?? 0, 0, 0],
        facing: [Math.sin(yaw), 0, Math.cos(yaw)],
        speed: row.speed,
        balls: [{ at: [l, 0, -0.1], up: 0.05 }, { at: [r, 0, 0.1], up: 0.05 }],
        kept: row.kept ?? ["planted", "planted"],
      }],
    } as unknown as Frame;
  });
  return { name: "jog", frames, route: [], ground: [] };
}

const run = (n: number): Row[] => Array.from({ length: n }, () => ({ speed: 4.3, feet: [0, 0], kept: ["free", "free"] }));
const stand = (n: number, row: (i: number) => Partial<Row> = () => ({})): Row[] => Array.from({ length: n }, (_, i) => ({ speed: 0, ...row(i) }));

test("a stop whose feet rest with the body passes; a settle step half a second on fails", () => {
  const clean = settle([walk([...run(30), ...stand(60)])]);
  assert.ok(clean.ok, clean.line);
  assert.equal(clean.value, 0);
  // The right foot steps 20 cm over frames 30-36 after the body stood (frame 30).
  const shuffled = settle([walk([...run(30), ...stand(60, (i) => ({ feet: [0, i >= 30 && i < 36 ? (i - 29) * 0.033 : i >= 36 ? 0.2 : 0] }))])]);
  assert.ok(!shuffled.ok, shuffled.line);
  assert.ok(Math.abs(shuffled.value - 35 / 60) < 0.02, shuffled.line);
});

test("a start whose foot the rule steps fails; one the clip steps passes; a turning start is a turn's", () => {
  const off = (kept: (i: number) => [string, string], yaw = (_: number) => 0) =>
    walk([...stand(30), ...Array.from({ length: 60 }, (_, i) => ({ speed: Math.min(4.3, i * 0.5), yaw: yaw(i), kept: kept(i) }))]);
  const clean = starts([off(() => ["free", "free"])]);
  assert.ok(clean.ok && clean.value === 0, clean.line);
  const dragged = starts([off((i) => (i >= 8 && i < 14 ? ["stepping", "free"] : ["free", "free"]))]);
  assert.equal(dragged.value, 1, dragged.line);
  const turning = (i: number) => Math.min(180, i * 10);
  const pivot = off((i) => (i >= 8 && i < 14 ? ["stepping", "free"] : ["free", "free"]), turning);
  assert.equal(starts([pivot]).line.includes("no start"), true);
  const turned = turns([pivot]);
  assert.equal(turned.value, 1, turned.line);
});

test("a turn in place stepped by the rule fails; a small turn or one at a run is none", () => {
  const turn = (deg: number, speed: number) =>
    walk(Array.from({ length: 90 }, (_, i) => ({ speed, yaw: Math.min(deg, Math.max(0, (i - 10) * 3)), kept: (i % 20 === 5 ? ["stepping", "planted"] : ["planted", "planted"]) as [string, string] })));
  assert.ok(turns([turn(90, 0)]).value > 0);
  assert.ok(turns([turn(30, 0)]).line.includes("no turn"));
  assert.ok(turns([turn(90, 4.3)]).line.includes("no turn"));
});
