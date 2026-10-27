#!/usr/bin/env node
// The worn looks build_bodies.sh and build_item.sh make, read from each item's data,
// art/gear/<item>/fit.json (docs/attachment.md, D546), so adding a worn item is adding its
// files, never editing a script. A line an item, in the words the makers take:
//
//   node tools/characters/gear_list.ts garments   # "item maker role coverage model flags...", made in this order
//   node tools/characters/gear_list.ts rigid      # the pieces fit_gear.py makes (packs, belt pieces), a name a line
//   node tools/characters/gear_list.ts one ITEM   # ITEM's line: a garment's, or "fit_gear ITEM"
//
// A garment's maker is its data's "fit": "shell" (shell_garment.py) or "hung" (hung_garment.py,
// what hangs: a skirt, an apron); its flags come from "shape", "over", "under" and "plain". A
// piece worn over or under another (its "over" and "under") is made after it, so the garments
// come in that order, names breaking ties. Node 22.18 or later; no dependencies.

import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dirname, "..", "..");

export type GearData = {
  kind?: string;
  fit?: string;
  model?: string;
  role?: string;
  coverage?: string;
  shape?: string;
  over?: string[];
  under?: string[];
  plain?: boolean;
};

/** Every worn item with data, art/gear/<item>/fit.json, by name. */
export function gearData(root = ROOT): Map<string, GearData> {
  const dir = join(root, "art", "gear");
  const items = new Map<string, GearData>();
  for (const item of readdirSync(dir).sort()) {
    const file = join(dir, item, "fit.json");
    if (existsSync(file)) items.set(item, JSON.parse(readFileSync(file, "utf8")));
  }
  return items;
}

/** A garment's line for its maker: "item maker role coverage art/gear/<model>/model.glb flags". */
export function garmentLine(item: string, d: GearData): string {
  if (d.fit !== "shell" && d.fit !== "hung") throw new Error(`${item}: a garment's fit is "shell" or "hung", not ${JSON.stringify(d.fit)}`);
  for (const key of ["role", "coverage", "model"] as const) if (typeof d[key] !== "string") throw new Error(`${item}: its data gives no "${key}"`);
  const flags = [
    ...(d.shape ? [`--shape=${d.shape}`] : []),
    ...(d.over?.length ? [`--over=${d.over.join(",")}`] : []),
    ...(d.under?.length ? [`--under=${d.under.join(",")}`] : []),
    ...(d.plain ? ["--plain"] : []),
  ];
  return [item, d.fit, d.role, d.coverage, `art/gear/${d.model}/model.glb`, ...flags].join(" ");
}

/** The garments in the order they are made: each after every piece it is worn over or under. */
export function garmentOrder(items: Map<string, GearData>): string[] {
  const garments = [...items].filter(([, d]) => d.kind === "garment").map(([item]) => item);
  const needs = new Map(garments.map((g) => [g, [...(items.get(g)!.over ?? []), ...(items.get(g)!.under ?? [])]]));
  for (const [g, ns] of needs) for (const n of ns) if (!needs.has(n)) throw new Error(`${g}: worn over or under ${n}, which is no garment with data`);
  const order: string[] = [];
  while (order.length < garments.length) {
    const next = garments.find((g) => !order.includes(g) && needs.get(g)!.every((n) => order.includes(n)));
    if (!next) throw new Error(`these are worn over or under each other in a circle: ${garments.filter((g) => !order.includes(g)).join(", ")}`);
    order.push(next);
  }
  return order;
}

/** The pieces fit_gear.py makes from their own models: packs and belt pieces. */
export function rigidPieces(items: Map<string, GearData>): string[] {
  return [...items].filter(([, d]) => d.kind !== "garment").map(([item]) => item);
}

function main() {
  const [what, item] = process.argv.slice(2);
  const items = gearData();
  if (what === "garments") for (const g of garmentOrder(items)) console.log(garmentLine(g, items.get(g)!));
  else if (what === "rigid") for (const r of rigidPieces(items)) console.log(r);
  else if (what === "one" && item) {
    const d = items.get(item);
    if (!d) throw new Error(`${item}: no art/gear/${item}/fit.json`);
    console.log(d.kind === "garment" ? garmentLine(item, d) : `fit_gear ${item}`);
  } else {
    console.error("usage: gear_list.ts garments | rigid | one ITEM");
    process.exit(1);
  }
}

if (import.meta.main) main();
