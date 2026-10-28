import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { boxHead, collar, collarMesh, glbOf, haft, head } from "./fixtures.ts";
import { type Mesh, normal, readMesh, triangles } from "./mesh.ts";
import { checkPart, type Ctx, loadCtx } from "./part.ts";

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
  const f = faults(h, ctx([h], { test_box_head: boxHead() }), "slots");
  // 2e-4 m^3 is 1.574 kg of iron (7870) and 0.520 kg of stone (2600): over the band in one, under it in the other.
  assert.ok(f.some((s) => /axe: in no material is its head inside the axe's head mass \[0\.656,1\.28\] kg \(iron 1\.574 kg, stone 0\.520 kg/.test(s)), f.join("\n"));
  assert.deepEqual(faults(head(), good(), "slots"), [], "0.9 kg in iron is inside it");
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
