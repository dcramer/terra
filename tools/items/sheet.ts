// A kind's review sheets (D565, docs/assembled-items.md "How we know it renders right", layer 4:
// the developers' eye, once a kind): every working part by every handle, each pairing assembled
// with trims chosen so it is whole (or the assembler's refusal, in plain words, in its cell), held
// in the base body's main hand as play holds it, seen from its side in the game's renderer and
// light (the workbench, through `cargo xtask gpu sheet`); and each part alone. Two pictures:
// `<out>/items.png` (working parts by row, handles by column) and `<out>/parts.png`.
//
//   node tools/items/sheet.ts <kind> [--material M] [--out DIR]
//
// --material is the working part's (assets/data/parts/materials.json; its first by default);
// --out is ~/.cache/terra-review/<kind>-sheet by default. A line a stage (D388), every run's
// output in target/items/sheet/<kind>/, named; a failure says what to do (D556).

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, join } from "node:path";
import { PARTS, readJson, ROOT } from "./kind.ts";
import { assembler } from "./part.ts";

/** One pairing on the items sheet: its parts, and the item the assembler made or why it would not. */
export type Cell = { row: string; col?: string; parts: string[]; item?: any; why?: string };

/** What `assemble item` says of a recipe: the item, or the refusal's words. */
export type Assemble = (kind: string, ids: string[], material: string) => { item?: any; why?: string };

/** The parts of the kind's slot `slot` (working, handle) that exist, in name order. */
export function partsFor(kind: any, slot: string, parts: Record<string, any>): string[] {
  const fits = kind.slots.filter((s: any) => s.slot === slot).flatMap((s: any) => s.fits);
  return Object.values(parts)
    .filter((p) => fits.includes(p.type) && (!p.kinds || p.kinds.includes(kind.kind)))
    .map((p) => p.part)
    .sort();
}

/** Every set of trims, one or none a trim slot, those with most trims first and none last. */
export function trimSets(kind: any, parts: Record<string, any>): string[][] {
  let sets: string[][] = [[]];
  for (const slot of kind.slots.filter((s: any) => s.slot === "trim")) {
    const fitting = Object.values(parts)
      .filter((p) => slot.fits.includes(p.type) && (!p.kinds || p.kinds.includes(kind.kind)))
      .map((p) => p.part)
      .sort();
    sets = sets.flatMap((set) => [...fitting.map((t) => [...set, t]), set]);
  }
  return sets.sort((a, b) => b.length - a.length);
}

/**
 * Each working part with each handle. A cell takes the first whole recipe among the trim sets,
 * turned by its place on the sheet so neighbours show different trims; with none whole, the
 * least recipe's refusal (no trim), which is what the part checker's whole stage asks too.
 */
export function cells(kind: any, parts: Record<string, any>, material: string, assemble: Assemble): Cell[] {
  const rows = partsFor(kind, "working", parts);
  const cols: (string | undefined)[] = kind.slots.some((s: any) => s.slot === "handle") ? partsFor(kind, "handle", parts) : [undefined];
  const sets = trimSets(kind, parts);
  const trimmed = sets.filter((s) => s.length);
  const out: Cell[] = [];
  rows.forEach((row, r) =>
    cols.forEach((col, c) => {
      const base = col ? [row, col] : [row];
      const turn = trimmed.length ? (r + c) % trimmed.length : 0;
      const order = [...trimmed.slice(turn), ...trimmed.slice(0, turn), []];
      for (const trims of order) {
        const got = assemble(kind.kind, [...base, ...trims], material);
        if (got.item) return void out.push({ row, col, parts: [...base, ...trims], item: got.item });
      }
      out.push({ row, col, parts: base, why: assemble(kind.kind, base, material).why });
    }),
  );
  return out;
}

const name = (part: string) => part.replace(/_(axe_)?(head|haft)$/, "").replace(/_/g, " ");
const slotOf = (part: any, kind: any) => kind.slots.find((s: any) => s.fits.includes(part.type));

/** A cell's label: its parts in plain words, and its weight and length when whole. */
export function label(cell: Cell, kind: any, parts: Record<string, any>): string {
  const words = cell.parts.map((p, i) => (i < (cell.col ? 2 : 1) ? `${name(p)} ${slotOf(parts[p], kind)?.name ?? ""}`.trim() : name(p)));
  const said = words.join(", ");
  return cell.item ? `${said}: ${cell.item.mass_kg.toFixed(2)} kg, ${cell.item.length_m.toFixed(2)} m` : said;
}

