import assert from "node:assert/strict";
import { join } from "node:path";
import { test } from "node:test";
import { checkKind, KINDS, kindNames, PARTS, readJson } from "./kind.ts";
import { validate } from "./schema.ts";

const schema = readJson(join(KINDS, "kind.schema.json"));
const types = readJson(join(PARTS, "types.json")).types;
const axe = () => readJson(join(KINDS, "axe.json"));
const faults = (kind: any, name = "axe") => checkKind(kind, name, schema, types);
const said = (kind: any, at: string) => faults(kind).filter((f) => f.at === at);

test("every kind holds to its schema, its part types and D459's rule", () => {
  for (const name of kindNames()) assert.deepEqual(faults(readJson(join(KINDS, `${name}.json`)), name), [], name);
});

test("no kind leaves a question to the developers once they have answered it (D567)", () => {
  for (const name of kindNames()) assert.ok(!JSON.stringify(readJson(join(KINDS, `${name}.json`))).includes('"developers"'), name);
});

test("a blank situation is refused at its field", () => {
  const k = axe();
  k.carried.how = "";
  assert.match(said(k, "/carried/how")[0].says, /too short/);
});

test("an open situation without its owner and why is refused", () => {
  const k = axe();
  delete k.held.open;
  assert.match(said(k, "/held/open")[0].says, /missing/);
});

test("a number without its source is refused, wherever it is", () => {
  const k = axe();
  delete k.reference.mass_kg.source;
  assert.ok(faults(k).some((f) => f.at.startsWith("/reference/mass_kg")));
  const deep = axe();
  deep.slots[0].depth = 3;
  assert.ok(faults(deep).some((f) => f.at === "/slots/0/depth" && /no source/.test(f.says)));
});

test("an inertia not D459's rule on the reference's own numbers is refused, with the right one", () => {
  const k = axe();
  k.reference.inertia_kgm2.value = 0.4;
  assert.match(said(k, "/reference/inertia_kgm2")[0].says, /gives 0\.3206/);
});

test("an envelope that leaves out its reference is refused", () => {
  const k = axe();
  k.envelope.length_m.value = [0.7, 0.8];
  assert.match(said(k, "/envelope/length_m")[0].says, /leaves out the reference's 0\.68/);
});

test("a slot fitting a part type that does not exist, or made otherwise than its type, is refused", () => {
  const k = axe();
  k.slots[0].fits = ["axe_blade"];
  assert.match(said(k, "/slots/0/fits")[0].says, /types\.json does not have/);
  const m = axe();
  m.slots[1].made_by = "model";
  assert.match(said(m, "/slots/1/made_by")[0].says, /made by code/);
});

test("a third trim is refused (D565 answer 2)", () => {
  const k = axe();
  k.slots.push({ ...k.slots[2], name: "wrap" });
  assert.ok(said(k, "/slots").some((f) => /at most 2/.test(f.says)));
});

test("a file not named for its kind is refused", () => {
  assert.match(faults(axe(), "sword").find((f) => f.at === "/kind")!.says, /file is sword\.json/);
});

test("the validator refuses a schema keyword it does not know, rather than ignore it", () => {
  assert.throws(() => validate({ type: "object", minProperties: 1 }, {}), /minProperties is not supported/);
});

test("a oneOf reports the shape the data came closest to", () => {
  const k = axe();
  k.hands = { value: "Main" };
  assert.deepEqual(said(k, "/hands/source").length, 1);
});
