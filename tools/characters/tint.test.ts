// node --test tools/characters/tint.test.ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { tinted } from "./tint.ts";

/** A .glb of `gltf` with `bin` as its binary chunk. */
function glb(gltf: object, bin: Buffer): Buffer {
  let json = Buffer.from(JSON.stringify(gltf));
  if (json.length % 4) json = Buffer.concat([json, Buffer.alloc(4 - (json.length % 4), 0x20)]);
  const binHead = Buffer.alloc(8);
  binHead.writeUInt32LE(bin.length, 0);
  binHead.write("BIN\0", 4);
  const head = Buffer.alloc(20);
  head.write("glTF", 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(20 + json.length + 8 + bin.length, 8);
  head.writeUInt32LE(json.length, 12);
  head.write("JSON", 16);
  return Buffer.concat([head, json, binHead, bin]);
}

test("every material takes the colour, keeping its alpha, and the binary chunk is kept", () => {
  const bin = Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]);
  const made = glb({ asset: { version: "2.0" }, materials: [{ name: "skin", pbrMetallicRoughness: { baseColorFactor: [1, 1, 1, 0.5] } }, { name: "bare" }] }, bin);
  const out = tinted(made, [0.8, 0.74, 0.66]);
  const jsonLen = out.readUInt32LE(12);
  assert.equal(jsonLen % 4, 0);
  assert.equal(out.readUInt32LE(8), out.length);
  const gltf = JSON.parse(out.subarray(20, 20 + jsonLen).toString());
  assert.deepEqual(gltf.materials[0].pbrMetallicRoughness.baseColorFactor, [0.8, 0.74, 0.66, 0.5]);
  assert.deepEqual(gltf.materials[1].pbrMetallicRoughness.baseColorFactor, [0.8, 0.74, 0.66, 1]);
  assert.deepEqual(out.subarray(out.length - bin.length), bin);
});