/** The assembler's refusal said as a player would: what is wrong and by how much, from its envelope numbers. */
export function plain(said: string, kind: string): string {
  const m = said.match(/'s (\w+) ([\d.]+) is outside its envelope \[([\d.]+), ([\d.]+)\]/);
  if (!m) return said.replace(/^.*✗ assemble: /s, "");
  const what = m[1];
  const [v, lo, hi] = m.slice(2).map(Number);
  const over = v > hi, bar = over ? hi : lo, most = over ? "at most" : "at least";
  const n = (x: number, places: number) => x.toFixed(places);
  switch (what) {
    case "length_m":
      return `too ${over ? "long" : "short"}: ${n(v, 2)} m\n(an ${kind} is ${most} ${n(bar, 2)} m)`;
    case "inertia_kgm2":
      return over
        ? `too hard to swing:\nits weight sits too far out\n(${n(v, 3)} kg m², the ${kind} allows ${most} ${n(bar, 3)})`
        : `too light to swing as an ${kind}:\nits weight sits too close in\n(${n(v, 3)} kg m², the ${kind} wants ${most} ${n(bar, 3)})`;
    case "head_mass_kg":
      return `its head is too ${over ? "heavy" : "light"}: ${n(v, 2)} kg\n(${most} ${n(bar, 2)} kg)`;
    case "balance_m":
      return `it balances too ${over ? "far out" : "close to the hand"}:\n${n(v, 2)} m from the butt\n(${most} ${n(bar, 2)} m)`;
    default:
      return `its ${what} ${v} is outside ${lo} to ${hi}`;
  }
}

/** `assemble item`, the item's JSON or the refusal in plain words. */
const assembleItem: Assemble = (kind, ids, material) => {
  try {
    return { item: assembler(["item", kind, ...ids, "--material", material]) };
  } catch (e) {
    return { why: plain((e as Error).message, kind) };
  }
};

type Run = { ok: boolean; picture?: string; log: string };

/** One workbench sheet through the GPU scheduler and the Windows build (D319), its picture found where the copy ran. */
function render(copy: string, shot: string, args: string[], log: string): Run {
  const run = spawnSync(
    "cargo",
    ["xtask", "gpu", "sheet", "--", "cargo", "xtask", "win", "-p", "terra_sandbox", "--copy", copy, "--", "--experiment", "workbench", ...args, "--sides", "270", "--moments", "0", "--sheet", shot, "--clean", "--settle", "60"],
    { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 },
  );
  const said = `${run.stdout}\n${run.stderr}`;
  writeFileSync(log, said);
  const dir = said.match(/\(in (\/[^)]+)\)/)?.[1];
  if (run.status !== 0 || !dir) return { ok: false, log };
  const folder = join(dir, ...shot.split("/").slice(0, -1));
  const prefix = `${basename(shot)}-`;
  const found = existsSync(folder) ? readdirSync(folder).filter((f) => f.startsWith(prefix) && f.endsWith(".png")) : [];
  return found.length === 1 ? { ok: true, picture: join(folder, found[0]), log } : { ok: false, log };
}

function magick(tool: string, args: string[], log: string): void {
  const run = spawnSync(tool, args, { encoding: "utf8" });
  if (run.status !== 0) {
    writeFileSync(log, `${tool} ${args.join(" ")}\n${run.stderr}`);
    fail("montage", `ImageMagick's ${tool} failed; see ${log}. It needs ImageMagick 6 and the DejaVu-Sans font (apt install imagemagick fonts-dejavu)`);
  }
}

function fail(stage: string, why: string): never {
  console.log(`✗ ${stage}: ${why}`);
  process.exit(1);
}

