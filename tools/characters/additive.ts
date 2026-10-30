// The carried arm's swing, as additive clips (D584, docs/animation.md step 3): each stride loop
// of art/animation/swings.json gives, for each arm, its listed bones' rotations less the loop's
// own mean pose of them (Unreal's additive clip, "local space" against a reference pose; its
// delta taken on the left, `q * mean⁻¹`, as Unreal's and Bevy's additive blends accumulate it),
// written into the file as `<loop>.Arm_L_Add` and `<loop>.Arm_R_Add`. The game plays a loop's
// clip on an arm carrying in a pose under an additive blend node, in step with the loop and at
// the pose's share (its set's `swing`, written here into the extras as `swing`), so a carried
// hammer swings as the stride's arm does, a lit torch less (terra_walker::layer, figure). Ported
// from ALS v4's overlay layering: its `Arm_R_Add` curve adds the locomotion's arm (the base
// layer less its base pose) over the overlay's pose, at a weight each overlay pose sets.
//
//   node tools/characters/additive.ts FILE.glb... [--print]   (after mirror.ts, before markers.ts)

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { type Clip, Glb, type Q, qinv, qmul } from "./glb.ts";
import { compact, glbBytes } from "./glb_clips.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const SWINGS = resolve(ROOT, "art/animation/swings.json");
export const SET = resolve(ROOT, "art/animation/sets/fight.json");

/** The additive clip of `loop` on the arm of `side` ("L" or "R"). Its twin is
 * terra_walker::layer::swing_of. */
export const additiveName = (loop: string, side: "L" | "R") => `${loop}.Arm_${side}_Add`;
export const isAdditive = (clip: string) => /\.Arm_[LR]_Add$/.test(clip);

const dot = (a: Q, b: Q) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
const unit = (q: Q): Q => {
  const n = Math.hypot(...q);
  return q.map((x) => x / n) as Q;
};

/** The mean of rotations near each other: their sum on one side of the sphere, normalised. */
export function meanOf(qs: Q[]): Q {
  const sum: Q = [0, 0, 0, 0];
  for (const q of qs) {
    const s = dot(q, qs[0]) < 0 ? -1 : 1;
    for (let k = 0; k < 4; k++) sum[k] += s * q[k];
  }
  return unit(sum);
}

/** Each rotation less `mean`, taken on the left (`q * mean⁻¹`), each on the side of the last so
 * the keys blend the short way. */
export function lessMean(qs: Q[], mean: Q): Q[] {
  let prev: Q = [0, 0, 0, 1];
  return qs.map((q) => {
    const d = unit(qmul(q, qinv(mean)));
    const v = (dot(d, prev) < 0 ? d.map((x) => -x) : d) as Q;
    prev = v;
    return v;
  });
}

/** Degrees a rotation turns. */
export const degrees = (q: Q) => (2 * Math.acos(Math.min(1, Math.abs(q[3]))) * 180) / Math.PI;

/** `share` of a rotation: the slerp from none to `q`. */
export function partOf(q: Q, share: number): Q {
  const s = q[3] < 0 ? -1 : 1;
  const half = Math.acos(Math.min(1, s * q[3]));
  if (half < 1e-9) return [0, 0, 0, 1];
  const k = Math.sin(share * half) / Math.sin(half);
  return [s * q[0] * k, s * q[1] * k, s * q[2] * k, Math.cos(share * half)];
}

/** Each carry pose's share of the stride's swing, as the clips file `body` carries it (its
 * extras' `swing`, which the game reads). */
export const sharesIn = (body: Glb): Record<string, number> =>
  (body.gltf.nodes.map((n) => (n as { extras?: { swing?: Record<string, number> } }).extras?.swing).find(Boolean) ?? {});

/** `clip` with its additive swing on the arm of `side` added at `share`, as the game's graph adds
 * it (terra_walker::layer): each swung bone's rotation `partOf(delta, share) * rotation`, at the
 * same time. The clip as it was where the file has no swing for it. */
export function swung(body: Glb, clip: Clip, side: "L" | "R", share: number): Clip {
  const add = body.clip(additiveName(clip.name, side));
  if (!add || share <= 0) return clip;
  const tracks = new Map(clip.tracks);
  for (const [bone, track] of add.tracks) {
    const delta = track.r;
    const under = clip.tracks.get(bone);
    if (!delta || !under?.r) continue;
    const r = under.r;
    tracks.set(bone, { ...under, r: (t: number) => qmul(partOf(delta(t) as Q, share), r(t) as Q) });
  }
  return { ...clip, tracks };
}

type Swings = { from: string[]; bones: string[] };
type Anim = { name: string; channels: { sampler: number; target: { node: number; path: string } }[]; samplers: { input: number; output: number; interpolation?: string }[] };

/** `file`'s bytes with each listed loop it has made again as its arms' additive clips, and each
 * carry pose's share (`swing`) in the extras beside the paces; and the most each clip turns. */
