// The checks a GLB can pass without a GPU, each with what it is for. Read in Node from the
// file alone: the clips by name and length, whether the moving clips stay in place and how fast
// their feet say the body goes, whether loops close and the death ends down, the triangles and
// textures against the kind's budget, the height, the feet on y=0 and the facing.

import { triangles } from "../tripo/glb.ts";
import { type Clip, type Glb, type Vec3, bounds, clips, imageSizes, origin, points, pose } from "./gltf.ts";
import { type Kind, SPECS, budget, clipsFor } from "./kinds.ts";

export type Result = { check: string; ok: boolean; value: string; want: string; why: string };

/** Frames a clip is sampled at, a second. */
export const FPS = 30;
/** The most a loop's seam may turn, against the largest turn inside the clip (`seamTurn`). */
export const SEAM_TURN = 1.5;

type Skeleton = { joints: number[]; body: number; chain: number[]; feet: number[]; restHeight: number };

/** The skin's joints, the body joint (the first that branches, the pelvis of a biped), the chain
 * from the top joint down to it, and the feet (the joints under the body lowest at rest), found by
 * shape rather than by name, so any rig Tripo makes is read alike. Tripo's V3 rig has a `root`
 * joint on the ground above the pelvis, as low as the feet: it is no foot. */
export function skeleton(glb: Glb): Skeleton | undefined {
  const skin = glb.doc.skins?.[0];
  if (!skin) return undefined;
  const joints: number[] = skin.joints;
  const set = new Set(joints);
  const nodes = glb.doc.nodes;
  const world = pose(glb.doc);
  const kids = (j: number) => (nodes[j].children ?? []).filter((c: number) => set.has(c));
  const roots = joints.filter((j) => !joints.some((p) => kids(p).includes(j)));
  let body = roots[0];
  const chain = [body];
  while (kids(body).length === 1) chain.push((body = kids(body)[0]));
  const under: number[] = [];
  const down = (j: number) => kids(j).forEach((c: number) => (under.push(c), down(c)));
  down(body);
  const ys = joints.map((j) => origin(world[j])[1]);
  const low = Math.min(...ys);
  const high = Math.max(...ys);
  const restHeight = high - low;
  const below = under.length ? under : joints;
  const footLow = Math.min(...below.map((j) => ys[joints.indexOf(j)]));
  const feet = below.filter((j) => ys[joints.indexOf(j)] <= footLow + 0.08 * restHeight && kids(j).length <= 1);
  return { joints, body, chain, feet: feet.length ? feet : [below[0]], restHeight };
}

export type Motion = {
  /** How far the body joint ends from where it began, across the ground, in metres. */
  travel: number;
  /** The body's speed over the ground the planted foot gives, in metres a second. */
  speed: number;
  /** Which way it goes, in degrees from +Z (glTF's front), turning toward +X. */
  heading: number;
  /** The most any joint ends from where it began, as a share of the rest height: 0 for a loop
   * that closes. */
  gap: number;
  /** The body joint's height at the end as a share of its height at the start. */
  endHeight: number;
  /** How far the body's mean place across the ground is from its rest place, a share of the
   * rest height: 0 for a body over the origin. */
  offCentre: number;
  /** Played round, the larger step into and out of the seam (the last frame to the first, the
   * first to the second) against the clip's median frame step: a jump at the seam reads over 1. */
  seamStep: number;
  /** The change of velocity across the seam against the largest inside the clip: over 1 is a
   * seam rougher than any frame of the motion (the pose meets, not the velocity). */
  seamTurn: number;
};

const flat = (a: Vec3, b: Vec3) => Math.hypot(a[0] - b[0], a[2] - b[2]);

