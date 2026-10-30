import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { lookDiff, runMoves, setOf } from "./fit.ts";

/** A GLB of one node with these extras over this binary chunk. */
function glb(extras: Record<string, unknown>, bin: number[], mesh = "Mesh_0"): Buffer {
  let json = Buffer.from(JSON.stringify({ asset: { version: "2.0" }, nodes: [{ mesh: 0, extras }], meshes: [{ name: mesh }] }));
  json = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 0x20)]);
  const body = Buffer.from(bin);
  const head = Buffer.alloc(20);
  head.writeUInt32LE(0x46546c67, 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(20 + json.length + 8 + body.length, 8);
  head.writeUInt32LE(json.length, 12);
  head.writeUInt32LE(0x4e4f534a, 16);
  const binHead = Buffer.alloc(8);
  binHead.writeUInt32LE(body.length, 0);
  binHead.writeUInt32LE(0x004e4942, 4);
  return Buffer.concat([head, json, binHead, body]);
}

test("a look made again says which of its parts changed", () => {
  const was = glb({ out: 0.12 }, [1, 2, 3, 4]);
  assert.equal(lookDiff(was, was), "the committed file's bytes");
  assert.equal(lookDiff(glb({ out: 0.12, source_sha: "0dd80ed77a80" }, [1, 2, 3, 4]), was), 'its binary chunk the same; extras source_sha "0dd80ed77a80" added');
  assert.equal(lookDiff(glb({ out: 0.2 }, [1, 2, 3, 5, 6, 7, 8, 9]), was), "its binary chunk changed (8 bytes, was 4); extras out 0.2, was 0.12");
  // Made in a scene of its own, Blender names its mesh with another suffix.
  assert.equal(lookDiff(glb({ out: 0.12 }, [1, 2, 3, 4], "Mesh_0.001"), was), "its binary chunk the same; its glTF meshes otherwise changed");
});

test("a held item is checked in its class's set, else in the walker's clips", () => {
  // The sword and the shield fight in the one set there is; the shield in the off hand.
  assert.deepEqual(setOf("arming_sword"), { set: "assets/data/moves/sword.shielded.ron", hand: "R", clips: "" });
  assert.deepEqual(setOf("heater_shield"), { set: "assets/data/moves/sword.shielded.ron", hand: "L", clips: "" });
  // An axe has no set: every clip the game carries it in, and those it uses it in.
  const axe = setOf("iron_axe");
  assert.equal(axe.set, "none");
  assert.equal(axe.hand, "R");
  for (const clip of ["Idle_Loop", "Walk_Loop", "Crouch_Fwd_Loop", "Jump_Land", "Interact"]) assert.ok(axe.clips.split(",").includes(clip), clip);
  // A bow leads with the off hand (its kind's lead), carried, never swung as a sword.
  const bow = setOf("bow");
  assert.equal(bow.hand, "L");
  assert.ok(bow.clips.split(",").includes("Sprint_Loop"));
  for (const clip of ["Interact", "Sword_Attack", "Sword_Idle"]) assert.ok(!bow.clips.split(",").includes(clip), clip);
});

test("a moves run that writes no rows fails, never reading an earlier run's", () => {
  const dir = mkdtempSync(join(tmpdir(), "fit-moves-"));
  const json = join(dir, "fit-x-moves.json");
  const log = join(dir, "fit.log");
  writeFileSync(json, JSON.stringify([{ clip: "Old_Clip", measured: { held_cm: 0.1 } }]));
  const failed = runMoves(["-e", 'console.error("Error: no clip Missing_Clip"); process.exit(1)'], json, log);
  assert.equal(typeof failed, "string");
  assert.match(failed as string, /wrote no rows \(exit 1\): Error: no clip Missing_Clip/);
  assert.ok(!existsSync(json), "the earlier run's rows are gone");
  // Over moves.ts's own bars it writes its rows and exits 1: they are this run's.
  const write = `require("fs").writeFileSync(${JSON.stringify(json)}, '[{"clip":"New_Clip"}]'); process.exit(1)`;
  assert.deepEqual(runMoves(["-e", write], json, log), [{ clip: "New_Clip" }]);
  assert.match(readFileSync(log, "utf8"), /no clip Missing_Clip/);
  rmSync(dir, { recursive: true, force: true });
});
