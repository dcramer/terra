import { test } from "node:test";
import assert from "node:assert/strict";
import { Glb } from "./glb.ts";

/// A .glb of the JSON and binary chunk given, each padded to 4 bytes as the format asks.
function glb(json: unknown, bin: Buffer): Buffer {
  const pad = (b: Buffer, fill: number) => (b.length % 4 ? Buffer.concat([b, Buffer.alloc(4 - (b.length % 4), fill)]) : b);
  const chunk = (body: Buffer, kind: string) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(body.length, 0);
    h.write(kind, 4, "latin1");
    return Buffer.concat([h, body]);
  };
  const body = Buffer.concat([chunk(pad(Buffer.from(JSON.stringify(json)), 0x20), "JSON"), chunk(pad(bin, 0), "BIN\0")]);
  const head = Buffer.alloc(12);
  head.write("glTF", 0, "latin1");
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + body.length, 8);
  return Buffer.concat([head, body]);
}

test("a sparse accessor reads as its base with its listed elements replaced", () => {
  // A shape key's offsets as Blender writes them: no buffer view (zeros), vertex 1 moved.
  const bin = Buffer.alloc(16);
  bin.writeUInt16LE(1, 0);
  [1, 2, 3].forEach((v, i) => bin.writeFloatLE(v, 4 + 4 * i));
  const g = new Glb(
    glb(
      {
        asset: { version: "2.0" },
        nodes: [],
        buffers: [{ byteLength: 16 }],
        bufferViews: [
          { buffer: 0, byteOffset: 0, byteLength: 2 },
          { buffer: 0, byteOffset: 4, byteLength: 12 },
        ],
        accessors: [{ componentType: 5126, count: 3, type: "VEC3", sparse: { count: 1, indices: { bufferView: 0, componentType: 5123 }, values: { bufferView: 1 } } }],
      },
      bin,
    ),
  );
  assert.deepEqual([...g.read(0)], [0, 0, 0, 1, 2, 3, 0, 0, 0]);
});
