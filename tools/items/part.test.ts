import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { box, boxHead, collar, collarMesh, glbOf, haft, head } from "./fixtures.ts";
import { type Mesh, normal, readMesh, triangles } from "./mesh.ts";
import { checkPart, type Ctx, loadCtx, withMeasured } from "./part.ts";

// The real types, kinds and mounts, with only the made-up parts.
function ctx(parts: any[], meshes: Record<string, Mesh>): Ctx {
  const real = loadCtx();
  return { ...real, parts: Object.fromEntries(parts.map((p) => [p.part, p])), mesh: (p) => meshes[p.part] };
}
const faults = (part: any, c: Ctx, stage?: string) => checkPart(part, c).filter((s) => !stage || s.stage === stage).flatMap((s) => s.faults);
const good = () => ctx([head(), haft(), collar()], { test_box_head: boxHead(), test_collar: collarMesh() });

test("the made-up head is what its numbers say: its top faces up, its eye round the line", () => {
  const top = triangles(boxHead()).filter((t) => t.every((p) => p[1] === 0.06));
  assert.ok(top.length > 0 && top.every((t) => normal(t)[1] > 0.99));
});

test("a good head, haft and collar pass every stage", () => {
  const c = good();
  for (const p of Object.values(c.parts)) assert.deepEqual(checkPart(p, c).filter((s) => s.faults.length), [], p.part);
});

