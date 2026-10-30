#!/usr/bin/env node
// Lays a colour factor on every material of a .glb, multiplying its base colour texture, as
// glTF's baseColorFactor does: a creature's colour as its design wants it in the game's light
// (the troll's darker and warmer than Tripo's pale grey, 1e1a8615), set on the exported file so
// a build gives it every time (bodies.json `creatures`, build_steps.ts).
//
//   node tools/characters/tint.ts FILE.glb R,G,B
//
// Rewrites only the file's JSON chunk; its binary chunk is kept byte for byte. Node 22.18 or
// later; no dependencies.

import { readFileSync, writeFileSync } from "node:fs";
import { glbBytes } from "./glb_clips.ts";

/** The .glb's bytes with every material's base colour factor set to `rgb` (alpha kept). */
export function tinted(bytes: Buffer, rgb: [number, number, number]): Buffer {
  const jsonLen = bytes.readUInt32LE(12);
  const gltf = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString());
  for (const m of gltf.materials ?? []) {
    const pbr = (m.pbrMetallicRoughness ??= {});
    const alpha = pbr.baseColorFactor?.[3] ?? 1;
    pbr.baseColorFactor = [...rgb, alpha];
  }
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  return glbBytes(gltf, bytes.subarray(28 + jsonLen, 28 + jsonLen + binLen));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [file, colour] = process.argv.slice(2);
  const rgb = colour?.split(",").map(Number);
  if (!file || rgb?.length !== 3 || rgb.some((v) => !(v >= 0 && v <= 1))) {
    console.error("tint: wants a .glb and a colour as R,G,B, each 0 to 1, like `node tools/characters/tint.ts assets/models/characters/troll.glb 0.8,0.74,0.66`");
    process.exit(2);
  }
  writeFileSync(file, tinted(readFileSync(file), rgb as [number, number, number]));
  console.log(`TINT ${file} ${rgb.join(",")}`);
}
