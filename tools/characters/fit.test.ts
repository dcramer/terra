import assert from "node:assert/strict";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { allItems, deepest, lookDiff, makeInputs, movesReads, nockFault, recipeArgs, recipeFault, remedy, runMoves, setOf } from "./fit.ts";
import { closure } from "./fit_cache.ts";

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
  // A strapped shield with no item (the round shield) rides its strap's fist, the left, not
  // the right hand through the walker's use clips (13.8 cm into the body in Interact before).
  const round = setOf("round_shield");
  assert.equal(round.hand, "L");
  for (const clip of ["Interact", "Sword_Attack"]) assert.ok(!round.clips.split(",").includes(clip), clip);
  // A hung basket is carried by its handle in the off hand (D567; its kind's lead), not the right.
  assert.equal(setOf("basket").hand, "L");
});

test("a moves run that writes no rows fails, never reading an earlier run's", async () => {
  const dir = mkdtempSync(join(tmpdir(), "fit-moves-"));
  const json = join(dir, "fit-x-moves.json");
  const log = join(dir, "fit.log");
  writeFileSync(json, JSON.stringify([{ clip: "Old_Clip", measured: { held_cm: 0.1 } }]));
  const failed = await runMoves(["-e", 'console.error("Error: no clip Missing_Clip"); process.exit(1)'], json, log);
  assert.equal(typeof failed, "string");
  assert.match(failed as string, /wrote no rows \(exit 1\): Error: no clip Missing_Clip/);
  assert.ok(!existsSync(json), "the earlier run's rows are gone");
  // Over moves.ts's own bars it writes its rows and exits 1: they are this run's.
  const write = `require("fs").writeFileSync(${JSON.stringify(json)}, '[{"clip":"New_Clip"}]'); process.exit(1)`;
  assert.deepEqual(await runMoves(["-e", write], json, log), [{ clip: "New_Clip" }]);
  assert.match(readFileSync(log, "utf8"), /no clip Missing_Clip/);
  rmSync(dir, { recursive: true, force: true });
});

const ROOT = join(import.meta.dirname, "..", "..");
const rel = (fs: string[]) => fs.map((f) => f.slice(ROOT.length + 1));

test("--all is every item with data, held and worn", () => {
  const all = allItems();
  for (const name of ["arming_sword", "bow", "heater_shield", "stone_axe", "torch", "quiver", "small_pack"]) assert.ok(all.includes(name), name);
  assert.deepEqual(all, [...all].sort());
  assert.equal(new Set(all).size, all.length);
});

test("a make is keyed on every script its maker reaches and the item's data and source", () => {
  const data = JSON.parse(readFileSync(join(ROOT, "art", "held", "stone_pickaxe.json"), "utf8"));
  const held = rel(makeInputs({ name: "stone_pickaxe", worn: false, file: join(ROOT, "art", "held", "stone_pickaxe.json"), data }));
  // fit_held.py imports look.py and haft.py: an edit to either makes it again.
  for (const f of ["tools/characters/fit_held.py", "tools/characters/look.py", "tools/characters/haft.py", "tools/blender/version", "art/held/stone_pickaxe.json", `assets/models/${data.source}`]) assert.ok(held.includes(f), f);
  // A worn item's maker runs build.ts, whose steps name the Python scripts they run.
  const worn = rel(makeInputs({ name: "quiver", worn: true, file: join(ROOT, "art", "gear", "quiver", "fit.json"), data: {} }));
  for (const f of ["tools/characters/build_item.sh", "tools/characters/build.ts", "tools/characters/fit_gear.py", "tools/characters/gear_check.py", "art/gear/quiver/fit.json", "art/gear/small_pack/fit.json"]) assert.ok(worn.includes(f), f);
});

test("an assembled look is made by the item writer from its recipe, keyed on its parts, and gives no fitted look's numbers", () => {
  const file = join(ROOT, "art", "held", "iron_axe.json");
  const data = JSON.parse(readFileSync(file, "utf8"));
  assert.equal(recipeFault(data), undefined);
  assert.match(recipeFault({ ...data, length: 0.68 })!, /length comes from its parts and kind \(D565\): delete it/);
  assert.match(recipeFault({ assembled: "axe", parts: [] })!, /"parts" \(part ids\) and "material"/);
  assert.deepEqual(recipeArgs({ assembled: "axe", parts: ["h", "s"], material: "iron" }, "o.glb").slice(4), ["--bin", "item", "--", "axe", "h", "s", "--material", "iron", "--out", "o.glb"]);
  const reads = rel(makeInputs({ name: "iron_axe", worn: false, file, data }));
  for (const f of ["art/held/iron_axe.json", "crates/terra_items/src/assembly/meet.rs", "crates/terra_bake/src/item.rs", "assets/data/kinds/axe.json", "assets/data/parts/ash_haft.json", "assets/models/parts/bearded_axe_head.glb"]) assert.ok(reads.includes(f), f);
  assert.ok(!reads.includes("tools/characters/fit_held.py"), "no Blender in an assembly");
});