export function motion(glb: Glb, clip: Clip, sk: Skeleton): Motion {
  const frames = Math.max(2, Math.round(clip.duration * FPS) + 1);
  const at = (f: number) => (f / (frames - 1)) * clip.duration;
  const poses = Array.from({ length: frames }, (_, f) => pose(glb.doc, clip, at(f)));
  const pos = (f: number, j: number) => origin(poses[f][j]);
  const ground = Math.min(...poses.flatMap((_, f) => sk.feet.map((j) => pos(f, j)[1])));
  // The floor the clip starts on: where a fall should end.
  const floor = Math.min(...sk.feet.map((j) => pos(0, j)[1]));
  const steps: [number, number][] = [];
  for (let f = 0; f + 1 < frames; f++) {
    // The planted foot is the slowest of those on the ground, not the lowest: a creature that
    // drags its toes (treant-c's root feet, 2026-11-08) swings a ball lower than the planted one,
    // and the lowest read its walk as going backwards.
    const near = sk.feet.filter((j) => pos(f, j)[1] <= ground + 0.05 * sk.restHeight);
    if (!near.length) continue;
    const foot = near.reduce((a, b) => (flat(pos(f + 1, a), pos(f, a)) <= flat(pos(f + 1, b), pos(f, b)) ? a : b));
    const dt = at(f + 1) - at(f);
    const body = [pos(f + 1, sk.body), pos(f, sk.body)];
    const ft = [pos(f + 1, foot), pos(f, foot)];
    steps.push([(body[0][0] - body[1][0] - (ft[0][0] - ft[1][0])) / dt, (body[0][2] - body[1][2] - (ft[0][2] - ft[1][2])) / dt]);
  }
  const speeds = steps.map(([x, z]) => Math.hypot(x, z)).sort((a, b) => a - b);
  const sum = steps.reduce((s, [x, z]) => [s[0] + x, s[1] + z], [0, 0]);
  const last = frames - 1;
  // The seam, every joint in the world (so a body that jumps counts too), the last frame being
  // the first when played round.
  const apart = (a: number, b: number, c?: number) => Math.max(...sk.joints.map((j) => Math.hypot(...pos(a, j).map((v, i) => (c === undefined ? v - pos(b, j)[i] : v - 2 * pos(b, j)[i] + pos(c, j)[i]))))) / sk.restHeight;
  const stepsIn = Array.from({ length: last }, (_, f) => apart(f, f + 1)).sort((a, b) => a - b);
  const turnsIn = Array.from({ length: Math.max(0, last - 1) }, (_, f) => apart(f, f + 1, f + 2));
  const median = stepsIn[Math.floor(stepsIn.length / 2)] ?? 0;
  const seamStep = last >= 2 ? Math.max(apart(last - 1, 0), apart(0, 1)) / Math.max(1e-6, median) : 0;
  const seamTurn = last >= 2 ? apart(last - 1, 0, 1) / Math.max(1e-6, ...turnsIn) : 0;
  const home = origin(pose(glb.doc)[sk.body]);
  const mean = [0, 2].map((k) => poses.slice(0, last).reduce((s, _, f) => s + pos(f, sk.body)[k] - home[k], 0) / Math.max(1, last));
  return {
    offCentre: Math.hypot(mean[0], mean[1]) / sk.restHeight,
    seamStep,
    seamTurn,
    travel: flat(pos(0, sk.body), pos(last, sk.body)),
    speed: speeds.length ? speeds[Math.floor(speeds.length / 2)] : 0,
    heading: (Math.atan2(sum[0], sum[1]) * 180) / Math.PI,
    gap: Math.max(...sk.joints.map((j) => Math.hypot(...pos(0, j).map((v, i) => v - pos(last, j)[i])))) / sk.restHeight,
    endHeight: (pos(last, sk.body)[1] - floor) / Math.max(1e-6, pos(0, sk.body)[1] - floor),
  };
}

/** `width` and `depth` are the standing pose's (the idle's first frame), across X and Z. */
export type Measures = { height: number; width: number; depth: number; ground: number; triangles: number; textures: number[]; clips: Record<string, { seconds: number; motion?: Motion }> };

export function measure(glb: Glb): Measures {
  const all = clips(glb);
  const sk = skeleton(glb);
  const idle = all.find((c) => c.name === "idle");
  const rest = bounds(points(glb, pose(glb.doc)));
  const standing = idle ? bounds(points(glb, pose(glb.doc, idle, 0))) : rest;
  const out: Measures = {
    height: rest.max[1] - rest.min[1],
    width: standing.max[0] - standing.min[0],
    depth: standing.max[2] - standing.min[2],
    ground: standing.min[1],
    triangles: triangles(glb.doc),
    textures: imageSizes(glb).map((s) => Math.max(s.width, s.height)),
    clips: {},
  };
  for (const c of all) out.clips[c.name] = { seconds: c.duration, motion: sk ? motion(glb, c, sk) : undefined };
  return out;
}

/** Most the moving clips' headings may differ before `export` refuses: twice `check`'s facing
 * bound, so one turn cannot bring both within it. */
export const HEADINGS_APART = 50;

/** Why the moving clips go different ways, or undefined when they agree: `export` turns the
 * model by the walk alone, so a run (or walk) measured the other way would be shipped backwards
 * and found only by `check` (treant-c, 2026-11-08: walk 0, run -170). */
export function headingsDisagree(name: string, m: Measures): string | undefined {
  const moving = Object.entries(m.clips).filter(([, c]) => c.motion && c.motion.speed > 0.2);
  const walk = moving.find(([k]) => k === "walk");
  if (!walk) return undefined;
  const apart = (a: number, b: number) => Math.abs((((a - b) % 360) + 540) % 360 - 180);
  const off = moving.filter(([k, c]) => k !== "walk" && apart(c.motion!.heading, walk[1].motion!.heading) > HEADINGS_APART);
  if (!off.length) return undefined;
  const say = (k: string, c: { motion?: Motion }) => `${k} ${c.motion!.heading.toFixed(0)} deg`;
  return `${name}'s clips go different ways (${[walk, ...off].map(([k, c]) => say(k, c)).join(", ")}; more than ${HEADINGS_APART} deg apart), and export turns the model by the walk alone, so it would ship the others backwards (check.ts motion, the planted foot's heading). Look at each clip in the asset view (\`--asset ${name}\`): if one preset is truly reversed, animate that one again; if the feet are mis-read, fix check.ts motion with a test. Do not turn the model by hand to pass \`check\`.`;
}

