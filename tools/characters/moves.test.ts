import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { writeFileSync } from "node:fs";
import { Glb, place, qinv, rot, sub, type V3 } from "./glb.ts";
import { shaped, skinOf } from "./skin.ts";

const ROOT = join(import.meta.dirname, "..", "..");
const MOVES = join(ROOT, "tools", "characters", "moves.ts");
const TEMPLATES = join(ROOT, "assets", "models", "character", "templates.json");

/** A copy of a look's file with its mesh node moved to `at` (its own frame: the socket's). */
function movedLook(file: string, to: string, at: V3) {
  const raw = readFileSync(file);
  const jsonLen = raw.readUInt32LE(12);
  const gltf = JSON.parse(raw.subarray(20, 20 + jsonLen).toString("utf8")) as { nodes: { mesh?: number; matrix?: number[]; translation?: V3 }[] };
  const node = gltf.nodes.find((n) => n.mesh !== undefined)!;
  assert.equal(node.matrix, undefined, "the look's node is placed by a matrix: move it there instead");
  node.translation = at;
  let json = Buffer.from(JSON.stringify(gltf), "utf8");
  json = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, " ")]);
  const rest = raw.subarray(20 + jsonLen);
  const head = Buffer.alloc(20);
  raw.copy(head, 0, 0, 20);
  head.writeUInt32LE(20 + json.length + rest.length, 8);
  head.writeUInt32LE(json.length, 12);
  writeFileSync(to, Buffer.concat([head, json, rest]));
}

// The planted faults (D632): the check is shown to catch what it is for before its number is a pass.
test("a held look moved into a calf fails under --legs, naming the calf, the clip and the body", () => {
  // The stone axe as committed, and a copy with its grip moved from the right fist to the middle
  // of the right shin at the clip's first frame: only the copy fails. The clip is one whose own
  // arm, with no carry pose laid on it, holds the committed axe clear of the body (the walk's and
  // the idle's swing it into the thigh, and the walk's 2.5 cm into the calf).
  const CLIP = "Jump_Loop";
  const look = join(ROOT, "assets", "models", "held", "stone_axe.glb");
  const body = new Glb(join(ROOT, "assets", "models", "character", "base.glb"));
  const clip = new Glb(join(ROOT, "assets", "models", "character", "fight.glb")).clip(CLIP)!;
  const { shapes } = skinOf(body, TEMPLATES, ["male"]);
  const pose = shaped(body, shapes[0], clip, 0);
  const at = (n: string) => pose[body.named(n)];
  const [knee, ankle, hand] = [at("calf_r").t, at("foot_r").t, at("socket.hand.R")];
  const shin = knee.map((x, j) => (x + ankle[j]) / 2) as V3;
  const inHand = rot(qinv(hand.r), sub(shin, hand.t)).map((x, j) => x / hand.s[j]) as V3;

  const dir = mkdtempSync(join(tmpdir(), "moves-legs-"));
  const planted = join(dir, "stone_axe.glb");
  movedLook(look, planted, inHand);
  const run = (file: string, legs: boolean) => {
    const json = join(dir, "rows.json");
    rmSync(json, { force: true });
    const r = spawnSync(process.execPath, [MOVES, "--set", "none", "--also", CLIP, "--template", "male", "--hold", `socket.hand.R=${file}`, "--no-grip", "--json", json, ...(legs ? ["--legs"] : [])], { cwd: ROOT, encoding: "utf8" });
    const rows = JSON.parse(readFileSync(json, "utf8")) as { clip: string; bad: string[]; measured: Record<string, number>; where?: Record<string, string> }[];
    return { status: r.status, out: r.stdout + r.stderr, row: rows.find((x) => x.clip === CLIP)! };
  };
  const [sound, fault, read] = [run(look, true), run(planted, true), run(planted, false)];
  rmSync(dir, { recursive: true, force: true });
  const LEGS = "held_legs_cm.stone_axe";

  assert.equal(sound.status, 0, `the committed look fails: ${sound.out}`);
  assert.ok(!(sound.row.measured[LEGS] > 1), "the committed look reads in a leg");

  assert.equal(fault.status, 1, fault.out);
  assert.ok(fault.row.measured[LEGS] > 1, `${fault.row.measured[LEGS]} cm in the leg`);
  assert.match(fault.row.where?.[LEGS] ?? "", new RegExp(`^its point \\(-?\\d+, -?\\d+, -?\\d+\\) cm in calf_r in ${CLIP} at [\\d.]+ s, male$`));
  // (Moved with the hand through the clip, it may cross the thigh too: the legs' line is the one judged.)
  const legs = fault.row.bad.filter((b) => b.includes("in a leg"));
  assert.equal(legs.length, 1, fault.out);
  assert.match(legs[0], /^held [\d.]+ cm deep in a leg, stone_axe in calf_r at [\d.]+ s, male$/);
  assert.match(fault.out, new RegExp(`${CLIP}: held [\\d.]+ cm deep in a leg, stone_axe in calf_r`));

  // Without --legs it is read, counted and said, not over the bar.
  assert.equal(read.status, 0, read.out);
  assert.deepEqual(read.row.bad, []);
  assert.ok(read.row.measured[LEGS] > 1);
  assert.match(read.out, /legs: a held look over 1 cm into a calf or foot in 1 rows, read and not barred/);
});

