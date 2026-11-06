// What `export` does to Tripo's GLB: names each clip as the game asks for it (`attack` for
// Tripo's `slash`…), takes the walk's and run's travel out, cuts a clip to its window, and puts the scene under one root node that makes the asset
// life size, turned to face +Z and standing on y=0 with its body over the origin, the model
// contract's `base` anchor (docs/art-style.md, D303). A parent's transform carries a skinned
// mesh's joints and so its skin, and every clip with them, so nothing else changes.

import { type Glb, type Mat4, apply, bounds, clips, origin, points, pose, readChannel, sample, writeChannel } from "./gltf.ts";
import type { ClipSpec } from "./kinds.ts";
import { skeleton } from "./check.ts";

/** Renames Tripo's clips (`preset:biped:slash`, `slash`, `Slash_…`) to ours; returns what is
 * missing. */
export function renameClips(glb: Glb, specs: ClipSpec[]): string[] {
  const anims: any[] = glb.doc.animations ?? [];
  const missing: string[] = [];
  for (const c of specs) {
    const re = new RegExp(`(^|[^a-z])${c.preset}([^a-z]|$)`, "i");
    const a = anims.find((x) => x.name === c.name) ?? anims.find((x) => re.test(x.name ?? ""));
    if (a) a.name = c.name;
    else missing.push(c.name);
  }
  return missing;
}

/** Takes the travel out of the walk and run: Tripo's in-place export keeps its `root` joint still
 * but the pelvis still carries the clip forward (the knight's walk went 2.8 m, 2026-11-05). Each
 * translation key of the chain from the top joint to the body loses its share of the clip's drift
 * across the ground, end against start, so the sway stays, the loop closes and the feet slide
 * back as the game moves the body (the in-place cut Mixamo's and Unreal's root-motion tools make). */
export function inPlace(glb: Glb, specs: ClipSpec[]): string[] {
  const sk = skeleton(glb);
  if (!sk) return [];
  const world = pose(glb.doc);
  const parent = new Map<number, number>();
  glb.doc.nodes.forEach((n: any, i: number) => (n.children ?? []).forEach((c: number) => parent.set(c, i)));
  const done: string[] = [];
  for (const spec of specs.filter((c) => c.moves)) {
    const anim = (glb.doc.animations ?? []).find((a: any) => a.name === spec.name);
    if (!anim) continue;
    anim.channels.forEach((ch: any, i: number) => {
      if (ch.target?.path !== "translation" || !sk.chain.includes(ch.target.node)) return;
      const { times, values } = readChannel(glb, anim, ch);
      const span = times.at(-1)! - times[0];
      if (span <= 0) return;
      const p = parent.get(ch.target.node);
      const m = p === undefined ? [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1] : world[p];
      const local = values.at(-1)!.map((v, k) => v - values[0][k]);
      const across = apply(m, local).map((v, k) => v - m[12 + k]);
      across[1] = 0; // the drift across the ground, in the world, then back in the parent's frame
      const drift = solve3(m, across);
      writeChannel(glb, anim, i, times, values.map((v, k) => v.map((x, c) => x - drift[c] * ((times[k] - times[0]) / span))));
    });
    done.push(spec.name);
  }
  return done;
}

/** x with M x = b, M the 3x3 linear part of a column-major matrix (Cramer's rule). */
function solve3(m: Mat4, b: number[]): number[] {
  const col = (c: number) => [m[c * 4], m[c * 4 + 1], m[c * 4 + 2]];
  const det = (a: number[], b2: number[], c: number[]) => a[0] * (b2[1] * c[2] - b2[2] * c[1]) - b2[0] * (a[1] * c[2] - a[2] * c[1]) + c[0] * (a[1] * b2[2] - a[2] * b2[1]);
  const [c0, c1, c2] = [col(0), col(1), col(2)];
  const d = det(c0, c1, c2) || 1;
  return [det(b, c1, c2) / d, det(c0, b, c2) / d, det(c0, c1, b) / d];
}

/** Cuts each clip with a `window` to it, in the preset's own seconds, starting at 0. */
export function cut(glb: Glb, specs: ClipSpec[]): string[] {
  const done: string[] = [];
  for (const spec of specs.filter((c) => c.window)) {
    const anim = (glb.doc.animations ?? []).find((a: any) => a.name === spec.name);
    if (!anim) continue;
    cutTo(glb, anim, ...spec.window!);
    done.push(spec.name);
  }
  return done;
}

/** Cuts an animation to seconds `a` to `b`, keys at both ends, starting at 0. */
function cutTo(glb: Glb, anim: any, a: number, b: number): void {
  anim.channels.forEach((ch: any, i: number) => {
    if (ch.target?.node === undefined || ch.target.path === "weights") return;
    const c = readChannel(glb, anim, ch);
    const times = [a, ...c.times.filter((t) => t > a && t < b), b];
    writeChannel(glb, anim, i, times.map((t) => t - a), times.map((t) => sample(c, t)));
  });
}

/** Frames a second the loop search compares: 8 ms apart, finer than the 30 a second a clip is
 * checked at, so the best pair is not a frame off. */
const LOOP_FPS = 120;
/** Ends this close (the most any joint is apart, a share of the rest height) are one cycle
 * already: the clip is kept whole and only closed. Tripo's walks and idles are 0.0 to 0.1%. */
const WHOLE = 0.01;
/** A window whose ends are further apart than this is no cycle: closing it would bend the
 * motion visibly, so the clip is left as it is for `check` to fail. */
const NO_CYCLE = 0.1;

export type Looped = { name: string; from: number; to: number; gap: number };