/** Every check for an asset of this kind, rigged as `rigType`, made to `height` metres. */
export function check(m: Measures, kind: Kind, rigType: string | undefined, height: number | undefined): Result[] {
  const spec = SPECS[kind];
  const r: Result[] = [];
  const add = (check: string, ok: boolean, value: string, want: string, why: string) => r.push({ check, ok, value, want, why });
  const f = (n: number, d = 2) => n.toFixed(d);

  for (const c of clipsFor(rigType)) {
    const got = m.clips[c.name];
    add(`clip ${c.name}`, Boolean(got), got ? `${f(got.seconds)} s` : "missing", `${c.seconds[0]}-${c.seconds[1]} s`, "the game plays it by this name (from Tripo's preset " + c.preset + ")");
    if (!got) continue;
    const mo = got.motion;
    add(`${c.name} length`, got.seconds >= c.seconds[0] && got.seconds <= c.seconds[1], `${f(got.seconds)} s`, `${c.seconds[0]}-${c.seconds[1]} s`, "a clip far off this is the wrong preset or a broken export");
    if (!mo) continue;
    if (c.loop) add(`${c.name} loops`, mo.gap < 0.05, `${f(mo.gap * 100, 1)}% of height`, "under 5%", "its last frame meets its first, or the body jumps each time round");
    if (c.loop) add(`${c.name} seam`, mo.seamStep < 3 && mo.seamTurn < SEAM_TURN, `step ${f(mo.seamStep, 1)}x median, turn ${f(mo.seamTurn, 1)}x the clip's most`, `step under 3x, turn under ${SEAM_TURN}x`, "played round, the seam must move like any frame: a big step is a snap (a last key at rest), a big turn a velocity that does not meet (normalize.ts loop); re-export, or pick another window");
    if (c.moves) add(`${c.name} centred`, mo.offCentre < 0.03, `body ${f(mo.offCentre * 100, 1)}% of height off`, "under 3%", "the body plays over the entity's place, or the gait runs ahead of its shadow and leaps there from the walk (normalize.ts centre); re-export");
    if (c.moves) {
      add(`${c.name} in place`, mo.travel < 0.1, `${f(mo.travel)} m`, "under 0.10 m", "the game moves the body at the clip's speed; root motion would carry it twice");
      add(`${c.name} speed`, mo.speed > 0.2, `${f(mo.speed)} m/s`, "over 0.20 m/s (recorded for the game)", "how fast the planted foot says the body goes, so the game moves it without sliding the feet");
      add(`${c.name} facing`, Math.abs(mo.heading) < 25, `${f(mo.heading, 0)} deg`, "within 25 deg of +Z", "glTF's front is +Z; the game turns a body by the way it faces");
    }
    if (c.ends === "down") add(`${c.name} ends down`, mo.endHeight < 0.6, `body at ${f(mo.endHeight * 100, 0)}% of its start`, "under 60%", "a death should end on the ground; the nearest preset may not");
  }
  const walk = m.clips.walk?.motion, run = m.clips.run?.motion;
  if (walk && run) add("run faster than walk", run.speed > walk.speed * 1.3, `${f(run.speed)} vs ${f(walk.speed)} m/s`, "run over 1.3x walk", "two gaits the player can tell apart");

  const most = budget(kind, m);
  add("triangles", m.triangles <= most, String(m.triangles), `at most ${most} (${most < spec.triangles ? "one a pixel of its box at the closest camera" : `the ${kind} cap`})`, `past it triangles are under a pixel or over the ${kind}'s share of the frame (kinds.ts, budget); decimate it in Blender to ${most} (free, the models skill) or \`tripo.ts retopo PROJECT --faces ${Math.floor(most / 2)}\` (10 credits, quads)`);
  const big = Math.max(0, ...m.textures);
  add("textures", big > 0 && big <= spec.texture, m.textures.length ? `${m.textures.length}, largest ${big}` : "none", `largest at most ${spec.texture}`, "a figure 100-200 px tall on screen needs no more; `export` asks Tripo for this size");
  add("height", m.height >= spec.height[0] && m.height <= spec.height[1] && (!height || Math.abs(m.height / height - 1) < 0.05), `${f(m.height)} m`, height ? `${height} m (within 5%)` : `${spec.height[0]}-${spec.height[1]} m`, "life size (D110), so it stands right beside the others and the camera frames it");
  add("on the ground", Math.abs(m.ground) <= 0.02 * m.height, `lowest point at ${f(m.ground, 3)} m`, "within 2% of height of y=0", "the game sets its origin on the ground; feet below sink, above float");
  return r;
}

export function printResults(name: string, results: Result[]): string {
  const w = Math.max(...results.map((x) => x.check.length));
  const v = Math.max(...results.map((x) => x.value.length));
  return results.map((x) => `${x.ok ? "✓" : "✗"} ${name} ${x.check.padEnd(w)}  ${x.value.padEnd(v)}  want ${x.want}${x.ok ? "" : `: ${x.why}`}`).join("\n");
}
