// Every made clip plays at the pace its take was made at (D585): a stride's loop lasts as long
// as a stride of its take at the take's own frame rate, and a clip kept whole lasts as long as
// its take. Kimodo's takes are 30 frames a second; made_clips.py once keyed a stride's take
// frames one to one on Blender's 24 fps scene, so the walk, jog and sprint played at 0.8 of
// their take's cadence (clip-fps j1). Fails, with no Blender, on the bodies the game plays.
// node --test tools/characters/clip_rate.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { Glb } from "./glb.ts";

type Entry = { still?: boolean; once?: boolean; looped?: number; span?: [number, number] };
type Joint = { name: string; parent: number; offset: number[]; channels: string[]; at: number };

/** A BVH take: its joints, each frame's channel values and its frame time (s). */
function bvh(file: string) {
  const [head, motion] = readFileSync(file, "utf8").split("MOTION");
  const joints: Joint[] = [];
  const stack: number[] = [];
  let width = 0;
  for (const line of head.split("\n").map((l) => l.trim())) {
    const [word, ...rest] = line.split(/\s+/);
    if (word === "ROOT" || word === "JOINT") joints.push({ name: rest[0], parent: stack.at(-1) ?? -1, offset: [], channels: [], at: 0 });
    else if (word === "End") joints.push({ name: "", parent: stack.at(-1) ?? -1, offset: [], channels: [], at: 0 });
    else if (word === "{") stack.push(joints.length - 1);
    else if (word === "}") stack.pop();
    else if (word === "OFFSET") joints.at(-1)!.offset = rest.map(Number);
    else if (word === "CHANNELS") Object.assign(joints.at(-1)!, { channels: rest.slice(1), at: width }), (width += Number(rest[0]));
  }
  const lines = motion.trim().split("\n");
  const dt = Number(lines[1].split(":")[1]);
  return { joints, dt, frames: lines.slice(2).map((l) => l.trim().split(/\s+/).map(Number)) };
}