test("--write measures a box's volume, centre and bounds, and the checker then holds the data to its model", () => {
  // A 4 x 6 x 10 cm box from (-0.02, 0, -0.05): 2.4e-4 m^3, its centre the box's middle.
  const b = box([-0.02, 0, -0.05], [0.02, 0.06, 0.05]);
  const bare = head();
  for (const k of ["volume_m3", "centre_m", "bounds_m"]) delete bare[k];
  const c = ctx([bare], { test_box_head: b });
  assert.ok(faults(bare, c, "data").some((s) => /\/centre_m is missing.*--write test_box_head measures it/.test(s)));
  const w = withMeasured(bare, c);
  assert.ok(Math.abs(w.volume_m3.value - 2.4e-4) < 1e-9, `${w.volume_m3.value}`);
  assert.match(w.volume_m3.source, /measured on models\/parts\/test_box_head\.glb/);
  assert.deepEqual(w.centre_m, [0, 0.03, 0]);
  assert.deepEqual(w.bounds_m, [[-0.02, 0, -0.05], [0.02, 0.06, 0.05]]);
  assert.deepEqual(faults(w, c, "measured"), []);
  // Data typed off its model is refused at each field, the fix being --write.
  const typed = { ...w, centre_m: [0, 0.035, 0], volume_m3: { value: 2.5e-4, source: "typed in by hand" } };
  const f = faults(typed, c, "measured");
  assert.ok(f.some((s) => /\/centre_m is 5\.0 mm from the model's centre of mass: run node tools\/items\/part\.ts --write/.test(s)) && f.some((s) => /\/volume_m3 is 0\.00025, the model's 0\.00024/.test(s)), f.join("\n"));
  // A box with a face missing closes nothing: refused, not measured.
  const open = { ...b, indices: b.indices.slice(6) };
  assert.throws(() => withMeasured(bare, ctx([bare], { test_box_head: open })), /not a closed surface facing out \(4 edges/);
  // A trim gives its own mass, so only its centre and bounds are measured.
  assert.deepEqual(Object.keys(withMeasured(collar(), ctx([collar()], { test_collar: collarMesh() }))).filter((k) => k === "volume_m3"), []);
});

test("a point off the line is refused at its field", () => {
  const h = head();
  h.points[1].at = [0.005, 0.06, 0];
  const f = faults(h, ctx([h, haft()], { test_box_head: boxHead() }), "points");
  assert.ok(f.some((s) => /eye\.upper\.at is 5\.0 mm off the line/.test(s)), f.join("\n"));
});

test("a point of the wrong type, and a missing one, are refused", () => {
  const h = head();
  h.points[0].type = "socket";
  h.points.pop();
  const f = faults(h, ctx([h], { test_box_head: boxHead() }), "points");
  assert.ok(f.some((s) => /eye\.lower is a socket/.test(s)) && f.some((s) => /no edge\.upper/.test(s)), f.join("\n"));
});

test("an eye wider than its points say is a gap round the haft", () => {
  const f = faults(head(), ctx([head(), haft()], { test_box_head: boxHead(0.04) }), "openings");
  assert.ok(f.some((s) => /2\.5 mm gap round the handle/.test(s)), f.join("\n"));
});

test("an eye narrower than its points say buries the haft", () => {
  const f = faults(head(), ctx([head(), haft()], { test_box_head: boxHead(0.03) }), "openings");
  assert.ok(f.some((s) => /buried 2\.5 mm/.test(s)), f.join("\n"));
});

test("a scaled part is refused, read from its glb", () => {
  const dir = mkdtempSync(join(tmpdir(), "part-"));
  const file = join(dir, "head.glb");
  writeFileSync(file, glbOf(boxHead(), [1.2, 1.2, 1.2]));
  const mesh = readMesh(file);
  assert.deepEqual(mesh.scaled, ["head"]);
  const f = faults(head(), ctx([head()], { test_box_head: mesh }), "scale");
  assert.match(f[0], /parts are never scaled \(D566\)/);
});

test("a head outside its kind's envelope in every material is refused with each mass and the limit", () => {
  const h = head({ volume_m3: { value: 2e-4, source: "made up for the part checker's tests" } });
  const c = ctx([h], { test_box_head: boxHead() });
  const f = faults(h, c, "slots");
  // 2e-4 m^3 is over 1.5 kg of iron and about half a kilo of stone: over the band in one, under it in the other.
  const kg = (m: string) => (2e-4 * c.materials[m].density_kg_m3.value).toFixed(3);
  const said = `axe: in no material is its head inside the axe's head mass [0.656,1.28] kg (iron ${kg("iron")} kg, stone ${kg("stone")} kg`;
  assert.ok(f.some((s) => s.startsWith(said)), `${said}\n${f.join("\n")}`);
  assert.deepEqual(faults(head(), good(), "slots"), [], "the box head, 1.25 kg in iron, is inside it");
});

test("a working part gives its volume, not a mass, and a trim its mass", () => {
  const h = head({ mass_kg: { value: 0.9, source: "made up for the part checker's tests" } });
  assert.ok(faults(h, ctx([h], { test_box_head: boxHead() }), "data").some((s) => /\/mass_kg is not a working part's: its mass is its volume_m3 in the recipe's material/.test(s)));
  const c = collar();
  delete c.mass_kg;
  assert.ok(faults(c, ctx([c], { test_collar: collarMesh() }), "data").some((s) => /\/mass_kg is missing: a modelled trim part gives it/.test(s)));
});

test("a haft too long for its kind, or its grip off its mount's range, is refused from its numbers", () => {
  const long = haft({ length_m: { value: 0.8, source: "made up for the part checker's tests" } });
  assert.ok(faults(long, ctx([long], {}), "slots").some((s) => /axe: 0\.8 m long, past the axe's longest item 0\.748 m/.test(s)));
  const off = haft({ grip_from_end_m: { value: 0.2, source: "made up for the part checker's tests" } });
  assert.ok(faults(off, ctx([off], {}), "slots").some((s) => /grip 0\.2 m from the butt is outside a hafted look's \[0\.04,0\.12\]/.test(s)));
});

test("a haft that fits many kinds must name the ones it is made for", () => {
  const h = haft();
  delete h.kinds;
  assert.match(faults(h, ctx([h], {}), "data")[0], /\/kinds is missing: a haft fits axe, hammer/);
});

test("the kinds a part fits are the assembler's, never worked out here", () => {
  const c = good();
  // The real assembler, asked as part.ts asks it: a haft is taken by these kinds' slots (assets/data/kinds).
  assert.deepEqual(c.kindsOf("haft"), { taking: ["axe", "hammer", "pickaxe", "spear", "torch"], made_for: ["axe", "hammer", "pickaxe", "spear", "torch"] });
  assert.deepEqual(c.kindsOf("haft", ["axe"]).made_for, ["axe"]);
  // An assembler that said a haft fits the axe alone would leave nothing to name: the verdict follows its answer.
  const h = haft();
  delete h.kinds;
  const alone: Ctx = { ...c, kindsOf: () => ({ taking: ["axe"], made_for: ["axe"] }) };
  assert.deepEqual(faults(h, alone, "data"), []);
  assert.equal(checkPart(h, alone).find((s) => s.stage === "slots")?.ok, "axe");
});

test("a collar whose model stops short of its cover leaves a gap under the head", () => {
  const f = faults(collar(), ctx([head(), collar()], { test_box_head: boxHead(), test_collar: collarMesh(-0.003) }), "joins");
  assert.ok(f.some((s) => /axe, with test_box_head: a 3\.0 mm gap where they meet/.test(s)), f.join("\n"));
});

test("a collar pushed up into the head is buried in it", () => {
  const f = faults(collar(), ctx([head(), collar()], { test_box_head: boxHead(), test_collar: collarMesh(0.004) }), "joins");
  assert.ok(f.some((s) => /buried 4\.0 mm/.test(s)), f.join("\n"));
});

test("an eye.upper below the head's top puts the haft's end in the head's top face", () => {
  const h = head();
  h.points[1].at = [0, 0.056, 0];
  const f = faults(h, ctx([h, haft()], { test_box_head: boxHead() }), "joins");
  assert.ok(f.some((s) => /faces of the head lie in the handle's top plane/.test(s)), f.join("\n"));
});

test("a material not named by its role is refused", () => {
  const m = { ...boxHead(), materials: ["Material.001"] };
  assert.match(faults(head(), ctx([head()], { test_box_head: m }), "roles")[0], /"Material" is not a role/);
});

test("the joins are where the assembler puts them, never placed here", () => {
  const c = good();
  // The real assembler, asked as part.ts asks it: the collar's cover top on eye.lower, the haft 4 mm proud.
  const said = c.pairings("test_box_head", Object.values(c.parts));
  assert.deepEqual(
    said.map((p) => [p.partner, p.meet.meet]).sort(),
    [["test_collar", "placed"], ["test_haft", "handle_top"]],
  );
  const top = said.find((p) => p.meet.meet === "handle_top")!.meet as { y: number; d: number };
  assert.ok(Math.abs(top.y - 0.064) < 1e-6 && Math.abs(top.d - 0.035) < 1e-6);
  // An assembler that put the collar 3 mm lower would leave a 3 mm gap: the verdict follows its placing.
  const lower: Ctx = { ...c, pairings: (id, parts) => c.pairings(id, parts).map((p) => (p.meet.meet === "placed" ? { ...p, meet: { ...p.meet, at: [0, -0.003, 0] } } : p)) };
  assert.ok(faults(collar(), lower, "joins").some((s) => /a 3\.0 mm gap where they meet/.test(s)));
  assert.deepEqual(faults(collar(), c, "joins"), []);
});