test("a move check is keyed on the files its scripts read, not only on its scripts", () => {
  const reads = rel(movesReads(closure(join(ROOT, "tools", "characters", "moves.ts"), ROOT)));
  // Its defaults (the body, the clips the carry poses are in, the templates) and the walker's
  // carried clips: a new clip or body runs it again.
  for (const f of ["assets/models/character/base.glb", "assets/models/character/fight.glb", "assets/models/character/templates.json", "art/animation/sets/fight.json", "crates/terra_walker/src/character/held.rs"]) assert.ok(reads.includes(f), f);
  for (const f of reads) assert.ok(existsSync(join(ROOT, f)), `${f} is read but not there`);
});

test("a look's rows are merged over the bodies, each at its deepest and where it went in", () => {
  const k = "held_cm.stone_axe";
  const male = [{ clip: "Walk_Loop", measured: { [k]: 8.8 }, where: { [k]: "its point (-1, 15, 0) cm in thigh_r in Walk_Loop at 0.60 s, male" } }, { clip: "Idle_Loop", measured: { [k]: -3 } }, { clip: "fist", measured: {} }];
  const heavy = [{ clip: "Walk_Loop", measured: { [k]: 9.4 }, where: { [k]: "its point (-2, 14, 0) cm in thigh_r in Walk_Loop at 0.53 s, heavy" } }, { clip: "Idle_Loop", measured: { [k]: -5 } }];
  const deep = deepest([male, heavy], "stone_axe").sort((a, b) => a.row.localeCompare(b.row));
  assert.deepEqual(deep, [
    { row: "Idle_Loop", cm: -3, where: "" },
    { row: "Walk_Loop", cm: 9.4, where: "its point (-2, 14, 0) cm in thigh_r in Walk_Loop at 0.53 s, heavy" },
  ]);
  assert.deepEqual(deepest([male], "knife"), []);
});

test("a check fault names the field of its data and the value that sets it right", () => {
  const it = { name: "hammer", file: join(ROOT, "art", "held", "hammer.json"), data: { kind: "hafted", fit: "length", turn: -90 } };
  const fault = (rule: string, detail: string) => ({ look: "held/hammer", rule, detail });
  // The contract's own words for a head leaning the wrong way, and for one lying across X.
  const contract = readFileSync(join(ROOT, "tools", "characters", "attach_contract.ts"), "utf8");
  assert.ok(contract.includes("it leans ") && contract.includes("it lies across X"), "the contract's facing faults say how the head lies");
  assert.equal(remedy(fault("facing", "its working end reaches 3.0 cm toward +Z and 0.5 cm toward -Z: it leans +Z, not -Z"), it), 'set "turn" 90 in art/held/hammer.json');
  assert.equal(remedy(fault("facing", "its working end is 6.0 cm across X and 1.0 cm across Z (...): it lies across X, not in the Y-Z plane"), it), 'set "turn" 0 or -180 in art/held/hammer.json, whichever puts its working end toward -Z');
  // The detector's turn, when it has one, is the value to set.
  assert.equal(remedy(fault("facing", "...: it lies across X, not in the Y-Z plane"), it, { turn: 0 }), 'set "turn" 0 in art/held/hammer.json (the detector\'s)');
  assert.equal(remedy(fault("grip", "nothing crosses y = 0: the grip is not at the origin"), it, { grip_from_end: 0.07 }), 'set "grip_from_end" 0.07 in art/held/hammer.json (the detector\'s)');
  assert.equal(remedy(fault("hold", "its item's hold.length is 0.62 m (items.ron), its look 0.55 m"), it), 'set "length" 0.62 in art/held/hammer.json, or its item\'s hold.length in items.ron to its look\'s');
  assert.match(remedy(fault("stale", "its source has changed since its look was made"), it), /fit\.ts hammer --fresh/);
});

test("a bow's data nock is checked against its look's string", () => {
  const bow = JSON.parse(readFileSync(join(ROOT, "art", "held", "bow.json"), "utf8"));
  const it = { name: "bow", look: join(ROOT, "assets", "models", "held", "bow.glb") };
  assert.equal(nockFault({ ...it, data: bow }), undefined, "the committed bow's nock is its string's");
  const moved = [bow.nock[0], bow.nock[1] + 0.02, bow.nock[2]];
  const f = nockFault({ ...it, data: { ...bow, nock: moved } })!;
  assert.match(f.detail, /2\.0 cm off its string's middle/);
  assert.equal(remedy(f, { name: "bow", file: join(ROOT, "art", "held", "bow.json"), data: bow }), `set "nock" [${bow.nock.join(", ")}] in art/held/bow.json`);
  // A look with no string, given a nock, is a fault; an item with none is not checked.
  assert.match(nockFault({ name: "knife", look: join(ROOT, "assets", "models", "held", "knife.glb"), data: { nock: [0, 0, 0] } })!.detail, /no String/);
  assert.equal(nockFault({ name: "knife", look: join(ROOT, "assets", "models", "held", "knife.glb"), data: {} }), undefined);
});
