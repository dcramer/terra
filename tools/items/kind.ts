// The kind check (D565, D566): every kind in assets/data/kinds/ against kind.schema.json, plus what a
// schema cannot say: the file is named for its kind, every number sits beside its source, every
// slot fits part types that exist (assets/data/parts/types.json) and are made as it says, and the
// reference's inertia and envelope follow D459's rule from its own numbers.
//
//   node tools/items/kind.ts            every kind
//   node tools/items/kind.ts axe bow    those
//
// A line a kind (D388); a failure names the file, the field and the fix (D556).

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { type Fault, validate } from "./schema.ts";

export const ROOT = join(import.meta.dirname, "..", "..");
export const KINDS = join(ROOT, "assets", "data", "kinds");
export const PARTS = join(ROOT, "assets", "data", "parts");

// D459's tempo clamp: an item's tempo is sqrt(I_ref / I), held within 0.85 to 1.15.
const TEMPO = [0.85, 1.15];
// How near a written number must be to the one it is computed from: the files round to 3 or 4 places.
const ROUNDING = 0.01;

export const readJson = (file: string) => JSON.parse(readFileSync(file, "utf8"));

export function checkKind(kind: any, name: string, schema: any, types: Record<string, any>): Fault[] {
  const faults = validate(schema, kind);
  if (kind?.kind !== undefined && kind.kind !== name) faults.push({ at: "/kind", says: `is ${JSON.stringify(kind.kind)} but the file is ${name}.json; name the file for its kind` });
  faults.push(...unsourced(kind, ""));
  for (const [i, slot] of (kind?.slots ?? []).entries()) {
    for (const t of slot?.fits ?? []) {
      const type = types[t];
      if (!type) faults.push({ at: `/slots/${i}/fits`, says: `names part type ${t}, which assets/data/parts/types.json does not have; add it there or fit one it has` });
      else if (type.made_by !== slot.made_by) faults.push({ at: `/slots/${i}/made_by`, says: `is ${slot.made_by} but part type ${t} is made by ${type.made_by} (types.json); make them agree` });
      else if (type.slot !== slot.slot) faults.push({ at: `/slots/${i}/slot`, says: `is ${slot.slot} but part type ${t} goes in a ${type.slot} slot (types.json)` });
    }
  }
  faults.push(...inertia(kind));
  return faults;
}

// Every number in a kind is a fact with its source beside it (D566: the table refuses a blank).
function unsourced(v: any, at: string, parent?: any): Fault[] {
  if (typeof v === "number") {
    const ok = parent && typeof parent.source === "string" && /\S/.test(parent.source);
    return ok ? [] : [{ at, says: `is a number with no source beside it; give its object a "source" saying where it comes from` }];
  }
  if (Array.isArray(v)) return v.flatMap((x, i) => unsourced(x, `${at}/${i}`, parent));
  if (v && typeof v === "object") return Object.entries(v).flatMap(([k, x]) => unsourced(x, `${at}/${k}`, v));
  return [];
}

const num = (x: any): number | undefined => (typeof x?.value === "number" ? x.value : undefined);
const near = (a: number, b: number) => Math.abs(a - b) <= ROUNDING * Math.abs(b);

function inertia(kind: any): Fault[] {
  const ref = kind?.reference ?? {};
  const env = kind?.envelope ?? {};
  const out: Fault[] = [];
  const [m, b, l, i] = [num(ref.mass_kg), num(ref.balance_m), num(ref.length_m), num(ref.inertia_kgm2)];
  if (m !== undefined && b !== undefined && l !== undefined && i !== undefined) {
    const want = m * (b * b + (l * l) / 12);
    if (!near(i, want)) out.push({ at: "/reference/inertia_kgm2", says: `is ${i} but D459's I = m (b^2 + L^2/12) on the reference's mass, balance and length gives ${want.toFixed(4)}; write that, or fix the number that is wrong` });
    const band = env.inertia_kgm2?.value;
    if (Array.isArray(band)) {
      const [lo, hi] = [i / TEMPO[1] ** 2, i / TEMPO[0] ** 2];
      if (!near(band[0], lo) || !near(band[1], hi))
        out.push({ at: "/envelope/inertia_kgm2", says: `is [${band}] but D459's tempo clamp (0.85 to 1.15) on I_ref ${i} gives [${lo.toFixed(4)}, ${hi.toFixed(4)}]` });
    }
  }
  for (const key of ["inertia_kgm2", "length_m", "head_mass_kg", "balance_m"]) {
    const band = env[key]?.value;
    const r = num(ref[key]);
    if (!Array.isArray(band)) continue;
    if (!(band[0] < band[1])) out.push({ at: `/envelope/${key}`, says: `is [${band}]: its low must be below its high` });
    else if (r !== undefined && (r < band[0] || r > band[1])) out.push({ at: `/envelope/${key}`, says: `is [${band}], which leaves out the reference's ${r}: an envelope is round its reference` });
  }
  return out;
}

export function kindNames(): string[] {
  return readdirSync(KINDS)
    .filter((f) => f.endsWith(".json") && f !== "kind.schema.json")
    .map((f) => f.slice(0, -5))
    .sort();
}

export function fixLine(name: string, f: Fault): string {
  return `✗ ${name}: ${f.at || "/"} ${f.says} (assets/data/kinds/${name}.json; schema kind.schema.json; docs/assembled-items.md, The kinds table, D566)`;
}

if (import.meta.main) {
  const schema = readJson(join(KINDS, "kind.schema.json"));
  const types = readJson(join(PARTS, "types.json")).types;
  const asked = process.argv.slice(2);
  const names = asked.length ? asked : kindNames();
  let bad = 0;
  for (const name of names) {
    let kind: any;
    try {
      kind = readJson(join(KINDS, `${name}.json`));
    } catch (e) {
      console.log(`✗ ${name}: cannot read assets/data/kinds/${name}.json (${(e as Error).message}); the kinds are ${kindNames().join(", ")}`);
      bad++;
      continue;
    }
    const faults = checkKind(kind, name, schema, types);
    if (faults.length) bad++;
    console.log(faults.length ? faults.map((f) => fixLine(name, f)).join("\n") : `✓ ${name}`);
  }
  console.log(bad ? `✗ kinds: ${bad} of ${names.length} refused` : `✓ kinds: ${names.length} hold to their schema and D459's rule`);
  process.exit(bad ? 1 : 0);
}