export function withAdditives(file: string, swings: Swings, shares: Record<string, number>): { bytes: Buffer; widest: Record<string, number> } {
  const body = new Glb(file);
  const bytes = readFileSync(file);
  const jsonLen = bytes.readUInt32LE(12);
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  const bin = bytes.subarray(28 + jsonLen, 28 + jsonLen + binLen);
  const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString()) as Glb["gltf"] & {
    animations: Anim[];
    buffers: { byteLength: number }[];
    bufferViews: { buffer: number; byteOffset?: number; byteLength: number }[];
    nodes: { name?: string; extras?: Record<string, unknown> }[];
  };
  gltf.animations = gltf.animations.filter((a) => !isAdditive(a.name));
  const extra: Buffer[] = [];
  let offset = bin.length;
  const push = (values: number[], type: string, count: number, minMax = false): number => {
    const buf = Buffer.alloc(values.length * 4);
    values.forEach((v, i) => buf.writeFloatLE(v, i * 4));
    gltf.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: buf.length });
    extra.push(buf);
    offset += buf.length;
    const acc: Glb["gltf"]["accessors"][number] & { min?: number[] } = { bufferView: gltf.bufferViews.length - 1, componentType: 5126, count, type };
    if (minMax) Object.assign(acc, { min: [Math.min(...values)], max: [Math.max(...values)] });
    gltf.accessors.push(acc);
    return gltf.accessors.length - 1;
  };
  const widest: Record<string, number> = {};
  for (const loop of swings.from) {
    const source = gltf.animations.find((a) => a.name === loop);
    const clip = body.clip(loop);
    if (!source || !clip) continue;
    // Keyed on the loop's longest keys, so the two play in step at one seek time.
    const keys = source.samplers.map((s) => s.input).reduce((a, b) => (gltf.accessors[b].count > gltf.accessors[a].count ? b : a));
    const times = Array.from(body.read(keys));
    const input = push(times, "SCALAR", times.length, true);
    for (const side of ["L", "R"] as const) {
      const anim: Anim = { name: additiveName(loop, side), channels: [], samplers: [] };
      for (const bone of swings.bones) {
        const name = `${bone}_${side.toLowerCase()}`;
        const node = body.named(name);
        const track = clip.tracks.get(name)?.r;
        if (node < 0) throw new Error(`${file}: no bone ${name} (art/animation/swings.json's bones)`);
        if (!track) continue;
        const qs = times.map((t) => unit(track(t) as Q));
        const deltas = lessMean(qs, meanOf(qs));
        widest[anim.name] = Math.max(widest[anim.name] ?? 0, ...deltas.map(degrees));
        anim.samplers.push({ input, output: push(deltas.flat(), "VEC4", times.length), interpolation: "LINEAR" });
        anim.channels.push({ sampler: anim.samplers.length - 1, target: { node, path: "rotation" } });
      }
      if (anim.channels.length) gltf.animations.push(anim);
    }
  }
  const carrier = gltf.nodes.find((n) => n.extras?.paces) ?? gltf.nodes[0];
  carrier.extras = { ...(carrier.extras ?? {}), swing: shares };
  gltf.buffers[0].byteLength = offset;
  // The clips it replaced dropped from the buffer, so baking again gives the same bytes (D563).
  return { bytes: compact(glbBytes(gltf, Buffer.concat([bin, ...extra]))), widest };
}

/** Each carry pose of the set with a `swing`, its share of the stride's arm. */
export function sharesOf(set: Record<string, { swing?: number }>): Record<string, number> {
  return Object.fromEntries(Object.entries(set).flatMap(([pose, spec]) => (typeof spec?.swing === "number" ? [[pose, spec.swing]] : [])));
}

/** Bakes `file`'s arm swings in place from the data files; whether it changed, and the most each
 * swing turns off its loop's mean, degrees. */
export function additiveFile(file: string): { same: boolean; widest: Record<string, number> } {
  const swings: Swings = JSON.parse(readFileSync(SWINGS, "utf8"));
  const shares = sharesOf(JSON.parse(readFileSync(SET, "utf8")));
  const { bytes, widest } = withAdditives(file, swings, shares);
  const same = Buffer.compare(bytes, readFileSync(file)) === 0;
  if (!same) writeFileSync(file, bytes);
  return { same, widest };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  const files = args.filter((a) => !a.startsWith("--"));
  if (!files.length) {
    console.log("additive.ts FILE.glb... [--print]   (art/animation/swings.json's loops; then markers.ts)");
    process.exit(1);
  }
  for (const file of files) {
    const { same, widest } = additiveFile(file);
    console.log(`additive: ${file}, ${Object.keys(widest).length} arm swings ${same ? "as baked" : "baked"}`);
    if (args.includes("--print")) for (const [clip, deg] of Object.entries(widest)) console.log(`  ${clip}: widest ${deg.toFixed(1)} deg off its mean`);
  }
}