test("a --template that names no body template is an error naming it and the ones there are", () => {
  // `--template base` ran no body, so every depth was -Infinity and the check passed.
  const dir = mkdtempSync(join(tmpdir(), "moves-template-"));
  const json = join(dir, "rows.json");
  const run = spawnSync(process.execPath, [MOVES, "--set", "none", "--also", "Walk_Loop", "--template", "male,base", "--json", json], { cwd: ROOT, encoding: "utf8" });
  const wrote = (() => {
    try {
      return readFileSync(json, "utf8");
    } catch {
      return undefined;
    }
  })();
  rmSync(dir, { recursive: true, force: true });
  assert.equal(run.status, 2, run.stdout + run.stderr);
  assert.equal(wrote, undefined, "rows were written for a run on no such body");
  assert.doesNotMatch(run.stdout, /✓/);
  assert.match(run.stderr, /no body template "base"/);
  assert.doesNotMatch(run.stderr, /"male"/);
  for (const name of Object.keys(JSON.parse(readFileSync(TEMPLATES, "utf8")))) assert.ok(run.stderr.includes(name), `does not name ${name}: ${run.stderr}`);
  // fit.ts shows the line of a failed run that says "error" (runMoves).
  assert.match(run.stderr.split("\n").find((l) => /error/i.test(l)) ?? "", /no body template/);
});

test("a held look's row says which of its points went deepest, into what, when and on which body", () => {
  const dir = mkdtempSync(join(tmpdir(), "moves-where-"));
  const json = join(dir, "rows.json");
  const look = "assets/models/held/stone_axe.glb";
  const run = spawnSync(process.execPath, [join(ROOT, "tools", "characters", "moves.ts"), "--set", "none", "--also", "Walk_Loop", "--template", "male", "--hold", `socket.hand.R=${look}`, "--carry", "--arms", "--json", json], { cwd: ROOT, encoding: "utf8" });
  const rows = JSON.parse(readFileSync(json, "utf8")) as { clip: string; measured: Record<string, number>; where?: Record<string, string> }[];
  rmSync(dir, { recursive: true, force: true });
  const walk = rows.find((r) => r.clip === "Walk_Loop");
  assert.ok(walk, run.stderr);
  const said = walk.where?.["held_cm.stone_axe"] ?? "";
  const m = /^its point \((-?\d+), (-?\d+), (-?\d+)\) cm in (\w+) in Walk_Loop at ([\d.]+) s, male$/.exec(said);
  assert.ok(m, said);
  // The point is one of the look's own vertices (its frame as made, the grip at the origin), to a
  // centimetre's rounding, not one hung in the hand.
  const g = new Glb(join(ROOT, look));
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const pose = g.rest(node);
  const at = [1, 2, 3].map((i) => Number(m[i]) / 100);
  let near = Infinity;
  for (const prim of g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives) {
    const p = g.read(prim.attributes.POSITION);
    for (let i = 0; i < p.length; i += 3) near = Math.min(near, Math.hypot(...place(pose, [p[i], p[i + 1], p[i + 2]] as V3).map((v, j) => v - at[j])));
  }
  assert.ok(near < 0.01, `${said}: ${(near * 100).toFixed(1)} cm from its nearest vertex`);
});