type M = number[]; // 3x3 row-major
const mul = (a: M, b: M): M => [0, 1, 2].flatMap((r) => [0, 1, 2].map((c) => a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c]));
const apply = (a: M, v: number[]) => [0, 1, 2].map((r) => a[r * 3] * v[0] + a[r * 3 + 1] * v[1] + a[r * 3 + 2] * v[2]);
function axis(which: string, deg: number): M {
  const [c, s] = [Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
  if (which === "X") return [1, 0, 0, 0, c, -s, 0, s, c];
  if (which === "Y") return [c, 0, s, 0, 1, 0, -s, 0, c];
  return [c, -s, 0, s, c, 0, 0, 0, 1];
}

/** Where `joint` is (cm, Y up) in each frame of the take. */
function track(take: ReturnType<typeof bvh>, joint: string): number[][] {
  const chain: Joint[] = [];
  for (let j = take.joints.findIndex((x) => x.name === joint); j >= 0; j = take.joints[j].parent) chain.unshift(take.joints[j]);
  return take.frames.map((row) => {
    let [rot, pos]: [M, number[]] = [[1, 0, 0, 0, 1, 0, 0, 0, 1], [0, 0, 0]];
    for (const j of chain) {
      let local = j.offset;
      let turn: M = [1, 0, 0, 0, 1, 0, 0, 0, 1];
      j.channels.forEach((ch, i) => {
        const v = row[j.at + i];
        if (ch.endsWith("rotation")) turn = mul(turn, axis(ch[0], v));
        else local = local === j.offset ? [...j.offset] : local, (local["XYZ".indexOf(ch[0])] = v);
      });
      pos = pos.map((p, i) => p + apply(rot, local)[i]);
      rot = mul(rot, turn);
    }
    return pos;
  });
}

/** The take's stride (s): the middle of the times between left heel strikes past its first
 * second, a strike found as made_clips.py's `stride` finds it (the foot within 5 cm of its
 * lowest and slower than 0.75 m/s over the ground, after it was not). */
function stride(take: ReturnType<typeof bvh>): number {
  const foot = track(take, "LeftFoot");
  const low = Math.min(...foot.map((p) => p[1]));
  const planted = foot.map((p, k) => k > 0 && p[1] < low + 5 && (Math.hypot(p[0] - foot[k - 1][0], p[2] - foot[k - 1][2]) / 100) / take.dt < 0.75);
  const strikes = planted.flatMap((on, k) => (on && !planted[k - 1] && k * take.dt >= 1 && k < foot.length - 5 ? [k] : []));
  const gaps = strikes.slice(1).map((b, i) => (b - strikes[i]) * take.dt).filter((s) => s >= 0.4 && s <= 1.5).sort((a, b) => a - b);
  assert.ok(gaps.length, `no left heel strikes a stride apart in the take (strikes at frames ${strikes})`);
  return gaps[Math.floor(gaps.length / 2)];
}

/** How long a GLB take lasts (s): its longest track. */
function glbLength(file: string): number {
  const take = new Glb(file);
  let end = 0;
  for (const a of take.gltf.animations ?? []) for (const s of a.samplers) end = Math.max(end, take.gltf.accessors[s.input].max?.[0] ?? 0);
  return end;
}

const SETS = [
  { list: "art/animation/made.json", takes: "art/animation/made", body: "assets/models/character/base.glb" },
  { list: "art/animation/sets/fight.json", takes: "art/animation/sets/fight", body: "assets/models/character/fight.glb" },
  // Not yet art/animation/sets/troll.json on assets/models/characters/troll.glb: that file is
  // exported by hand outside the build (its colour factor set after, 1e1a8615), and its
  // Troll_Walk still plays at 0.8 of its take. Add it here when the troll is made again.
];
/** A stride may be a frame or two off its take's middle one (the loop is the pair of strikes
 * whose poses best match), well inside the tenth D585 holds a gait to; 24 fps keys of a 30 fps
 * take make it 0.25 too long. */
const STRIDE_WITHIN = 0.1;
/** A clip kept whole is its take's length, give or take the scene's frame it is rounded to. */
const WHOLE_WITHIN_S = 1 / 20;

for (const set of SETS) {
  const list: Record<string, Entry & Record<string, unknown>> = JSON.parse(readFileSync(set.list, "utf8"));
  const body = new Glb(set.body);
  for (const [name, entry] of Object.entries(list)) {
    const file = [".glb", ".bvh"].map((ext) => `${set.takes}/${name}${ext}`).find(existsSync);
    if (!file || entry.still && !entry.looped) continue; // derived from another clip, or a window chosen in the take
    test(`${name} in ${set.body} lasts as its take at the take's own frame rate`, () => {
      const clip = body.clip(name);
      assert.ok(clip, `${name} is not in ${set.body}: make it (made_clips.py) and rebuild the bodies (tools/characters/build.ts)`);
      const fix = "key the take at its own frame rate in made_clips.py (its `k`, the scene's frames a take frame), never a speed factor, then rebuild the bodies";
      if (entry.once || entry.looped) {
        const whole = file.endsWith(".glb") ? glbLength(file) : (() => {
          const take = bvh(file);
          const all = (take.frames.length - 1) * take.dt;
          return entry.span ? Math.min(all, entry.span[1]) - entry.span[0] : all;
        })();
        assert.ok(Math.abs(clip.end - whole) <= WHOLE_WITHIN_S,
          `${name} lasts ${clip.end.toFixed(3)} s but its take ${whole.toFixed(3)} s (${(clip.end / whole).toFixed(2)} times): ${fix}`);
      } else {
        const take = bvh(file);
        const made = stride(take);
        assert.ok(Math.abs(clip.end / made - 1) <= STRIDE_WITHIN,
          `${name}'s stride lasts ${clip.end.toFixed(3)} s but its take's ${made.toFixed(3)} s at ${(1 / take.dt).toFixed(0)} fps (${(clip.end / made).toFixed(2)} times, so it plays at ${(made / clip.end).toFixed(2)} of its cadence): ${fix}`);
      }
    });
  }
}
