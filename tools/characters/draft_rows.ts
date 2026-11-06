// Drafting the catalogue's rows for their newly vendored pieces (D611): each glTF piece of an
// outfit under art/characters/quaternius/modular-outfits-fantasy/ that no row's `source` names,
// on a body the catalogue has, gets a row in the shape of its neighbours, and each outfit drafted
// whole (body, arms, legs, feet) a look. What a person must still choose is marked: the item it is
// the look of is a placeholder (`<outfit>_<slot>`), the regions a headwear holds are the hair alone,
// tags are empty. Accessories (`_Acc_`) are not slots yet and are left out. Used by
// `wardrobe.ts --build`.

import type { Wardrobe } from "./build_steps.ts";

export const OUTFITS = "art/characters/quaternius/modular-outfits-fantasy";
const PART: { [part: string]: { slot: string; regions: string[] } } = {
  Body: { slot: "body", regions: ["torso"] },
  Arms: { slot: "arms", regions: ["arms"] },
  Legs: { slot: "legs", regions: ["legs"] },
  Feet: { slot: "feet", regions: ["feet"] },
  Head: { slot: "headwear", regions: ["hair"] },
};

export type Row = { id: string; row: { [k: string]: unknown } };
export type Draft = { pieces: Row[]; looks: Row[]; skipped: string[] };

/** Rows for the files of `files` (names under OUTFITS) that no piece of `w` comes from. */
export function draft(w: Wardrobe, files: string[]): Draft {
  const out: Draft = { pieces: [], looks: [], skipped: [] };
  const used = new Set(Object.values(w.pieces).map((p) => p.source));
  const ids = new Set(Object.keys(w.pieces));
  const whole = new Map<string, { body: string; pieces: { [slot: string]: string } }>();
  for (const f of files.filter((f) => f.endsWith(".gltf")).sort()) {
    const source = `${OUTFITS}/${f}`;
    if (used.has(source)) continue;
    const m = /^(Male|Female)_([A-Za-z]+)_(Acc|Body|Arms|Legs|Feet|Head)(?:_([A-Za-z_]+))?\.gltf$/.exec(f);
    if (!m) continue;
    const [, who, name, part, variant] = m;
    const body = who.toLowerCase();
    if (part === "Acc") {
      out.skipped.push(`${f}: an accessory, not a slot yet`);
      continue;
    }
    if (!w.bodies[body]) {
      out.skipped.push(`${f}: no ${body} body in the catalogue`);
      continue;
    }
    const outfit = name.toLowerCase();
    const { slot, regions } = PART[part];
    const id = [body, outfit, slot === "headwear" ? (variant ?? "headwear") : slot, slot !== "headwear" && variant ? variant : ""].filter(Boolean).join("_").toLowerCase();
    if (ids.has(id)) {
      out.skipped.push(`${f}: a piece ${id} is there already; give it its row by hand`);
      continue;
    }
    ids.add(id);
    const colourways = ["shipped", ...files.map((x) => new RegExp(`^T_${name}_(\\d+)_BaseColor\\.png$`).exec(x)?.[1]).filter((x): x is string => !!x).sort()];
    out.pieces.push({ id, row: { body, source, outfit, slot, item: `${outfit}_${slot}`, layer: 1, regions, colourways, dye_mask: null, tags: [] } });
    const look = whole.get(outfit) ?? { body, pieces: {} };
    if (!look.pieces[slot]) look.pieces[slot] = id;
    whole.set(outfit, look);
  }
  for (const [outfit, look] of whole) {
    if (w.looks[outfit] || !w.slots.every((s) => look.pieces[s])) continue;
    const name = outfit[0].toUpperCase() + outfit.slice(1);
    out.looks.push({ id: outfit, row: { about: `Their ${name} as made (drafted by wardrobe.ts --build: name its items).`, body: look.body, pieces: Object.fromEntries(w.slots.map((s) => [s, look.pieces[s]])), colourway: "shipped", hair: null } });
  }
  return out;
}

/** A value as the catalogue writes it on one line: `{ "k": v, … }`, `[a, b]`. */
function inline(v: unknown): string {
  if (Array.isArray(v)) return `[${v.map(inline).join(", ")}]`;
  if (v && typeof v === "object") return `{ ${Object.entries(v).map(([k, x]) => `${JSON.stringify(k)}: ${inline(x)}`).join(", ")} }`;
  return JSON.stringify(v);
}

/** The catalogue's text with the drafted rows added at the end of `pieces` and `looks`, the rest as written. */
export function withDraft(text: string, d: Draft): string {
  let t = text;
  if (d.pieces.length) {
    const at = t.indexOf('\n  },\n  "looks": {');
    if (at < 0) throw new Error('draft: the catalogue has no "looks" after "pieces"');
    t = `${t.slice(0, at)},\n${d.pieces.map((p) => `    ${JSON.stringify(p.id)}: ${inline(p.row)}`).join(",\n")}${t.slice(at)}`;
  }
  if (d.looks.length) {
    const at = t.indexOf('\n  },\n  "pairings": [');
    if (at < 0) throw new Error('draft: the catalogue has no "pairings" after "looks"');
    const block = (l: Row) => `    ${JSON.stringify(l.id)}: {\n${Object.entries(l.row).map(([k, v]) => `      ${JSON.stringify(k)}: ${inline(v)}`).join(",\n")}\n    }`;
    t = `${t.slice(0, at)},\n${d.looks.map(block).join(",\n")}${t.slice(at)}`;
  }
  return t;
}