/** Lays pictures in a grid with their labels, a title over it (ImageMagick's montage). */
function montage(tiles: { file: string; label: string }[], columns: number, title: string, out: string, log: string): void {
  const args = ["-font", "DejaVu-Sans", "-pointsize", "18", "-background", "#202020", "-fill", "white", "-title", title];
  for (const t of tiles) args.push("-label", t.label, t.file);
  magick("montage", [...args, "-tile", `${columns}x`, "-geometry", "+6+6", out], log);
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const flag = (f: string) => (args.includes(f) ? args[args.indexOf(f) + 1] : undefined);
  const kindName = args.find((a, i) => !a.startsWith("--") && !args[i - 1]?.startsWith("--"));
  const kindFile = join(ROOT, "assets", "data", "kinds", `${kindName}.json`);
  if (!kindName || !existsSync(kindFile)) fail("kind", `name a kind of assets/data/kinds/ (node tools/items/sheet.ts axe)`);
  const kind = readJson(kindFile);
  const materials = Object.keys(readJson(join(PARTS, "materials.json")).materials);
  const material = flag("--material") ?? materials[0];
  if (!materials.includes(material)) fail("material", `${material} is not in assets/data/parts/materials.json: one of ${materials.join(", ")}`);
  const out = flag("--out") ?? join(homedir(), ".cache", "terra-review", `${kindName}-sheet`);
  const parts = Object.fromEntries(
    readdirSync(PARTS)
      .filter((f) => f.endsWith(".json") && !["types.json", "materials.json", "part.schema.json"].includes(f))
      .map((f) => readJson(join(PARTS, f)))
      .map((p) => [p.part, p]),
  );
  const logs = join(ROOT, "target", "items", "sheet", kindName);
  const models = join(ROOT, "assets", "models", "sheets", kindName);
  rmSync(logs, { recursive: true, force: true });
  rmSync(models, { recursive: true, force: true });
  mkdirSync(logs, { recursive: true });
  mkdirSync(models, { recursive: true });
  mkdirSync(out, { recursive: true });

  // 1. Assemble every pairing.
  const grid = cells(kind, parts, material, assembleItem);
  const whole = grid.filter((c) => c.item);
  if (grid.length === 0) fail("assemble", `the ${kindName} has no working part in assets/data/parts/ yet: make one (the models skill)`);
  console.log(`✓ assemble: ${whole.length} of ${grid.length} ${kindName}s whole in ${material}${whole.length < grid.length ? `, ${grid.length - whole.length} refused (said in their cells)` : ""}`);

  // 2. Each whole one's model, as the held-look checks read it.
  const file = (c: Cell) => [c.row, c.col].filter(Boolean).join("_");
  for (const c of whole) {
    const log = join(logs, `${file(c)}-model.log`);
    const run = spawnSync("cargo", ["run", "-q", "-p", "terra_bake", "--bin", "item", "--", kindName, ...c.parts, "--material", material, "--out", join(models, `${file(c)}.glb`)], { cwd: ROOT, encoding: "utf8" });
    writeFileSync(log, `${run.stdout}\n${run.stderr}`);
    if (run.status !== 0) fail("models", `${c.parts.join(" ")} did not bake; see ${log}`);
  }
  console.log(`✓ models: ${whole.length} in assets/models/sheets/${kindName}/ (git-ignored)`);

  // 3. Render each: a whole item in the base body's main hand at rest, each part alone.
  const copy = `items-${basename(ROOT)}`;
  const pictures = new Map<string, string>();
  const failed: string[] = [];
  const shoot = (key: string, args: string[]) => {
    const run = render(copy, `shots/sheet/${kindName}/${key}`, args, join(logs, `${key}.log`));
    if (run.picture) pictures.set(key, run.picture);
    else failed.push(run.log);
  };
  for (const c of whole) shoot(file(c), ["--model", "models/character/base.glb", "--attach", `models/sheets/${kindName}/${file(c)}.glb@socket.hand.R`, "--focus", "socket.hand.R", "--anim", "Idle_Loop"]);
  const shown = Object.values(parts).filter((p) => slotOf(p, kind) && (!p.kinds || p.kinds.includes(kindName)) && (p.model ?? p.skin));
  shown.sort((a, b) => kind.slots.indexOf(slotOf(a, kind)) - kind.slots.indexOf(slotOf(b, kind)) || a.part.localeCompare(b.part));
  for (const p of shown) shoot(`part_${p.part}`, ["--model", p.model ?? p.skin, "--focus", "whole"]);
  if (failed.length) fail("render", `${failed.length} runs made no picture; read the end of ${failed.join(", ")} (a panic names its summary.txt, D399)`);
  console.log(`✓ render: ${pictures.size} pictures, the runs' output in target/items/sheet/${kindName}/`);

  // 4. The two sheets: refused cells grey with the reason, the same size as the pictures.
  const size = spawnSync("identify", ["-format", "%wx%h", [...pictures.values()][0]], { encoding: "utf8" }).stdout.trim();
  const cols = new Set(grid.map((c) => c.col)).size;
  const tiles = grid.map((c) => {
    if (c.item) return { file: pictures.get(file(c))!, label: label(c, kind, parts) };
    const grey = join(logs, `${file(c)}-refused.png`);
    magick("convert", ["-size", size, "-background", "#5a5a5a", "-fill", "white", "-font", "DejaVu-Sans", "-pointsize", "26", "-gravity", "center", `caption:REFUSED\n\n${c.why}`, grey], join(logs, "montage.log"));
    return { file: grey, label: label(c, kind, parts) };
  });
  const by = kind.slots.find((s: any) => s.slot === "handle")?.name;
  montage(tiles, cols, `${kindName}s in ${material}: ${kind.slots.find((s: any) => s.slot === "working").name}s by row${by ? `, ${by}s by column` : ""}`, join(out, "items.png"), join(logs, "montage.log"));
  const partTiles = shown.map((p) => ({ file: pictures.get(`part_${p.part}`)!, label: `${name(p.part)} ${slotOf(p, kind).slot === "trim" ? "" : slotOf(p, kind).name}${p.model ? "" : " (its look; code turns it to length)"}`.trim() }));
  montage(partTiles, Math.min(4, partTiles.length), `the ${kindName}'s parts, each alone`, join(out, "parts.png"), join(logs, "montage.log"));
  console.log(`✓ montage: ${join(out, "items.png")} and ${join(out, "parts.png")}`);
}