/** Makes each looping clip a loop, as Unity's importer does with its loop-match light and Loop
 * Pose: Tripo's `run` is 1.25 s of nearly two strides whose ends are 15 to 23% of the height
 * apart (knight, ogre, tree monster, 2026-11-05), while one stride inside it, 0.41 to 1.04 s,
 * meets within 4 to 6%, the fingers the furthest. A clip whose ends meet within `WHOLE` stays
 * whole; any other is cut to the window, at least the spec's shortest length, whose ends match
 * best, the pose compared as each joint's place against the body over the ground, so in-place
 * drift does not count. Then what is left between the ends is spread over the clip, each
 * channel bent toward its first key by its share of the time (translations and scales by
 * difference, rotations by the quaternion between), so the last frame is the first. */
export function loop(glb: Glb, specs: ClipSpec[]): Looped[] {
  const sk = skeleton(glb);
  if (!sk) return [];
  const done: Looped[] = [];
  for (const spec of specs.filter((c) => c.loop)) {
    const anim = (glb.doc.animations ?? []).find((a: any) => a.name === spec.name);
    const clip = clips(glb).find((c) => c.name === spec.name);
    if (!anim || !clip || clip.duration <= 0) continue;
    const at = (t: number) => {
      const w = pose(glb.doc, clip, t);
      const b = origin(w[sk.body]);
      return sk.joints.map((j) => origin(w[j]).map((v, k) => (k === 1 ? v : v - b[k])));
    };
    const apart = (p: number[][], q: number[][]) => Math.max(...p.map((x, k) => Math.hypot(...x.map((v, c) => v - q[k][c])))) / sk.restHeight;
    let best = { from: 0, to: clip.duration, gap: apart(at(0), at(clip.duration)) };
    if (best.gap > WHOLE) {
      const n = Math.floor(clip.duration * LOOP_FPS) + 1;
      const frames = Array.from({ length: n }, (_, f) => at(f / LOOP_FPS));
      const [shortest, longest] = spec.seconds.map((s) => Math.round(s * LOOP_FPS));
      for (let i = 0; i < n; i++)
        for (let j = i + shortest; j < n && j - i <= longest; j++) {
          const gap = apart(frames[i], frames[j]);
          if (gap < best.gap) best = { from: i / LOOP_FPS, to: j / LOOP_FPS, gap };
        }
      if (best.gap > NO_CYCLE) continue;
      cutTo(glb, anim, best.from, best.to);
    }
    close(glb, anim);
    done.push({ name: spec.name, ...best });
  }
  return done;
}

/** Bends each channel of an animation so its last key is its first, by the share of the time. */
function close(glb: Glb, anim: any): void {
  anim.channels.forEach((ch: any, i: number) => {
    if (ch.target?.node === undefined || ch.target.path === "weights") return;
    const { times, values, step } = readChannel(glb, anim, ch);
    const span = times.at(-1)! - times[0];
    if (step || span <= 0) return;
    const share = (k: number) => (times[k] - times[0]) / span;
    const [first, last] = [values[0], values.at(-1)!];
    if (ch.target.path !== "rotation") {
      writeChannel(glb, anim, i, times, values.map((v, k) => v.map((x, c) => x - (last[c] - first[c]) * share(k))));
      return;
    }
    // The turn from the last key back to the first, applied in the parent's frame by its share.
    const back = qmul(first, qconj(last));
    if (back[3] < 0) back.forEach((_, c) => (back[c] = -back[c]));
    const angle = 2 * Math.acos(Math.min(1, back[3]));
    const axis = angle > 1e-9 ? back.slice(0, 3).map((v) => v / Math.sin(angle / 2)) : [0, 0, 1];
    writeChannel(glb, anim, i, times, values.map((v, k) => {
      const h = (angle * share(k)) / 2;
      return qmul([...axis.map((a) => a * Math.sin(h)), Math.cos(h)], v);
    }));
  });
}

const qconj = ([x, y, z, w]: number[]) => [-x, -y, -z, w];
function qmul(a: number[], b: number[]): number[] {
  const [ax, ay, az, aw] = a, [bx, by, bz, bw] = b;
  return [aw * bx + ax * bw + ay * bz - az * by, aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw, aw * bw - ax * bx - ay * by - az * bz];
}

export type Placement = { scale: number; turn: number; lift: number };

/** Adds the root node `name`: scaled to `height` metres (or left as made without one), turned by
 * `heading` degrees back to +Z, lifted so the lowest point of the idle's first frame (or of the
 * rest pose) is on y=0, and moved so the body is over the origin. */
export function place(glb: Glb, name: string, height: number | undefined, heading: number): Placement {
  const { doc } = glb;
  const idle = clips(glb).find((c) => c.name === "idle");
  const rest = bounds(points(glb, pose(doc)));
  const s = height ? height / (rest.max[1] - rest.min[1]) : 1;
  const stand = idle ? bounds(points(glb, pose(doc, idle, 0))) : rest;
  const sk = skeleton(glb);
  const centre = sk ? origin(pose(doc)[sk.body]) : [(rest.min[0] + rest.max[0]) / 2, 0, (rest.min[2] + rest.max[2]) / 2];
  const phi = (-heading * Math.PI) / 180;
  const [sin, cos] = [Math.sin(phi), Math.cos(phi)];
  // T·R·S on the centre: R turns (x, z) to (x cos + z sin, -x sin + z cos).
  const cx = s * centre[0], cz = s * centre[2];
  const translation = [-(cx * cos + cz * sin), -s * stand.min[1], -(-cx * sin + cz * cos)];
  const scene = doc.scenes[doc.scene ?? 0];
  doc.nodes.push({ name, translation, rotation: [0, Math.sin(phi / 2), 0, Math.cos(phi / 2)], scale: [s, s, s], children: scene.nodes });
  scene.nodes = [doc.nodes.length - 1];
  return { scale: s, turn: -heading, lift: translation[1] };
}
