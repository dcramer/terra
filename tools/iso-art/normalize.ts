// What `export` does to Tripo's GLB, in its JSON alone: names each clip as the game asks for it
// (`attack` for Tripo's `slash`…) and puts the scene under one root node that makes the asset
// life size, turned to face +Z and standing on y=0 with its body over the origin, the model
// contract's `base` anchor (docs/art-style.md, D303). A parent's transform carries a skinned
// mesh's joints and so its skin, and every clip with them, so nothing else changes.

import { type Glb, bounds, clips, origin, points, pose } from "./gltf.ts";
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
